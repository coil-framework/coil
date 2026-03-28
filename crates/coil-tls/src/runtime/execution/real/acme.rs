use super::common::{
    ProviderSecret, build_record, generate_private_key, provider_error, run_blocking,
};
use super::solvers::{FilesystemHttp01Solver, start_tls_alpn_solver};
use crate::TlsCertificateExecutor;
use crate::material::{CertificateMaterial, TlsMaterialProtector};
use crate::runtime::execution::{ChallengeValidation, ChallengeValidationCheck};
use crate::runtime::planning::IssuancePlan;
use crate::{
    CertificateId, CertificateProviderKind, CertificateRecord, CertificateStateStore,
    ChallengeStrategy, CloudflareEncryptionMode, HostnameBinding, TlsInstant, TlsModelError,
};
use lers::Directory;

#[derive(Debug, Clone)]
pub struct AcmeTlsCertificateExecutor {
    control_plane: crate::runtime::TlsControlPlaneRuntime,
    protector: TlsMaterialProtector,
    account_secret_ref: Option<String>,
}

impl AcmeTlsCertificateExecutor {
    pub fn new(
        control_plane: crate::runtime::TlsControlPlaneRuntime,
        protector: TlsMaterialProtector,
        account_secret_ref: Option<String>,
    ) -> Self {
        Self {
            control_plane,
            protector,
            account_secret_ref,
        }
    }
}

#[async_trait::async_trait]
impl TlsCertificateExecutor for AcmeTlsCertificateExecutor {
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
        run_blocking(
            CertificateProviderKind::Acme,
            "issue_acme_certificate",
            issue_acme_certificate(
                CertificateProviderKind::Acme,
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

        run_blocking(
            CertificateProviderKind::Acme,
            "renew_acme_certificate",
            issue_acme_certificate(
                existing.provider,
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

    fn certificate_material(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateMaterial, TlsModelError> {
        super::common::decrypt_material(&self.control_plane, &self.protector, certificate_id)
    }

    fn validate_issuance_plan(
        &self,
        plan: &IssuancePlan,
    ) -> Result<ChallengeValidation, TlsModelError> {
        validate_acme_issuance_plan(
            CertificateProviderKind::Acme,
            self.account_secret_ref.as_deref(),
            plan,
        )
    }
}

pub(crate) async fn issue_acme_certificate(
    provider: CertificateProviderKind,
    protector: TlsMaterialProtector,
    account_secret_ref: Option<String>,
    bindings: Vec<HostnameBinding>,
    challenge: Option<ChallengeStrategy>,
    state_store: CertificateStateStore,
    cloudflare_mode: Option<CloudflareEncryptionMode>,
    certificate_id: CertificateId,
    issued_at: TlsInstant,
) -> Result<CertificateRecord, TlsModelError> {
    let secret = ProviderSecret::resolve(provider, account_secret_ref.as_deref())?;
    let effective_challenge = resolve_challenge(provider, &secret, challenge)?;

    let directory = match effective_challenge {
        ChallengeStrategy::Dns01 => {
            let solver = secret.cloudflare_dns_solver()?;
            Directory::builder(secret.acme_directory_url().to_string())
                .dns01_solver(Box::new(solver))
                .build()
                .await
                .map_err(|error| provider_error(provider, "build_acme_directory", error))?
        }
        ChallengeStrategy::Http01 => {
            let solver =
                FilesystemHttp01Solver::new(secret.http_challenge_directory().ok_or_else(
                    || TlsModelError::MissingProviderCredential {
                        provider: provider.to_string(),
                    },
                )?);
            Directory::builder(secret.acme_directory_url().to_string())
                .http01_solver(Box::new(solver))
                .build()
                .await
                .map_err(|error| provider_error(provider, "build_acme_directory", error))?
        }
        ChallengeStrategy::TlsAlpn01 => {
            let address = secret.tls_alpn_bind_address()?.ok_or_else(|| {
                TlsModelError::MissingProviderCredential {
                    provider: provider.to_string(),
                }
            })?;
            let (solver, handle) = start_tls_alpn_solver(address)
                .await
                .map_err(|error| provider_error(provider, "start_tls_alpn_solver", error))?;
            let directory = Directory::builder(secret.acme_directory_url().to_string())
                .tls_alpn01_solver(Box::new(solver))
                .build()
                .await
                .map_err(|error| provider_error(provider, "build_acme_directory", error))?;
            let certificate = issue_with_directory(
                provider,
                &secret,
                &protector,
                directory,
                bindings,
                state_store,
                cloudflare_mode,
                certificate_id,
                issued_at,
            )
            .await;
            let stop_result = handle.stop().await;
            if certificate.is_ok() {
                stop_result
                    .map_err(|error| provider_error(provider, "stop_tls_alpn_solver", error))?;
            }
            return certificate;
        }
    };

    issue_with_directory(
        provider,
        &secret,
        &protector,
        directory,
        bindings,
        state_store,
        cloudflare_mode,
        certificate_id,
        issued_at,
    )
    .await
}

async fn issue_with_directory(
    provider: CertificateProviderKind,
    secret: &ProviderSecret,
    protector: &TlsMaterialProtector,
    directory: Directory,
    bindings: Vec<HostnameBinding>,
    state_store: CertificateStateStore,
    cloudflare_mode: Option<CloudflareEncryptionMode>,
    certificate_id: CertificateId,
    issued_at: TlsInstant,
) -> Result<CertificateRecord, TlsModelError> {
    let account_key = secret.account_key_pem()?;
    let account = directory
        .account()
        .terms_of_service_agreed(true)
        .private_key(account_key)
        .create_if_not_exists()
        .await
        .map_err(|error| provider_error(provider, "create_acme_account", error))?;

    let certificate_key = generate_private_key(provider)?;
    let mut builder = account.certificate().private_key(certificate_key);
    for binding in &bindings {
        builder = builder.add_domain(binding.hostname.as_str().to_string());
    }

    let certificate = builder
        .obtain()
        .await
        .map_err(|error| provider_error(provider, "obtain_certificate", error))?;

    let chain = String::from_utf8(
        certificate
            .fullchain_to_pem()
            .map_err(|error| provider_error(provider, "encode_certificate_chain", error))?,
    )
    .map_err(|error| provider_error(provider, "encode_certificate_chain", error))?;
    let private_key = String::from_utf8(
        certificate
            .private_key_to_pem()
            .map_err(|error| provider_error(provider, "encode_private_key", error))?,
    )
    .map_err(|error| provider_error(provider, "encode_private_key", error))?;

    build_record(
        provider,
        certificate_id,
        &bindings,
        state_store,
        cloudflare_mode,
        issued_at,
        chain,
        private_key,
        protector,
    )
}

pub(crate) fn validate_acme_issuance_plan(
    provider: CertificateProviderKind,
    account_secret_ref: Option<&str>,
    plan: &IssuancePlan,
) -> Result<ChallengeValidation, TlsModelError> {
    let secret = ProviderSecret::resolve(provider, account_secret_ref)?;
    secret.account_key_pem()?;
    let effective_challenge = resolve_challenge(provider, &secret, plan.challenge)?;
    let mut checks = vec![ChallengeValidationCheck {
        name: "account_key",
        ok: true,
        detail: "account credentials resolved for the active provider".to_string(),
    }];

    match effective_challenge {
        ChallengeStrategy::Dns01 => {
            secret.cloudflare_dns_solver()?;
            checks.push(ChallengeValidationCheck {
                name: "dns_solver",
                ok: true,
                detail: "cloudflare dns-01 solver credentials are present and parse correctly"
                    .to_string(),
            });
        }
        ChallengeStrategy::Http01 => {
            let directory = secret.http_challenge_directory().ok_or_else(|| {
                TlsModelError::MissingProviderCredential {
                    provider: provider.to_string(),
                }
            })?;
            checks.push(ChallengeValidationCheck {
                name: "http_challenge_directory",
                ok: true,
                detail: format!(
                    "http-01 challenge files will be written under `{}`",
                    directory.display()
                ),
            });
        }
        ChallengeStrategy::TlsAlpn01 => {
            let address = secret.tls_alpn_bind_address()?.ok_or_else(|| {
                TlsModelError::MissingProviderCredential {
                    provider: provider.to_string(),
                }
            })?;
            checks.push(ChallengeValidationCheck {
                name: "tls_alpn_bind_address",
                ok: true,
                detail: format!("tls-alpn-01 solver will bind to `{address}`"),
            });
        }
    }

    Ok(ChallengeValidation {
        provider,
        configured_challenge: plan.challenge,
        effective_challenge: Some(effective_challenge),
        shared_across_nodes: plan.shared_across_nodes,
        requires_hot_reload: plan.requires_hot_reload,
        checks,
    })
}

pub(crate) fn resolve_challenge(
    provider: CertificateProviderKind,
    secret: &ProviderSecret,
    requested: Option<ChallengeStrategy>,
) -> Result<ChallengeStrategy, TlsModelError> {
    if provider == CertificateProviderKind::CloudflareDns {
        return match requested {
            Some(ChallengeStrategy::Dns01) | None => {
                if secret.has_cloudflare_dns_credentials() {
                    Ok(ChallengeStrategy::Dns01)
                } else {
                    Err(TlsModelError::MissingProviderCredential {
                        provider: provider.to_string(),
                    })
                }
            }
            Some(challenge) => Err(TlsModelError::UnsupportedProviderChallenge {
                provider: provider.to_string(),
                challenge: challenge.to_string(),
            }),
        };
    }

    if let Some(challenge) = requested {
        return match challenge {
            ChallengeStrategy::Dns01 if secret.has_cloudflare_dns_credentials() => Ok(challenge),
            ChallengeStrategy::Http01 if secret.has_http_challenge_directory() => Ok(challenge),
            ChallengeStrategy::TlsAlpn01 if secret.has_tls_alpn_bind_address() => Ok(challenge),
            _ => Err(TlsModelError::MissingProviderCredential {
                provider: provider.to_string(),
            }),
        };
    }

    if secret.has_cloudflare_dns_credentials() {
        return Ok(ChallengeStrategy::Dns01);
    }

    if secret.has_http_challenge_directory() {
        return Ok(ChallengeStrategy::Http01);
    }

    if secret.has_tls_alpn_bind_address() {
        return Ok(ChallengeStrategy::TlsAlpn01);
    }

    Err(TlsModelError::MissingProviderCredential {
        provider: provider.to_string(),
    })
}
