use std::fmt;

use super::control_plane::TlsControlPlaneRuntime;
use crate::material::{CertificateMaterial, ManualCertificateBundle, TlsMaterialProtector};
use crate::{CertificateId, TlsModelError};

pub trait TlsCertificateExecutor: fmt::Debug + Send + Sync {
    fn import_manual_certificate(
        &self,
        bundle: ManualCertificateBundle,
    ) -> Result<(), TlsModelError>;

    fn certificate_material(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateMaterial, TlsModelError>;
}

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
}
