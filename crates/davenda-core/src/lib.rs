use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use davenda_a11y::{NavigationContract, ThemeAccessibilityContract};
use davenda_auth::{AuthModelPackage, Capability};
use davenda_cache::{CachePlanner, CacheTopology, DistributedCacheBackend};
use davenda_cli::CliRuntime;
use davenda_config::{
    CookieConfig as HttpCookieConfig, CookieProtection as ConfigCookieProtection,
    CsrfConfig as HttpCsrfConfig, DistributedCache, PlatformConfig, SameSitePolicy,
    SessionStore as ConfigSessionStore, TlsMode,
};
use davenda_data::{
    DataRuntime, MigrationPlan, PageRequest, PublicationVisibility, QueryCacheScope, RepositorySpec,
};
use davenda_i18n::{
    CurrencyCode, LocaleContext, LocaleRouter, LocaleTag, LocaleUrlConfig, TimeZoneId,
    TranslationCatalog, TranslationRuntime,
};
use davenda_jobs::{JobsRuntime, RetryPolicy};
use davenda_observability::{
    DependencyKind, DependencyStatus, HealthProbeKind, HealthReport, MaintenanceMode,
    ObservabilityRuntime,
};
use davenda_seo::HeadMetadata;
use davenda_template::{TemplateNamespace, TemplateRegistry, TemplateRuntime};
use davenda_tls::TlsRuntime;
use davenda_wasm::{ExtensionPointKind, ResourceLimits};
use hmac::{Hmac, Mac};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

mod bootstrap;
mod browser;
mod manifest;
mod registry;
#[cfg(test)]
mod tests;
mod validation;

pub use bootstrap::*;
pub use browser::*;
pub use manifest::*;
pub use registry::*;
pub use validation::*;
