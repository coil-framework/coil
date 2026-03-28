use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use coil_a11y::{NavigationContract, ThemeAccessibilityContract};
use coil_auth::{AuthModelPackage, Capability};
use coil_cache::{CachePlanner, CacheTopology, DistributedCacheBackend};
use coil_config::{
    CookieConfig as HttpCookieConfig, CookieProtection as ConfigCookieProtection,
    CsrfConfig as HttpCsrfConfig, DistributedCache, PlatformConfig, SameSitePolicy,
    SessionStore as ConfigSessionStore, TlsMode,
};
use coil_data::{
    DataRuntime, MigrationPlan, PageRequest, PublicationVisibility, QueryCacheScope, RepositorySpec,
};
use coil_i18n::{
    CurrencyCode, LocaleContext, LocaleRouter, LocaleTag, LocaleUrlConfig, TimeZoneId,
    TranslationCatalog, TranslationRuntime,
};
use coil_jobs::{JobsRuntime, RetryPolicy};
use coil_observability::{
    DependencyKind, DependencyStatus, HealthProbeKind, HealthReport, MaintenanceMode,
    ObservabilityRuntime,
};
use coil_seo::HeadMetadata;
use coil_template::{TemplateNamespace, TemplateRegistry, TemplateRuntime};
use coil_tls::TlsRuntime;
use coil_wasm::{ExtensionPointKind, ResourceLimits};
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
