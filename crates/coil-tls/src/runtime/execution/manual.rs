use super::super::control_plane::TlsControlPlaneRuntime;
use super::super::planning::{IssuancePlan, RenewalPlan};
use super::TlsCertificateExecutor;
use crate::material::{CertificateMaterial, ManualCertificateBundle, TlsMaterialProtector};
use crate::{CertificateId, CertificateRecord, TlsInstant, TlsModelError};

#[derive(Debug, Clone)]
pub struct ManualImportTlsCertificateExecutor {
    control_plane: TlsControlPlaneRuntime,
    protector: TlsMaterialProtector,
}

impl ManualImportTlsCertificateExecutor {
    pub fn new(control_plane: TlsControlPlaneRuntime, protector: TlsMaterialProtector) -> Self {
        Self {
            control_plane,
            protector,
        }
    }
}

impl TlsCertificateExecutor for ManualImportTlsCertificateExecutor {
    fn import_manual_certificate(
        &self,
        bundle: ManualCertificateBundle,
    ) -> Result<(), TlsModelError> {
        let record = bundle.into_encrypted_record(&self.protector)?;
        self.control_plane.import_certificate(record)
    }

    fn issue_certificate(
        &self,
        _plan: &IssuancePlan,
        _certificate_id: CertificateId,
        _issued_at: TlsInstant,
    ) -> Result<CertificateRecord, TlsModelError> {
        Err(TlsModelError::ManualModeRequiresImportedCertificate)
    }

    fn renew_certificate(
        &self,
        _plan: &RenewalPlan,
        _certificate_id: CertificateId,
        _replacement_certificate_id: CertificateId,
        _issued_at: TlsInstant,
    ) -> Result<CertificateRecord, TlsModelError> {
        Err(TlsModelError::ManualModeRequiresImportedCertificate)
    }

    fn certificate_material(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateMaterial, TlsModelError> {
        let record = self
            .control_plane
            .inventory()
            .record(certificate_id)
            .cloned()
            .ok_or_else(|| TlsModelError::UnknownCertificate {
                certificate_id: certificate_id.to_string(),
            })?;
        let material =
            record
                .material
                .ok_or_else(|| TlsModelError::MissingCertificateMaterial {
                    certificate_id: certificate_id.to_string(),
                })?;
        self.protector
            .decrypt(&material)
            .map_err(|error| match error {
                TlsModelError::UnsupportedEncryptedMaterialKey { key_id } => {
                    TlsModelError::UnsupportedEncryptedMaterialKey { key_id }
                }
                other => other,
            })
    }

    fn validate_issuance_plan(
        &self,
        _plan: &IssuancePlan,
    ) -> Result<super::ChallengeValidation, TlsModelError> {
        Err(TlsModelError::ManualModeRequiresImportedCertificate)
    }
}
