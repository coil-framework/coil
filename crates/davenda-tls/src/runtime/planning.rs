use std::time::Duration;

use davenda_config::{SecretRef, TlsConfig, TlsMode, TlsProvider};
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
}

impl TlsRuntime {
    pub fn from_config(config: &TlsConfig) -> Self {
        let account_secret_ref = config.account_secret.as_ref().map(SecretRef::redacted);

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
            },
            TlsMode::Acme => {
                let provider = match config.provider {
                    Some(TlsProvider::CloudflareDns) => CertificateProviderKind::CloudflareDns,
                    _ => CertificateProviderKind::Acme,
                };

                Self {
                    mode: config.mode,
                    edge_mode: EdgeMode::DirectTermination,
                    provider: Some(provider),
                    challenge: config.challenge.map(ChallengeStrategy::from),
                    state_store: CertificateStateStore::SharedSecrets,
                    shared_across_nodes: true,
                    requires_trusted_termination_metadata: false,
                    hot_reload_supported: true,
                    cloudflare_mode: None,
                    account_secret_ref,
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
            "mode={:?};edge={:?};provider={:?};challenge={:?};store={:?};shared={};hot_reload={};cloudflare={:?};trusted_termination={};account={}",
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
        )
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
