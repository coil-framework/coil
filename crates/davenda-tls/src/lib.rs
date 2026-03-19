use std::fmt;
use std::time::Duration;

use davenda_config::{AcmeChallenge, SecretRef, TlsConfig, TlsMode, TlsProvider};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TlsModelError {
    #[error("`{field}` cannot be empty")]
    EmptyField { field: &'static str },
    #[error("`{field}` contains an invalid token `{value}`")]
    InvalidToken { field: &'static str, value: String },
    #[error("external termination does not issue certificates")]
    ExternalTerminationDoesNotIssue,
    #[error("manual mode requires an imported certificate inventory entry")]
    ManualModeRequiresImportedCertificate,
    #[error("wildcard hostnames require dns-01 validation")]
    WildcardRequiresDns01,
    #[error("certificate `{certificate_id}` is not currently active")]
    CertificateNotActive { certificate_id: String },
    #[error(
        "certificate `{certificate_id}` cannot be renewed because it is already replacing itself"
    )]
    RenewalAlreadyInProgress { certificate_id: String },
    #[error("certificate `{certificate_id}` is not known to the TLS inventory")]
    UnknownCertificate { certificate_id: String },
    #[error("hostname `{hostname}` is already bound to active certificate `{certificate_id}`")]
    DuplicateHostnameBinding {
        hostname: String,
        certificate_id: String,
    },
    #[error(
        "certificate `{certificate_id}` cannot be renewed until `{renew_after}`, current time is `{now}`"
    )]
    RenewalNotDue {
        certificate_id: String,
        renew_after: TlsInstant,
        now: TlsInstant,
    },
    #[error("certificate `{certificate_id}` has no pending replacement")]
    MissingReplacementCertificate { certificate_id: String },
}

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, TlsModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(CertificateId, "certificate_id");
token_type!(Hostname, "hostname");
token_type!(CustomerAppId, "customer_app_id");
token_type!(CertificateFingerprint, "certificate_fingerprint");

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecretMaterialRef(String);

impl SecretMaterialRef {
    pub fn new(value: impl Into<String>) -> Result<Self, TlsModelError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(TlsModelError::EmptyField {
                field: "secret_material_ref",
            });
        }

        if trimmed.chars().any(|ch| ch.is_whitespace() || ch == '\0') {
            return Err(TlsModelError::InvalidToken {
                field: "secret_material_ref",
                value: trimmed.to_string(),
            });
        }

        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SecretMaterialRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TlsInstant(u64);

impl TlsInstant {
    pub const fn from_unix_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub const fn as_unix_seconds(self) -> u64 {
        self.0
    }

    pub fn saturating_sub(self, duration: Duration) -> Self {
        Self(self.0.saturating_sub(duration.as_secs()))
    }
}

impl fmt::Display for TlsInstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeStrategy {
    Http01,
    TlsAlpn01,
    Dns01,
}

impl ChallengeStrategy {
    pub fn supports_wildcards(self) -> bool {
        matches!(self, Self::Dns01)
    }
}

impl From<AcmeChallenge> for ChallengeStrategy {
    fn from(value: AcmeChallenge) -> Self {
        match value {
            AcmeChallenge::Http01 => Self::Http01,
            AcmeChallenge::TlsAlpn01 => Self::TlsAlpn01,
            AcmeChallenge::Dns01 => Self::Dns01,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeMode {
    DirectTermination,
    ExternalTermination,
    CloudflareOriginOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateProviderKind {
    Acme,
    CloudflareDns,
    CloudflareOriginCa,
    ManualImport,
}

impl fmt::Display for CertificateProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acme => f.write_str("acme"),
            Self::CloudflareDns => f.write_str("cloudflare_dns"),
            Self::CloudflareOriginCa => f.write_str("cloudflare_origin_ca"),
            Self::ManualImport => f.write_str("manual_import"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateStateStore {
    SharedSecrets,
    ExternalTermination,
    OperatorManaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudflareEncryptionMode {
    FullStrict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateStatus {
    PendingIssuance,
    Active,
    RenewalDue,
    Renewing,
    Failed,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenewalWindow {
    pub starts_at: TlsInstant,
    pub must_complete_by: TlsInstant,
    pub retry_interval: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostnameBinding {
    pub hostname: Hostname,
    pub customer_app: CustomerAppId,
    pub sni_enabled: bool,
}

impl HostnameBinding {
    pub fn new(hostname: Hostname, customer_app: CustomerAppId) -> Self {
        Self {
            hostname,
            customer_app,
            sni_enabled: true,
        }
    }

    pub fn is_wildcard(&self) -> bool {
        self.hostname.as_str().starts_with("*.")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateRecord {
    pub id: CertificateId,
    pub provider: CertificateProviderKind,
    pub status: CertificateStatus,
    pub fingerprint: CertificateFingerprint,
    pub issued_at: TlsInstant,
    pub not_after: TlsInstant,
    pub material_ref: SecretMaterialRef,
    pub bindings: Vec<HostnameBinding>,
    pub store: CertificateStateStore,
    pub cloudflare_mode: Option<CloudflareEncryptionMode>,
    pub replacing_certificate: Option<CertificateId>,
}

impl CertificateRecord {
    pub fn new(
        id: CertificateId,
        provider: CertificateProviderKind,
        status: CertificateStatus,
        fingerprint: CertificateFingerprint,
        issued_at: TlsInstant,
        not_after: TlsInstant,
        material_ref: SecretMaterialRef,
        store: CertificateStateStore,
    ) -> Self {
        Self {
            id,
            provider,
            status,
            fingerprint,
            issued_at,
            not_after,
            material_ref,
            bindings: Vec::new(),
            store,
            cloudflare_mode: None,
            replacing_certificate: None,
        }
    }

    pub fn with_binding(mut self, binding: HostnameBinding) -> Self {
        self.bindings.push(binding);
        self
    }

    pub fn with_cloudflare_mode(mut self, mode: CloudflareEncryptionMode) -> Self {
        self.cloudflare_mode = Some(mode);
        self
    }

    pub fn renewal_window(&self) -> RenewalWindow {
        RenewalWindow {
            starts_at: self
                .not_after
                .saturating_sub(Duration::from_secs(30 * 24 * 60 * 60)),
            must_complete_by: self.not_after,
            retry_interval: Duration::from_secs(6 * 60 * 60),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeTicket {
    pub certificate_id: CertificateId,
    pub replacement_certificate_id: Option<CertificateId>,
    pub provider: CertificateProviderKind,
    pub challenge: Option<ChallengeStrategy>,
    pub bindings: Vec<HostnameBinding>,
    pub account_secret_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotReloadEvent {
    pub certificate_id: CertificateId,
    pub bindings: Vec<HostnameBinding>,
    pub reloaded_without_restart: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CertificateInventory {
    certificates: Vec<CertificateRecord>,
}

impl CertificateInventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn certificates(&self) -> &[CertificateRecord] {
        &self.certificates
    }

    pub fn active_for_hostname(&self, hostname: &Hostname) -> Option<&CertificateRecord> {
        self.certificates.iter().find(|record| {
            record
                .bindings
                .iter()
                .any(|binding| &binding.hostname == hostname)
                && matches!(
                    record.status,
                    CertificateStatus::Active
                        | CertificateStatus::RenewalDue
                        | CertificateStatus::Renewing
                )
        })
    }

    pub fn record(&self, certificate_id: &CertificateId) -> Option<&CertificateRecord> {
        self.certificates
            .iter()
            .find(|record| &record.id == certificate_id)
    }

    pub fn record_mut(&mut self, certificate_id: &CertificateId) -> Option<&mut CertificateRecord> {
        self.certificates
            .iter_mut()
            .find(|record| &record.id == certificate_id)
    }

    pub fn insert(&mut self, record: CertificateRecord) -> Result<(), TlsModelError> {
        self.ensure_unique_bindings(&record, None)?;
        self.certificates.push(record);
        Ok(())
    }

    pub fn activate_replacement(
        &mut self,
        certificate_id: &CertificateId,
        replacement: CertificateRecord,
    ) -> Result<(), TlsModelError> {
        let original =
            self.record(certificate_id)
                .ok_or_else(|| TlsModelError::UnknownCertificate {
                    certificate_id: certificate_id.to_string(),
                })?;
        if original.replacing_certificate.as_ref() != Some(&replacement.id) {
            return Err(TlsModelError::MissingReplacementCertificate {
                certificate_id: certificate_id.to_string(),
            });
        }

        self.ensure_unique_bindings(&replacement, Some(certificate_id))?;
        let original =
            self.record_mut(certificate_id)
                .ok_or_else(|| TlsModelError::UnknownCertificate {
                    certificate_id: certificate_id.to_string(),
                })?;
        original.status = CertificateStatus::Superseded;
        original.replacing_certificate = None;

        self.certificates.push(replacement);
        Ok(())
    }

    fn ensure_unique_bindings(
        &self,
        candidate: &CertificateRecord,
        allowing_replaced_certificate: Option<&CertificateId>,
    ) -> Result<(), TlsModelError> {
        for binding in &candidate.bindings {
            if let Some(existing) = self.active_for_hostname(&binding.hostname) {
                let allowed = allowing_replaced_certificate
                    .is_some_and(|certificate_id| &existing.id == certificate_id);
                if !allowed {
                    return Err(TlsModelError::DuplicateHostnameBinding {
                        hostname: binding.hostname.to_string(),
                        certificate_id: existing.id.to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsAutomationRuntime {
    runtime: TlsRuntime,
    inventory: CertificateInventory,
    renewal_queue: Vec<RenewalPlan>,
    pending_challenges: Vec<ChallengeTicket>,
    hot_reload_events: Vec<HotReloadEvent>,
}

impl TlsAutomationRuntime {
    pub fn new(runtime: TlsRuntime) -> Self {
        Self {
            runtime,
            inventory: CertificateInventory::new(),
            renewal_queue: Vec::new(),
            pending_challenges: Vec::new(),
            hot_reload_events: Vec::new(),
        }
    }

    pub fn inventory(&self) -> &CertificateInventory {
        &self.inventory
    }

    pub fn renewal_queue(&self) -> &[RenewalPlan] {
        &self.renewal_queue
    }

    pub fn pending_challenges(&self) -> &[ChallengeTicket] {
        &self.pending_challenges
    }

    pub fn hot_reload_events(&self) -> &[HotReloadEvent] {
        &self.hot_reload_events
    }

    pub fn import_certificate(&mut self, record: CertificateRecord) -> Result<(), TlsModelError> {
        self.inventory.insert(record)
    }

    pub fn queue_renewal(
        &mut self,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, TlsModelError> {
        let record = self
            .inventory
            .record(certificate_id)
            .cloned()
            .ok_or_else(|| TlsModelError::UnknownCertificate {
                certificate_id: certificate_id.to_string(),
            })?;
        let plan = self.runtime.planner().renewal_plan(&record, now)?;
        if plan.renew_after > now {
            return Err(TlsModelError::RenewalNotDue {
                certificate_id: certificate_id.to_string(),
                renew_after: plan.renew_after,
                now,
            });
        }

        if let Some(existing) = self
            .renewal_queue
            .iter()
            .find(|plan| plan.certificate_id == *certificate_id)
        {
            return Err(TlsModelError::RenewalAlreadyInProgress {
                certificate_id: existing.certificate_id.to_string(),
            });
        }

        if let Some(record) = self.inventory.record_mut(certificate_id) {
            record.status = CertificateStatus::RenewalDue;
        }
        self.renewal_queue.push(plan.clone());
        Ok(plan)
    }

    pub fn begin_renewal(
        &mut self,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, TlsModelError> {
        let record = self.inventory.record_mut(certificate_id).ok_or_else(|| {
            TlsModelError::UnknownCertificate {
                certificate_id: certificate_id.to_string(),
            }
        })?;
        if record.replacing_certificate.is_some() {
            return Err(TlsModelError::RenewalAlreadyInProgress {
                certificate_id: certificate_id.to_string(),
            });
        }

        record.status = CertificateStatus::Renewing;
        record.replacing_certificate = Some(replacement_certificate_id.clone());

        let ticket = ChallengeTicket {
            certificate_id: certificate_id.clone(),
            replacement_certificate_id: Some(replacement_certificate_id),
            provider: record.provider,
            challenge: self.runtime.challenge,
            bindings: record.bindings.clone(),
            account_secret_ref: self.runtime.account_secret_ref.clone(),
        };
        self.pending_challenges.push(ticket.clone());
        Ok(ticket)
    }

    pub fn fail_renewal(
        &mut self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, TlsModelError> {
        let record = self.inventory.record_mut(certificate_id).ok_or_else(|| {
            TlsModelError::UnknownCertificate {
                certificate_id: certificate_id.to_string(),
            }
        })?;
        record.status = CertificateStatus::RenewalDue;
        record.replacing_certificate = None;
        self.pending_challenges
            .retain(|ticket| &ticket.certificate_id != certificate_id);
        self.renewal_queue
            .retain(|plan| &plan.certificate_id != certificate_id);
        Ok(record.clone())
    }

    pub fn activate_replacement(
        &mut self,
        certificate_id: &CertificateId,
        mut replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, TlsModelError> {
        replacement.status = CertificateStatus::Active;
        replacement.replacing_certificate = None;

        self.inventory
            .activate_replacement(certificate_id, replacement.clone())?;
        self.pending_challenges
            .retain(|ticket| &ticket.certificate_id != certificate_id);
        self.renewal_queue
            .retain(|plan| &plan.certificate_id != certificate_id);

        let event = HotReloadEvent {
            certificate_id: replacement.id.clone(),
            bindings: replacement.bindings.clone(),
            reloaded_without_restart: self.runtime.hot_reload_supported,
        };
        self.hot_reload_events.push(event.clone());
        Ok(event)
    }
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

    pub fn automation(&self) -> TlsAutomationRuntime {
        TlsAutomationRuntime::new(self.clone())
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
        record: CertificateRecord,
    ) -> Result<CertificateRecord, TlsModelError> {
        if self.runtime.mode != TlsMode::Manual {
            return Err(TlsModelError::ManualModeRequiresImportedCertificate);
        }

        Ok(record)
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

fn validate_token(field: &'static str, value: String) -> Result<String, TlsModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(TlsModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '*'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(TlsModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acme_config(challenge: AcmeChallenge, provider: Option<TlsProvider>) -> TlsConfig {
        TlsConfig {
            mode: TlsMode::Acme,
            challenge: Some(challenge),
            provider,
            account_secret: Some(SecretRef::Env {
                var: "TLS_ACCOUNT_KEY".to_string(),
            }),
        }
    }

    #[test]
    fn acme_dns_runtime_prefers_shared_state_and_hot_reload() {
        let runtime = TlsRuntime::from_config(&acme_config(
            AcmeChallenge::Dns01,
            Some(TlsProvider::CloudflareDns),
        ));
        let plan = runtime
            .planner()
            .issue_for_bindings(vec![
                HostnameBinding::new(
                    Hostname::new("*.example.com").unwrap(),
                    CustomerAppId::new("storefront").unwrap(),
                ),
                HostnameBinding::new(
                    Hostname::new("www.example.com").unwrap(),
                    CustomerAppId::new("storefront").unwrap(),
                ),
            ])
            .unwrap();

        assert_eq!(
            runtime.provider,
            Some(CertificateProviderKind::CloudflareDns)
        );
        assert_eq!(plan.challenge, Some(ChallengeStrategy::Dns01));
        assert_eq!(plan.state_store, CertificateStateStore::SharedSecrets);
        assert!(plan.shared_across_nodes);
        assert!(plan.requires_hot_reload);
        assert_eq!(plan.account_secret.as_deref(), Some("env:TLS_ACCOUNT_KEY"));
    }

    #[test]
    fn cloudflare_origin_runtime_forces_full_strict_origin_behavior() {
        let runtime = TlsRuntime::from_config(&TlsConfig {
            mode: TlsMode::CloudflareOrigin,
            challenge: None,
            provider: Some(TlsProvider::CloudflareOriginCa),
            account_secret: None,
        });
        let plan = runtime
            .planner()
            .issue_for_bindings(vec![HostnameBinding::new(
                Hostname::new("origin.example.com").unwrap(),
                CustomerAppId::new("storefront").unwrap(),
            )])
            .unwrap();

        assert_eq!(runtime.edge_mode, EdgeMode::CloudflareOriginOnly);
        assert_eq!(plan.provider, CertificateProviderKind::CloudflareOriginCa);
        assert_eq!(
            plan.cloudflare_mode,
            Some(CloudflareEncryptionMode::FullStrict)
        );
        assert!(plan.requires_hot_reload);
    }

    #[test]
    fn external_termination_uses_trusted_metadata_and_does_not_issue_certificates() {
        let runtime = TlsRuntime::from_config(&TlsConfig {
            mode: TlsMode::External,
            challenge: None,
            provider: None,
            account_secret: None,
        });

        assert!(runtime.requires_trusted_termination_metadata);
        assert_eq!(
            runtime.state_store,
            CertificateStateStore::ExternalTermination
        );
        assert_eq!(
            runtime
                .planner()
                .issue_for_bindings(vec![HostnameBinding::new(
                    Hostname::new("www.example.com").unwrap(),
                    CustomerAppId::new("storefront").unwrap(),
                )])
                .unwrap_err(),
            TlsModelError::ExternalTerminationDoesNotIssue
        );
    }

    #[test]
    fn renewal_keeps_current_certificate_live_until_replacement_succeeds() {
        let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Dns01, None));
        let record = CertificateRecord::new(
            CertificateId::new("cert-1").unwrap(),
            CertificateProviderKind::Acme,
            CertificateStatus::Active,
            CertificateFingerprint::new("sha256:abc123").unwrap(),
            TlsInstant::from_unix_seconds(1_000),
            TlsInstant::from_unix_seconds(4_000_000),
            SecretMaterialRef::new("secrets/tls/cert-1").unwrap(),
            CertificateStateStore::SharedSecrets,
        )
        .with_binding(HostnameBinding::new(
            Hostname::new("www.example.com").unwrap(),
            CustomerAppId::new("storefront").unwrap(),
        ));

        let renewal = runtime
            .planner()
            .renewal_plan(&record, TlsInstant::from_unix_seconds(3_900_000))
            .unwrap();
        assert_eq!(renewal.certificate_id.as_str(), "cert-1");
        assert!(renewal.keep_serving_current_certificate);
        assert!(renewal.requires_hot_reload);
        assert_eq!(renewal.challenge, Some(ChallengeStrategy::Dns01));
    }

    #[test]
    fn wildcard_bindings_are_rejected_without_dns_validation() {
        let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Http01, None));
        let error = runtime
            .planner()
            .issue_for_bindings(vec![HostnameBinding::new(
                Hostname::new("*.example.com").unwrap(),
                CustomerAppId::new("storefront").unwrap(),
            )])
            .unwrap_err();

        assert_eq!(error, TlsModelError::WildcardRequiresDns01);
    }

    #[test]
    fn inventory_rejects_duplicate_active_hostname_bindings() {
        let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Dns01, None));
        let mut automation = runtime.automation();
        let binding = HostnameBinding::new(
            Hostname::new("www.example.com").unwrap(),
            CustomerAppId::new("storefront").unwrap(),
        );

        automation
            .import_certificate(
                CertificateRecord::new(
                    CertificateId::new("cert-1").unwrap(),
                    CertificateProviderKind::Acme,
                    CertificateStatus::Active,
                    CertificateFingerprint::new("sha256:abc123").unwrap(),
                    TlsInstant::from_unix_seconds(1_000),
                    TlsInstant::from_unix_seconds(4_000_000),
                    SecretMaterialRef::new("secrets/tls/cert-1").unwrap(),
                    CertificateStateStore::SharedSecrets,
                )
                .with_binding(binding.clone()),
            )
            .unwrap();

        let error = automation
            .import_certificate(
                CertificateRecord::new(
                    CertificateId::new("cert-2").unwrap(),
                    CertificateProviderKind::Acme,
                    CertificateStatus::Active,
                    CertificateFingerprint::new("sha256:def456").unwrap(),
                    TlsInstant::from_unix_seconds(2_000),
                    TlsInstant::from_unix_seconds(4_000_000),
                    SecretMaterialRef::new("secrets/tls/cert-2").unwrap(),
                    CertificateStateStore::SharedSecrets,
                )
                .with_binding(binding),
            )
            .unwrap_err();

        assert_eq!(
            error,
            TlsModelError::DuplicateHostnameBinding {
                hostname: "www.example.com".to_string(),
                certificate_id: "cert-1".to_string(),
            }
        );
    }

    #[test]
    fn renewal_failure_keeps_current_certificate_bound() {
        let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Dns01, None));
        let mut automation = runtime.automation();
        let certificate_id = CertificateId::new("cert-active").unwrap();
        let binding = HostnameBinding::new(
            Hostname::new("www.example.com").unwrap(),
            CustomerAppId::new("storefront").unwrap(),
        );
        automation
            .import_certificate(
                CertificateRecord::new(
                    certificate_id.clone(),
                    CertificateProviderKind::Acme,
                    CertificateStatus::Active,
                    CertificateFingerprint::new("sha256:active").unwrap(),
                    TlsInstant::from_unix_seconds(1_000),
                    TlsInstant::from_unix_seconds(4_000_000),
                    SecretMaterialRef::new("secrets/tls/cert-active").unwrap(),
                    CertificateStateStore::SharedSecrets,
                )
                .with_binding(binding.clone()),
            )
            .unwrap();

        let queued = automation
            .queue_renewal(&certificate_id, TlsInstant::from_unix_seconds(3_900_000))
            .unwrap();
        assert_eq!(queued.certificate_id, certificate_id);

        let challenge = automation
            .begin_renewal(
                &certificate_id,
                CertificateId::new("cert-replacement").unwrap(),
            )
            .unwrap();
        assert_eq!(
            challenge.replacement_certificate_id,
            Some(CertificateId::new("cert-replacement").unwrap())
        );

        let reverted = automation.fail_renewal(&certificate_id).unwrap();
        assert_eq!(reverted.status, CertificateStatus::RenewalDue);
        assert!(reverted.replacing_certificate.is_none());
        assert_eq!(
            automation
                .inventory()
                .active_for_hostname(&Hostname::new("www.example.com").unwrap())
                .unwrap()
                .id,
            certificate_id
        );
    }

    #[test]
    fn activating_replacement_supersedes_old_certificate_and_emits_hot_reload() {
        let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Dns01, None));
        let mut automation = runtime.automation();
        let certificate_id = CertificateId::new("cert-live").unwrap();
        let binding = HostnameBinding::new(
            Hostname::new("shop.example.com").unwrap(),
            CustomerAppId::new("storefront").unwrap(),
        );
        automation
            .import_certificate(
                CertificateRecord::new(
                    certificate_id.clone(),
                    CertificateProviderKind::Acme,
                    CertificateStatus::Active,
                    CertificateFingerprint::new("sha256:live").unwrap(),
                    TlsInstant::from_unix_seconds(1_000),
                    TlsInstant::from_unix_seconds(4_000_000),
                    SecretMaterialRef::new("secrets/tls/cert-live").unwrap(),
                    CertificateStateStore::SharedSecrets,
                )
                .with_binding(binding.clone()),
            )
            .unwrap();
        automation
            .queue_renewal(&certificate_id, TlsInstant::from_unix_seconds(3_900_000))
            .unwrap();
        automation
            .begin_renewal(&certificate_id, CertificateId::new("cert-next").unwrap())
            .unwrap();

        let event = automation
            .activate_replacement(
                &certificate_id,
                CertificateRecord::new(
                    CertificateId::new("cert-next").unwrap(),
                    CertificateProviderKind::Acme,
                    CertificateStatus::PendingIssuance,
                    CertificateFingerprint::new("sha256:next").unwrap(),
                    TlsInstant::from_unix_seconds(3_900_500),
                    TlsInstant::from_unix_seconds(8_000_000),
                    SecretMaterialRef::new("secrets/tls/cert-next").unwrap(),
                    CertificateStateStore::SharedSecrets,
                )
                .with_binding(binding.clone()),
            )
            .unwrap();

        assert_eq!(event.certificate_id.as_str(), "cert-next");
        assert!(event.reloaded_without_restart);
        assert_eq!(
            automation
                .inventory()
                .active_for_hostname(&binding.hostname)
                .unwrap()
                .id
                .as_str(),
            "cert-next"
        );
        assert_eq!(
            automation
                .inventory()
                .record(&certificate_id)
                .unwrap()
                .status,
            CertificateStatus::Superseded
        );
        assert_eq!(automation.hot_reload_events().len(), 1);
    }
}
