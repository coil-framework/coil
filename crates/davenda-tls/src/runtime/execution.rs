#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
mod issued;
mod manual;
mod real;

pub use manual::ManualImportTlsCertificateExecutor;
pub use real::{AcmeTlsCertificateExecutor, CloudflareTlsCertificateExecutor};

use std::fmt;

use super::planning::{IssuancePlan, RenewalPlan};
use crate::material::{CertificateMaterial, ManualCertificateBundle};
use crate::{
    CertificateId, CertificateProviderKind, CertificateRecord, ChallengeStrategy, TlsInstant,
    TlsModelError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeValidationCheck {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeValidation {
    pub provider: CertificateProviderKind,
    pub configured_challenge: Option<ChallengeStrategy>,
    pub effective_challenge: Option<ChallengeStrategy>,
    pub shared_across_nodes: bool,
    pub requires_hot_reload: bool,
    pub checks: Vec<ChallengeValidationCheck>,
}

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

    fn validate_issuance_plan(
        &self,
        plan: &IssuancePlan,
    ) -> Result<ChallengeValidation, TlsModelError>;
}
