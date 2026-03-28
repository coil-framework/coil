use std::time::Duration;

use coil_config::{AcmeChallenge, SecretRef, TlsConfig, TlsMode, TlsProvider};
use serde::{Deserialize, Serialize};

use crate::{
    CertificateId, CertificateProviderKind, CertificateRecord, CertificateStateStore,
    CertificateStatus, ChallengeStrategy, CloudflareEncryptionMode, EdgeMode, HostnameBinding,
    ManualCertificateBundle, TlsInstant, TlsModelError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuancePlan {
    pub edge_mode: EdgeMode,
    pub provider: CertificateProviderKind,
    pub challenge: Option<ChallengeStrategy>,
    pub state_store: CertificateStateStore,
    pub bindings: Vec<HostnameBinding>,
    pub shared_across_nodes: bool,
    pub requires_hot_reload: bool,
    pub account_secret: Option<String>,
    pub cloudflare_mode: Option<CloudflareEncryptionMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenewalPlan {
    pub certificate_id: CertificateId,
    pub provider: CertificateProviderKind,
    pub challenge: Option<ChallengeStrategy>,
    pub renew_after: TlsInstant,
    pub must_complete_by: TlsInstant,
    pub retry_interval: Duration,
    pub keep_serving_current_certificate: bool,
    pub requires_hot_reload: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeTicket {
    pub certificate_id: CertificateId,
    pub replacement_certificate_id: Option<CertificateId>,
    pub provider: CertificateProviderKind,
    pub challenge: Option<ChallengeStrategy>,
    pub bindings: Vec<HostnameBinding>,
    pub account_secret_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotReloadEvent {
    pub certificate_id: CertificateId,
    pub bindings: Vec<HostnameBinding>,
    pub reloaded_without_restart: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsRuntime {
    pub mode: TlsMode,
    pub edge_mode: EdgeMode,
    pub provider: Option<CertificateProviderKind>,
    pub challenge: Option<ChallengeStrategy>,
    pub state_store: CertificateStateStore,
    pub shared_across_nodes: bool,
    pub requires_trusted_termination_metadata: bool,
    pub hot_reload_supported: bool,
    pub cloudflare_mode: Option<CloudflareEncryptionMode>,
    pub account_secret_ref: Option<String>,
    configuration_error: Option<TlsModelError>,
}

impl TlsRuntime {
    pub fn from_config(config: &TlsConfig) -> Self {
        let account_secret_ref = config.account_secret.as_ref().map(SecretRef::redacted);
        let requested_challenge = config.challenge.map(ChallengeStrategy::from);

        match config.mode {
            TlsMode::External => Self {
                mode: config.mode,
                edge_mode: EdgeMode::ExternalTermination,
                provider: None,
                challenge: None,
                state_store: CertificateStateStore::ExternalTermination,
                shared_across_nodes: true,
                requires_trusted_termination_metadata: true,
                hot_reload_supported: false,
                cloudflare_mode: None,
                account_secret_ref,
                configuration_error: external_configuration_error(config),
            },
            TlsMode::Acme => {
                let provider = match config.provider {
                    Some(TlsProvider::CloudflareDns) => CertificateProviderKind::CloudflareDns,
                    _ => CertificateProviderKind::Acme,
                };
                let challenge = match (config.provider, requested_challenge) {
                    (Some(TlsProvider::CloudflareDns), None) => Some(ChallengeStrategy::Dns01),
                    _ => requested_challenge,
                };

                Self {
                    mode: config.mode,
                    edge_mode: EdgeMode::DirectTermination,
                    provider: Some(provider),
                    challenge,
                    state_store: CertificateStateStore::SharedSecrets,
                    shared_across_nodes: challenge_supports_shared_issuance(challenge),
                    requires_trusted_termination_metadata: false,
                    hot_reload_supported: true,
                    cloudflare_mode: None,
                    account_secret_ref,
                    configuration_error: acme_configuration_error(config, challenge),
                }
            }
            TlsMode::CloudflareOrigin => Self {
                mode: config.mode,
                edge_mode: EdgeMode::CloudflareOriginOnly,
                provider: Some(CertificateProviderKind::CloudflareOriginCa),
                challenge: None,
                state_store: CertificateStateStore::SharedSecrets,
                shared_across_nodes: true,
                requires_trusted_termination_metadata: false,
                hot_reload_supported: true,
                cloudflare_mode: Some(CloudflareEncryptionMode::FullStrict),
                account_secret_ref,
                configuration_error: cloudflare_origin_configuration_error(config),
            },
            TlsMode::Manual => Self {
                mode: config.mode,
                edge_mode: EdgeMode::DirectTermination,
                provider: Some(CertificateProviderKind::ManualImport),
                challenge: None,
                state_store: CertificateStateStore::OperatorManaged,
                shared_across_nodes: true,
                requires_trusted_termination_metadata: false,
                hot_reload_supported: true,
                cloudflare_mode: None,
                account_secret_ref,
                configuration_error: manual_configuration_error(config),
            },
        }
    }

    pub fn planner(&self) -> TlsPlanner {
        TlsPlanner {
            runtime: self.clone(),
        }
    }

    pub fn control_plane_scope(&self) -> String {
        format!(
            "mode={:?};edge={:?};provider={:?};challenge={:?};store={:?};shared={};hot_reload={};cloudflare={:?};trusted_termination={};account={};config_error={}",
            self.mode,
            self.edge_mode,
            self.provider,
            self.challenge,
            self.state_store,
            self.shared_across_nodes,
            self.hot_reload_supported,
            self.cloudflare_mode,
            self.requires_trusted_termination_metadata,
            self.account_secret_ref.as_deref().unwrap_or("none"),
            self.configuration_error.is_some(),
        )
    }

    fn ensure_valid_configuration(&self) -> Result<(), TlsModelError> {
        match &self.configuration_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    #[cfg(test)]
    pub fn control_plane(&self) -> super::control_plane::TlsControlPlaneRuntime {
        super::control_plane::TlsControlPlaneRuntime::new(self.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsPlanner {
    runtime: TlsRuntime,
}

impl TlsPlanner {
    pub fn issue_for_bindings(
        &self,
        bindings: Vec<HostnameBinding>,
    ) -> Result<IssuancePlan, TlsModelError> {
        self.runtime.ensure_valid_configuration()?;

        if self.runtime.mode == TlsMode::External {
            return Err(TlsModelError::ExternalTerminationDoesNotIssue);
        }

        if self.runtime.mode == TlsMode::Manual {
            return Err(TlsModelError::ManualModeRequiresImportedCertificate);
        }

        if bindings.iter().any(HostnameBinding::is_wildcard)
            && self
                .runtime
                .challenge
                .is_none_or(|challenge| !challenge.supports_wildcards())
        {
            return Err(TlsModelError::WildcardRequiresDns01);
        }

        Ok(IssuancePlan {
            edge_mode: self.runtime.edge_mode,
            provider: self.runtime.provider.unwrap(),
            challenge: self.runtime.challenge,
            state_store: self.runtime.state_store,
            bindings,
            shared_across_nodes: self.runtime.shared_across_nodes,
            requires_hot_reload: self.runtime.hot_reload_supported,
            account_secret: self.runtime.account_secret_ref.clone(),
            cloudflare_mode: self.runtime.cloudflare_mode,
        })
    }

    pub fn import_manual_certificate(
        &self,
        bundle: ManualCertificateBundle,
    ) -> Result<ManualCertificateBundle, TlsModelError> {
        self.runtime.ensure_valid_configuration()?;

        if self.runtime.mode != TlsMode::Manual {
            return Err(TlsModelError::ManualModeRequiresImportedCertificate);
        }

        if bundle.record.provider != CertificateProviderKind::ManualImport {
            return Err(TlsModelError::InvalidCertificateMaterial {
                field: "certificate_provider",
                reason: "manual imports require provider=manual_import".to_string(),
            });
        }

        Ok(bundle)
    }

    pub fn renewal_plan(
        &self,
        record: &CertificateRecord,
        now: TlsInstant,
    ) -> Result<RenewalPlan, TlsModelError> {
        self.runtime.ensure_valid_configuration()?;

        if !matches!(
            record.status,
            CertificateStatus::Active | CertificateStatus::RenewalDue
        ) {
            return Err(TlsModelError::CertificateNotActive {
                certificate_id: record.id.to_string(),
            });
        }

        if record.replacing_certificate.is_some() {
            return Err(TlsModelError::RenewalAlreadyInProgress {
                certificate_id: record.id.to_string(),
            });
        }

        let window = record.renewal_window();
        Ok(RenewalPlan {
            certificate_id: record.id.clone(),
            provider: record.provider,
            challenge: self.runtime.challenge,
            renew_after: if now > window.starts_at {
                now
            } else {
                window.starts_at
            },
            must_complete_by: window.must_complete_by,
            retry_interval: window.retry_interval,
            keep_serving_current_certificate: true,
            requires_hot_reload: self.runtime.hot_reload_supported,
        })
    }
}

fn challenge_supports_shared_issuance(challenge: Option<ChallengeStrategy>) -> bool {
    !matches!(
        challenge,
        Some(ChallengeStrategy::Http01 | ChallengeStrategy::TlsAlpn01)
    )
}

fn external_configuration_error(config: &TlsConfig) -> Option<TlsModelError> {
    config.provider.map(|provider| {
        invalid_configuration(
            "tls.provider",
            format!(
                "`{}` requires built-in certificate automation, but tls.mode=external disables issuance",
                tls_provider_name(provider)
            ),
        )
    }).or_else(|| {
        config.challenge.map(|challenge| {
            invalid_configuration(
                "tls.challenge",
                format!(
                    "`{}` cannot be configured when tls.mode=external",
                    acme_challenge_name(challenge)
                ),
            )
        })
    })
}

fn acme_configuration_error(
    config: &TlsConfig,
    challenge: Option<ChallengeStrategy>,
) -> Option<TlsModelError> {
    match config.provider {
        Some(TlsProvider::CloudflareOriginCa) => Some(invalid_configuration(
            "tls.provider",
            "`cloudflare-origin-ca` requires tls.mode=cloudflare_origin".to_string(),
        )),
        Some(TlsProvider::ManualImport) => Some(invalid_configuration(
            "tls.provider",
            "`manual-import` requires tls.mode=manual".to_string(),
        )),
        Some(TlsProvider::CloudflareDns) => match challenge {
            Some(ChallengeStrategy::Dns01) => None,
            Some(challenge) => Some(TlsModelError::UnsupportedProviderChallenge {
                provider: CertificateProviderKind::CloudflareDns.to_string(),
                challenge: challenge.to_string(),
            }),
            None => Some(TlsModelError::UnsupportedProviderChallenge {
                provider: CertificateProviderKind::CloudflareDns.to_string(),
                challenge: "none".to_string(),
            }),
        },
        None => None,
    }
}

fn cloudflare_origin_configuration_error(config: &TlsConfig) -> Option<TlsModelError> {
    match config.provider {
        Some(TlsProvider::CloudflareDns) => Some(invalid_configuration(
            "tls.provider",
            "`cloudflare-dns` requires tls.mode=acme".to_string(),
        )),
        Some(TlsProvider::ManualImport) => Some(invalid_configuration(
            "tls.provider",
            "`manual-import` requires tls.mode=manual".to_string(),
        )),
        _ => config.challenge.map(|challenge| {
            invalid_configuration(
                "tls.challenge",
                format!(
                    "`{}` cannot be configured when tls.mode=cloudflare_origin",
                    acme_challenge_name(challenge)
                ),
            )
        }),
    }
}

fn manual_configuration_error(config: &TlsConfig) -> Option<TlsModelError> {
    match config.provider {
        Some(TlsProvider::CloudflareDns) => Some(invalid_configuration(
            "tls.provider",
            "`cloudflare-dns` requires tls.mode=acme".to_string(),
        )),
        Some(TlsProvider::CloudflareOriginCa) => Some(invalid_configuration(
            "tls.provider",
            "`cloudflare-origin-ca` requires tls.mode=cloudflare_origin".to_string(),
        )),
        _ => config.challenge.map(|challenge| {
            invalid_configuration(
                "tls.challenge",
                format!(
                    "`{}` cannot be configured when tls.mode=manual",
                    acme_challenge_name(challenge)
                ),
            )
        }),
    }
}

fn invalid_configuration(field: &'static str, reason: String) -> TlsModelError {
    TlsModelError::InvalidConfiguration { field, reason }
}

fn tls_provider_name(provider: TlsProvider) -> &'static str {
    match provider {
        TlsProvider::CloudflareDns => "cloudflare-dns",
        TlsProvider::CloudflareOriginCa => "cloudflare-origin-ca",
        TlsProvider::ManualImport => "manual-import",
    }
}

fn acme_challenge_name(challenge: AcmeChallenge) -> &'static str {
    match challenge {
        AcmeChallenge::Http01 => "http-01",
        AcmeChallenge::TlsAlpn01 => "tls-alpn-01",
        AcmeChallenge::Dns01 => "dns-01",
    }
}
