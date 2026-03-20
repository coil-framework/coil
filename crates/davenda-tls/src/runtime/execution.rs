mod issued;
mod manual;

pub use issued::{AcmeTlsCertificateExecutor, CloudflareTlsCertificateExecutor};
pub use manual::ManualImportTlsCertificateExecutor;

use std::fmt;

use super::planning::{IssuancePlan, RenewalPlan};
use crate::material::{CertificateMaterial, ManualCertificateBundle};
use crate::{CertificateId, CertificateRecord, TlsInstant, TlsModelError};

pub trait TlsCertificateExecutor: fmt::Debug + Send + Sync {
    fn import_manual_certificate(
        &self,
        bundle: ManualCertificateBundle,
    ) -> Result<(), TlsModelError>;

    fn issue_certificate(
        &self,
        plan: &IssuancePlan,
        certificate_id: CertificateId,
        issued_at: TlsInstant,
    ) -> Result<CertificateRecord, TlsModelError>;

    fn renew_certificate(
        &self,
        plan: &RenewalPlan,
        certificate_id: CertificateId,
        replacement_certificate_id: CertificateId,
        issued_at: TlsInstant,
    ) -> Result<CertificateRecord, TlsModelError>;

    fn certificate_material(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateMaterial, TlsModelError>;
}
