use std::fs;
use std::path::PathBuf;

use super::super::planning::TlsRuntime;
use super::super::state::TlsAutomationState;
use super::TlsAutomationBackend;
use crate::{CertificateId, CertificateRecord, CertificateStatus, TlsInstant, TlsModelError};
use serde_json;

use super::super::planning::{ChallengeTicket, HotReloadEvent, RenewalPlan};

#[derive(Debug, Clone)]
pub struct FileTlsAutomationBackend {
    path: PathBuf,
}

impl FileTlsAutomationBackend {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn load_state(&self) -> Result<TlsAutomationState, TlsModelError> {
        match fs::read_to_string(&self.path) {
            Ok(contents) => serde_json::from_str(&contents).map_err(|error| {
                TlsModelError::CorruptAutomationState {
                    path: self.path.display().to_string(),
                    reason: error.to_string(),
                }
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(TlsAutomationState::default())
            }
            Err(error) => Err(TlsModelError::AutomationStatePersistence {
                path: self.path.display().to_string(),
                reason: error.to_string(),
            }),
        }
    }

    fn persist_state(&self, state: &TlsAutomationState) -> Result<(), TlsModelError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TlsModelError::AutomationStatePersistence {
                    path: self.path.display().to_string(),
                    reason: error.to_string(),
                }
            })?;
        }

        let tmp_path =
            self.path
                .with_extension(format!("tmp-{}-{}", std::process::id(), current_nanos()));
        let serialized = serde_json::to_string_pretty(state).map_err(|error| {
            TlsModelError::AutomationStatePersistence {
                path: self.path.display().to_string(),
                reason: error.to_string(),
            }
        })?;
        fs::write(&tmp_path, serialized).map_err(|error| {
            TlsModelError::AutomationStatePersistence {
                path: self.path.display().to_string(),
                reason: error.to_string(),
            }
        })?;
        fs::rename(&tmp_path, &self.path).map_err(|error| {
            TlsModelError::AutomationStatePersistence {
                path: self.path.display().to_string(),
                reason: error.to_string(),
            }
        })?;
        Ok(())
    }
}

impl TlsAutomationBackend for FileTlsAutomationBackend {
    fn snapshot(&self) -> TlsAutomationState {
        self.load_state()
            .expect("TLS automation state should be readable")
    }

    fn import_certificate(&self, record: CertificateRecord) -> Result<(), TlsModelError> {
        let mut state = self.load_state()?;
        state.inventory.insert(record)?;
        self.persist_state(&state)
    }

    fn queue_renewal(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, TlsModelError> {
        let mut state = self.load_state()?;
        let record = state
            .inventory
            .record(certificate_id)
            .cloned()
            .ok_or_else(|| TlsModelError::UnknownCertificate {
                certificate_id: certificate_id.to_string(),
            })?;
        let plan = runtime.planner().renewal_plan(&record, now)?;
        if plan.renew_after > now {
            return Err(TlsModelError::RenewalNotDue {
                certificate_id: certificate_id.to_string(),
                renew_after: plan.renew_after,
                now,
            });
        }

        if let Some(existing) = state
            .renewal_queue
            .iter()
            .find(|plan| plan.certificate_id == *certificate_id)
        {
            return Err(TlsModelError::RenewalAlreadyInProgress {
                certificate_id: existing.certificate_id.to_string(),
            });
        }

        if let Some(record) = state.inventory.record_mut(certificate_id) {
            record.status = CertificateStatus::RenewalDue;
        }
        state.renewal_queue.push(plan.clone());
        self.persist_state(&state)?;
        Ok(plan)
    }

    fn begin_renewal(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, TlsModelError> {
        let mut state = self.load_state()?;
        let record = state.inventory.record_mut(certificate_id).ok_or_else(|| {
            TlsModelError::UnknownCertificate {
                certificate_id: certificate_id.to_string(),
            }
        })?;
        if record.replacing_certificate.is_some() {
            return Err(TlsModelError::RenewalAlreadyInProgress {
                certificate_id: certificate_id.to_string(),
            });
        }

        record.status = CertificateStatus::Renewing;
        record.replacing_certificate = Some(replacement_certificate_id.clone());

        let ticket = ChallengeTicket {
            certificate_id: certificate_id.clone(),
            replacement_certificate_id: Some(replacement_certificate_id),
            provider: record.provider,
            challenge: runtime.challenge,
            bindings: record.bindings.clone(),
            account_secret_ref: runtime.account_secret_ref.clone(),
        };
        state.pending_challenges.push(ticket.clone());
        self.persist_state(&state)?;
        Ok(ticket)
    }

    fn fail_renewal(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, TlsModelError> {
        let mut state = self.load_state()?;
        let record = {
            let record = state.inventory.record_mut(certificate_id).ok_or_else(|| {
                TlsModelError::UnknownCertificate {
                    certificate_id: certificate_id.to_string(),
                }
            })?;
            record.status = CertificateStatus::RenewalDue;
            record.replacing_certificate = None;
            record.clone()
        };
        state
            .pending_challenges
            .retain(|ticket| &ticket.certificate_id != certificate_id);
        state
            .renewal_queue
            .retain(|plan| &plan.certificate_id != certificate_id);
        self.persist_state(&state)?;
        Ok(record)
    }

    fn activate_replacement(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        mut replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, TlsModelError> {
        let mut state = self.load_state()?;
        replacement.status = CertificateStatus::Active;
        replacement.replacing_certificate = None;

        state
            .inventory
            .activate_replacement(certificate_id, replacement.clone())?;
        state
            .pending_challenges
            .retain(|ticket| &ticket.certificate_id != certificate_id);
        state
            .renewal_queue
            .retain(|plan| &plan.certificate_id != certificate_id);

        let event = HotReloadEvent {
            certificate_id: replacement.id.clone(),
            bindings: replacement.bindings.clone(),
            reloaded_without_restart: runtime.hot_reload_supported,
        };
        state.hot_reload_events.push(event.clone());
        self.persist_state(&state)?;
        Ok(event)
    }
}

fn current_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is after unix epoch")
        .as_nanos()
}
