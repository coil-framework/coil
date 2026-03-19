use std::fmt;

use super::planning::TlsRuntime;
use super::planning::{ChallengeTicket, HotReloadEvent, RenewalPlan};
use super::state::TlsAutomationState;
use crate::{CertificateId, CertificateRecord, TlsInstant, TlsModelError};

mod file;
mod memory;

pub use file::FileTlsAutomationBackend;
pub use memory::MemoryTlsAutomationBackend;

pub trait TlsAutomationBackend: fmt::Debug + Send + Sync {
    fn snapshot(&self) -> TlsAutomationState;
    fn import_certificate(&self, record: CertificateRecord) -> Result<(), TlsModelError>;
    fn queue_renewal(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, TlsModelError>;
    fn begin_renewal(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, TlsModelError>;
    fn fail_renewal(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, TlsModelError>;
    fn activate_replacement(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, TlsModelError>;
}

pub fn default_state_path(scope: impl Into<String>) -> std::path::PathBuf {
    let base = std::env::var_os("DAVENDA_TLS_STATE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("davenda/tls"));
    base.join(format!("{}.json", sanitize_state_scope(scope.into())))
}

fn sanitize_state_scope(scope: String) -> String {
    scope
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}
