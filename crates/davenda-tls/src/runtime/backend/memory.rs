#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use super::super::planning::TlsRuntime;
use super::super::planning::{ChallengeTicket, HotReloadEvent, RenewalPlan};
use super::super::state::TlsAutomationState;
use super::TlsAutomationBackend;
use crate::{CertificateId, CertificateRecord, CertificateStatus, TlsInstant, TlsModelError};

#[derive(Debug, Clone, Default)]
pub struct MemoryTlsAutomationBackend {
    state: Arc<Mutex<TlsAutomationState>>,
}

impl MemoryTlsAutomationBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, TlsAutomationState> {
        self.state
            .lock()
            .expect("TLS automation state lock poisoned")
    }
}

impl TlsAutomationBackend for MemoryTlsAutomationBackend {
    fn snapshot(&self) -> TlsAutomationState {
        self.lock_state().clone()
    }

    fn import_certificate(&self, record: CertificateRecord) -> Result<(), TlsModelError> {
        self.lock_state().inventory.insert(record)
    }

    fn queue_renewal(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, TlsModelError> {
        let mut state = self.lock_state();
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
        Ok(plan)
    }

    fn begin_renewal(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, TlsModelError> {
        let mut state = self.lock_state();
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
        Ok(ticket)
    }

    fn fail_renewal(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, TlsModelError> {
        let mut state = self.lock_state();
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
        Ok(record)
    }

    fn activate_replacement(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        mut replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, TlsModelError> {
        let mut state = self.lock_state();
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
        Ok(event)
    }
}
