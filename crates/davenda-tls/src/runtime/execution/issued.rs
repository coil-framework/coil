use super::super::control_plane::TlsControlPlaneRuntime;
use super::super::planning::{IssuancePlan, RenewalPlan};
use super::TlsCertificateExecutor;
use crate::material::{CertificateMaterial, TlsMaterialProtector};
use crate::{
    CertificateFingerprint, CertificateId, CertificateProviderKind, CertificateRecord,
    CertificateStateStore, CertificateStatus, CloudflareEncryptionMode, HostnameBinding,
    SecretMaterialRef, TlsInstant, TlsModelError,
};
use sha2::Digest;

#[derive(Debug, Clone)]
struct IssuedCertificateExecutor {
    provider: CertificateProviderKind,
    control_plane: TlsControlPlaneRuntime,
    protector: TlsMaterialProtector,
}

impl IssuedCertificateExecutor {
    fn new(
        provider: CertificateProviderKind,
        control_plane: TlsControlPlaneRuntime,
        protector: TlsMaterialProtector,
    ) -> Self {
        Self {
            provider,
            control_plane,
            protector,
        }
    }

    fn issue_record(
        &self,
        certificate_id: CertificateId,
        bindings: &[HostnameBinding],
        state_store: CertificateStateStore,
        cloudflare_mode: Option<CloudflareEncryptionMode>,
        issued_at: TlsInstant,
        not_after: TlsInstant,
        seed: impl AsRef<str>,
    ) -> Result<CertificateRecord, TlsModelError> {
        let material = issued_certificate_material(
            self.provider,
            certificate_id.as_str(),
            bindings,
            issued_at,
            not_after,
            seed.as_ref(),
            cloudflare_mode,
        )?;
        let encrypted = self.protector.encrypt(&material)?;
        let mut record = CertificateRecord::new(
            certificate_id.clone(),
            self.provider,
            CertificateStatus::Active,
            certificate_fingerprint(&material)?,
            issued_at,
            not_after,
            SecretMaterialRef::new(format!("secrets/tls/{certificate_id}"))?,
            state_store,
        )
        .with_material(encrypted);

        for binding in bindings.iter().cloned() {
            record = record.with_binding(binding);
        }

        if let Some(mode) = cloudflare_mode {
            record = record.with_cloudflare_mode(mode);
        }

        Ok(record)
    }
}

impl TlsCertificateExecutor for IssuedCertificateExecutor {
    fn import_manual_certificate(
        &self,
        _bundle: crate::material::ManualCertificateBundle,
    ) -> Result<(), TlsModelError> {
        Err(TlsModelError::ManualModeRequiresImportedCertificate)
    }

    fn issue_certificate(
        &self,
        plan: &IssuancePlan,
        certificate_id: CertificateId,
        issued_at: TlsInstant,
    ) -> Result<CertificateRecord, TlsModelError> {
        let not_after = TlsInstant::from_unix_seconds(
            issued_at.as_unix_seconds() + certificate_lifetime_seconds(),
        );
        self.issue_record(
            certificate_id,
            &plan.bindings,
            plan.state_store,
            plan.cloudflare_mode,
            issued_at,
            not_after,
            format!(
                "issue:{}:{:?}:{:?}",
                plan.provider, plan.challenge, plan.account_secret
            ),
        )
    }

    fn renew_certificate(
        &self,
        plan: &RenewalPlan,
        certificate_id: CertificateId,
        replacement_certificate_id: CertificateId,
        issued_at: TlsInstant,
    ) -> Result<CertificateRecord, TlsModelError> {
        let existing = self
            .control_plane
            .inventory()
            .record(&plan.certificate_id)
            .cloned()
            .ok_or_else(|| TlsModelError::UnknownCertificate {
                certificate_id: plan.certificate_id.to_string(),
            })?;
        let not_after = TlsInstant::from_unix_seconds(
            issued_at.as_unix_seconds() + certificate_lifetime_seconds(),
        );
        let replacement_certificate_id_for_log = replacement_certificate_id.clone();
        self.issue_record(
            replacement_certificate_id,
            &existing.bindings,
            existing.store,
            existing.cloudflare_mode,
            issued_at,
            not_after,
            format!(
                "renew:{}:{}:{:?}:{:?}",
                certificate_id, replacement_certificate_id_for_log, plan.provider, plan.challenge
            ),
        )
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

#[derive(Debug, Clone)]
pub struct AcmeTlsCertificateExecutor {
    inner: IssuedCertificateExecutor,
}

impl AcmeTlsCertificateExecutor {
    pub fn new(control_plane: TlsControlPlaneRuntime, protector: TlsMaterialProtector) -> Self {
        Self {
            inner: IssuedCertificateExecutor::new(
                CertificateProviderKind::Acme,
                control_plane,
                protector,
            ),
        }
    }
}

impl TlsCertificateExecutor for AcmeTlsCertificateExecutor {
    fn import_manual_certificate(
        &self,
        bundle: crate::material::ManualCertificateBundle,
    ) -> Result<(), TlsModelError> {
        self.inner.import_manual_certificate(bundle)
    }

    fn issue_certificate(
        &self,
        plan: &IssuancePlan,
        certificate_id: CertificateId,
        issued_at: TlsInstant,
    ) -> Result<CertificateRecord, TlsModelError> {
        self.inner
            .issue_certificate(plan, certificate_id, issued_at)
    }

    fn renew_certificate(
        &self,
        plan: &RenewalPlan,
        certificate_id: CertificateId,
        replacement_certificate_id: CertificateId,
        issued_at: TlsInstant,
    ) -> Result<CertificateRecord, TlsModelError> {
        self.inner
            .renew_certificate(plan, certificate_id, replacement_certificate_id, issued_at)
    }

    fn certificate_material(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateMaterial, TlsModelError> {
        self.inner.certificate_material(certificate_id)
    }
}

#[derive(Debug, Clone)]
pub struct CloudflareTlsCertificateExecutor {
    inner: IssuedCertificateExecutor,
}

impl CloudflareTlsCertificateExecutor {
    pub fn new(
        provider: CertificateProviderKind,
        control_plane: TlsControlPlaneRuntime,
        protector: TlsMaterialProtector,
    ) -> Self {
        Self {
            inner: IssuedCertificateExecutor::new(provider, control_plane, protector),
        }
    }
}

impl TlsCertificateExecutor for CloudflareTlsCertificateExecutor {
    fn import_manual_certificate(
        &self,
        bundle: crate::material::ManualCertificateBundle,
    ) -> Result<(), TlsModelError> {
        self.inner.import_manual_certificate(bundle)
    }

    fn issue_certificate(
        &self,
        plan: &IssuancePlan,
        certificate_id: CertificateId,
        issued_at: TlsInstant,
    ) -> Result<CertificateRecord, TlsModelError> {
        self.inner
            .issue_certificate(plan, certificate_id, issued_at)
    }

    fn renew_certificate(
        &self,
        plan: &RenewalPlan,
        certificate_id: CertificateId,
        replacement_certificate_id: CertificateId,
        issued_at: TlsInstant,
    ) -> Result<CertificateRecord, TlsModelError> {
        self.inner
            .renew_certificate(plan, certificate_id, replacement_certificate_id, issued_at)
    }

    fn certificate_material(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateMaterial, TlsModelError> {
        self.inner.certificate_material(certificate_id)
    }
}

fn issued_certificate_material(
    provider: CertificateProviderKind,
    certificate_id: &str,
    bindings: &[HostnameBinding],
    issued_at: TlsInstant,
    not_after: TlsInstant,
    seed: &str,
    cloudflare_mode: Option<CloudflareEncryptionMode>,
) -> Result<CertificateMaterial, TlsModelError> {
    let hostnames = bindings
        .iter()
        .map(|binding| binding.hostname.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let metadata = format!(
        "provider={provider}\ncertificate_id={certificate_id}\nhostnames={hostnames}\nissued_at={issued_at}\nnot_after={not_after}\nseed={seed}\ncloudflare_mode={:?}\n",
        cloudflare_mode
    );
    let certificate = format!(
        "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
        metadata
    );
    let private_key = format!(
        "-----BEGIN PRIVATE KEY-----\nprovider={provider}\ncertificate_id={certificate_id}\nseed={seed}\n-----END PRIVATE KEY-----\n"
    );
    CertificateMaterial::new(certificate, private_key)
}

fn certificate_fingerprint(
    material: &CertificateMaterial,
) -> Result<CertificateFingerprint, TlsModelError> {
    let digest = sha2::Sha256::digest(material.certificate_chain_pem().as_str().as_bytes());
    CertificateFingerprint::new(format!("sha256:{:x}", digest))
}

fn certificate_lifetime_seconds() -> u64 {
    90 * 24 * 60 * 60
}
