use super::common::{
    CLOUDFLARE_API_BASE_URL, CLOUDFLARE_ORIGIN_VALIDITY_DAYS, ProviderSecret,
    build_certificate_request, build_record, generate_private_key, private_key_to_pem,
    provider_error,
};
use crate::TlsCertificateExecutor;
use crate::material::{CertificateMaterial, TlsMaterialProtector};
use crate::{
    CertificateId, CertificateProviderKind, CertificateRecord, CloudflareEncryptionMode,
    HostnameBinding, TlsInstant, TlsModelError,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct CloudflareTlsCertificateExecutor {
    provider: CertificateProviderKind,
    control_plane: crate::runtime::TlsControlPlaneRuntime,
    protector: TlsMaterialProtector,
    account_secret_ref: Option<String>,
}

impl CloudflareTlsCertificateExecutor {
    pub fn new(
        provider: CertificateProviderKind,
        control_plane: crate::runtime::TlsControlPlaneRuntime,
        protector: TlsMaterialProtector,
        account_secret_ref: Option<String>,
    ) -> Self {
        Self {
            provider,
            control_plane,
            protector,
            account_secret_ref,
        }
    }
}

#[async_trait::async_trait]
impl TlsCertificateExecutor for CloudflareTlsCertificateExecutor {
    fn import_manual_certificate(
        &self,
        _bundle: crate::material::ManualCertificateBundle,
    ) -> Result<(), TlsModelError> {
        Err(TlsModelError::ManualModeRequiresImportedCertificate)
    }

    fn issue_certificate(
        &self,
        plan: &crate::runtime::planning::IssuancePlan,
        certificate_id: CertificateId,
        issued_at: TlsInstant,
    ) -> Result<CertificateRecord, TlsModelError> {
        match self.provider {
            CertificateProviderKind::CloudflareOriginCa => super::common::run_blocking(
                self.provider,
                "issue_cloudflare_origin_certificate",
                issue_cloudflare_origin_certificate(
                    self.provider,
                    self.protector.clone(),
                    self.account_secret_ref.clone(),
                    plan.bindings.clone(),
                    plan.state_store,
                    plan.cloudflare_mode,
                    certificate_id,
                    issued_at,
                ),
            ),
            CertificateProviderKind::CloudflareDns | CertificateProviderKind::Acme => {
                super::common::run_blocking(
                    self.provider,
                    "issue_acme_certificate",
                    super::acme::issue_acme_certificate(
                        self.provider,
                        self.protector.clone(),
                        self.account_secret_ref.clone(),
                        plan.bindings.clone(),
                        plan.challenge,
                        plan.state_store,
                        plan.cloudflare_mode,
                        certificate_id,
                        issued_at,
                    ),
                )
            }
            CertificateProviderKind::ManualImport => {
                Err(TlsModelError::ManualModeRequiresImportedCertificate)
            }
        }
    }

    fn renew_certificate(
        &self,
        plan: &crate::runtime::planning::RenewalPlan,
        _certificate_id: CertificateId,
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

        match self.provider {
            CertificateProviderKind::CloudflareOriginCa => super::common::run_blocking(
                self.provider,
                "renew_cloudflare_origin_certificate",
                issue_cloudflare_origin_certificate(
                    self.provider,
                    self.protector.clone(),
                    self.account_secret_ref.clone(),
                    existing.bindings,
                    existing.store,
                    existing.cloudflare_mode,
                    replacement_certificate_id,
                    issued_at,
                ),
            ),
            CertificateProviderKind::CloudflareDns | CertificateProviderKind::Acme => {
                super::common::run_blocking(
                    self.provider,
                    "renew_acme_certificate",
                    super::acme::issue_acme_certificate(
                        self.provider,
                        self.protector.clone(),
                        self.account_secret_ref.clone(),
                        existing.bindings,
                        plan.challenge,
                        existing.store,
                        existing.cloudflare_mode,
                        replacement_certificate_id,
                        issued_at,
                    ),
                )
            }
            CertificateProviderKind::ManualImport => {
                Err(TlsModelError::ManualModeRequiresImportedCertificate)
            }
        }
    }

    fn certificate_material(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateMaterial, TlsModelError> {
        super::common::decrypt_material(&self.control_plane, &self.protector, certificate_id)
    }
}

async fn issue_cloudflare_origin_certificate(
    provider: CertificateProviderKind,
    protector: TlsMaterialProtector,
    account_secret_ref: Option<String>,
    bindings: Vec<HostnameBinding>,
    state_store: crate::CertificateStateStore,
    cloudflare_mode: Option<CloudflareEncryptionMode>,
    certificate_id: CertificateId,
    issued_at: TlsInstant,
) -> Result<CertificateRecord, TlsModelError> {
    let secret = ProviderSecret::resolve(provider, account_secret_ref.as_deref())?;
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| provider_error(provider, "build_http_client", error))?;
    let private_key = generate_private_key(provider)?;
    let csr = build_certificate_request(provider, &private_key, &bindings)?;
    let hostnames = bindings
        .iter()
        .map(|binding| binding.hostname.as_str().to_string())
        .collect::<Vec<_>>();
    let response = client
        .post(format!("{CLOUDFLARE_API_BASE_URL}/certificates"))
        .headers(secret.cloudflare_headers()?)
        .json(&CloudflareOriginCaRequest {
            csr,
            hostnames,
            request_type: "origin-rsa".to_string(),
            requested_validity: CLOUDFLARE_ORIGIN_VALIDITY_DAYS,
        })
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| provider_error(provider, "create_origin_certificate", error))?
        .json::<CloudflareEnvelope<CloudflareOriginCaResult>>()
        .map_err(|error| provider_error(provider, "decode_origin_certificate", error))?;

    if !response.success {
        return Err(TlsModelError::ProviderRequestFailed {
            provider: provider.to_string(),
            operation: "create_origin_certificate",
            reason: response
                .errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("; "),
        });
    }

    let not_after = TlsInstant::from_unix_seconds(
        issued_at.as_unix_seconds() + CLOUDFLARE_ORIGIN_VALIDITY_DAYS * 24 * 60 * 60,
    );
    build_record(
        provider,
        certificate_id,
        &bindings,
        state_store,
        cloudflare_mode,
        issued_at,
        not_after,
        response.result.certificate,
        private_key_to_pem(provider, &private_key)?,
        &protector,
    )
}

#[derive(Debug, Serialize)]
struct CloudflareOriginCaRequest {
    csr: String,
    hostnames: Vec<String>,
    request_type: String,
    requested_validity: u64,
}

#[derive(Debug, Deserialize)]
struct CloudflareEnvelope<T> {
    success: bool,
    #[serde(default)]
    errors: Vec<CloudflareApiError>,
    result: T,
}

#[derive(Debug, Deserialize)]
struct CloudflareApiError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct CloudflareOriginCaResult {
    certificate: String,
}
