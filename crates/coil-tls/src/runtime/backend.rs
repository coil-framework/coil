use std::fmt;

use super::planning::TlsRuntime;
use super::planning::{ChallengeTicket, HotReloadEvent, RenewalPlan};
use super::state::TlsControlPlaneState;
use crate::{CertificateId, CertificateRecord, TlsInstant, TlsModelError};

mod memory;
mod shared;
#[cfg(test)]
mod testing;

pub(super) use memory::MemoryTlsControlPlaneStore;
pub use shared::PostgresTlsControlPlaneStore;
#[cfg(test)]
pub(crate) use testing::TestPersistenceTlsControlPlaneStore;
#[cfg(test)]
pub(crate) use testing::test_persistence_state_path;

pub trait TlsControlPlaneStore: fmt::Debug + Send + Sync {
    fn snapshot(&self) -> TlsControlPlaneState;
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
