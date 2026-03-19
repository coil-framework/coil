use crate::{
    CertificateId, CertificateRecord, CertificateStatus, Hostname, TlsModelError,
};

use super::planning::{ChallengeTicket, HotReloadEvent, RenewalPlan};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TlsAutomationState {
    pub inventory: CertificateInventory,
    pub renewal_queue: Vec<RenewalPlan>,
    pub pending_challenges: Vec<ChallengeTicket>,
    pub hot_reload_events: Vec<HotReloadEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CertificateInventory {
    certificates: Vec<CertificateRecord>,
}

impl CertificateInventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn certificates(&self) -> &[CertificateRecord] {
        &self.certificates
    }

    pub fn active_for_hostname(&self, hostname: &Hostname) -> Option<&CertificateRecord> {
        self.certificates.iter().find(|record| {
            record
                .bindings
                .iter()
                .any(|binding| &binding.hostname == hostname)
                && matches!(
                    record.status,
                    CertificateStatus::Active
                        | CertificateStatus::RenewalDue
                        | CertificateStatus::Renewing
                )
        })
    }

    pub fn record(&self, certificate_id: &CertificateId) -> Option<&CertificateRecord> {
        self.certificates
            .iter()
            .find(|record| &record.id == certificate_id)
    }

    pub fn record_mut(&mut self, certificate_id: &CertificateId) -> Option<&mut CertificateRecord> {
        self.certificates
            .iter_mut()
            .find(|record| &record.id == certificate_id)
    }

    pub fn insert(&mut self, record: CertificateRecord) -> Result<(), TlsModelError> {
        self.ensure_unique_bindings(&record, None)?;
        self.certificates.push(record);
        Ok(())
    }

    pub fn activate_replacement(
        &mut self,
        certificate_id: &CertificateId,
        replacement: CertificateRecord,
    ) -> Result<(), TlsModelError> {
        let original =
            self.record(certificate_id)
                .ok_or_else(|| TlsModelError::UnknownCertificate {
                    certificate_id: certificate_id.to_string(),
                })?;
        if original.replacing_certificate.as_ref() != Some(&replacement.id) {
            return Err(TlsModelError::MissingReplacementCertificate {
                certificate_id: certificate_id.to_string(),
            });
        }

        self.ensure_unique_bindings(&replacement, Some(certificate_id))?;
        let original =
            self.record_mut(certificate_id)
                .ok_or_else(|| TlsModelError::UnknownCertificate {
                    certificate_id: certificate_id.to_string(),
                })?;
        original.status = CertificateStatus::Superseded;
        original.replacing_certificate = None;

        self.certificates.push(replacement);
        Ok(())
    }

    fn ensure_unique_bindings(
        &self,
        candidate: &CertificateRecord,
        allowing_replaced_certificate: Option<&CertificateId>,
    ) -> Result<(), TlsModelError> {
        for binding in &candidate.bindings {
            if let Some(existing) = self.active_for_hostname(&binding.hostname) {
                let allowed = allowing_replaced_certificate
                    .is_some_and(|certificate_id| &existing.id == certificate_id);
                if !allowed {
                    return Err(TlsModelError::DuplicateHostnameBinding {
                        hostname: binding.hostname.to_string(),
                        certificate_id: existing.id.to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

impl TlsAutomationState {
    pub fn new() -> Self {
        Self::default()
    }
}
