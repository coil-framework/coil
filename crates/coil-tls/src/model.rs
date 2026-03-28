use std::fmt;
use std::time::Duration;

use coil_config::AcmeChallenge;
use serde::{Deserialize, Serialize};

use crate::TlsModelError;
use crate::material::EncryptedCertificateMaterial;
use crate::validation::validate_token;

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl fmt::Display for ChallengeStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http01 => f.write_str("http-01"),
            Self::TlsAlpn01 => f.write_str("tls-alpn-01"),
            Self::Dns01 => f.write_str("dns-01"),
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeMode {
    DirectTermination,
    ExternalTermination,
    CloudflareOriginOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CertificateStateStore {
    SharedSecrets,
    ExternalTermination,
    OperatorManaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudflareEncryptionMode {
    FullStrict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateRecord {
    pub id: CertificateId,
    pub provider: CertificateProviderKind,
    pub status: CertificateStatus,
    pub fingerprint: CertificateFingerprint,
    pub issued_at: TlsInstant,
    pub not_after: TlsInstant,
    pub material_ref: SecretMaterialRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<EncryptedCertificateMaterial>,
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
            material: None,
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

    pub fn with_material(mut self, material: EncryptedCertificateMaterial) -> Self {
        self.material = Some(material);
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
