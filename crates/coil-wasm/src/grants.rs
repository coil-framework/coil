use std::collections::BTreeSet;
use std::fmt;
use std::time::Duration;

use crate::error::WasmModelError;
use crate::ids::ExtensionPointKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StorageClassGrant {
    PublicUpload,
    PrivateShared,
    LocalOnlySensitive,
    PublicAsset,
}

impl fmt::Display for StorageClassGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublicUpload => f.write_str("public_upload"),
            Self::PrivateShared => f.write_str("private_shared"),
            Self::LocalOnlySensitive => f.write_str("local_only_sensitive"),
            Self::PublicAsset => f.write_str("public_asset"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetadataGrant {
    JsonLd,
    SitemapEntry,
    Translation,
    SeoHead,
}

impl fmt::Display for MetadataGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonLd => f.write_str("json_ld"),
            Self::SitemapEntry => f.write_str("sitemap_entry"),
            Self::Translation => f.write_str("translation"),
            Self::SeoHead => f.write_str("seo_head"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostCapabilityGrant {
    DataRead { resource: String },
    DataWrite { resource: String },
    AuthCheck,
    AuthList,
    AuthLookup,
    AuthTupleWrite,
    StorageRead { class: StorageClassGrant },
    StorageWrite { class: StorageClassGrant },
    RenderFragment { slot: String },
    MetadataWrite { kind: MetadataGrant },
    CacheHintWrite,
    OutboundHttp { integration: String },
    SecretRead { secret: String },
    EnqueueJob { queue: String },
}

impl fmt::Display for HostCapabilityGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataRead { resource } => write!(f, "data.read:{resource}"),
            Self::DataWrite { resource } => write!(f, "data.write:{resource}"),
            Self::AuthCheck => f.write_str("auth.check"),
            Self::AuthList => f.write_str("auth.list"),
            Self::AuthLookup => f.write_str("auth.lookup"),
            Self::AuthTupleWrite => f.write_str("auth.tuple_write"),
            Self::StorageRead { class } => write!(f, "storage.read:{class}"),
            Self::StorageWrite { class } => write!(f, "storage.write:{class}"),
            Self::RenderFragment { slot } => write!(f, "render.fragment:{slot}"),
            Self::MetadataWrite { kind } => write!(f, "metadata.write:{kind}"),
            Self::CacheHintWrite => f.write_str("cache.hint.write"),
            Self::OutboundHttp { integration } => write!(f, "http.outbound:{integration}"),
            Self::SecretRead { secret } => write!(f, "secret.read:{secret}"),
            Self::EnqueueJob { queue } => write!(f, "job.enqueue:{queue}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HostGrantSet {
    grants: BTreeSet<HostCapabilityGrant>,
}

impl HostGrantSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_grants(grants: impl IntoIterator<Item = HostCapabilityGrant>) -> Self {
        let mut set = Self::new();
        for grant in grants {
            set.insert(grant);
        }
        set
    }

    pub fn insert(&mut self, grant: HostCapabilityGrant) {
        self.grants.insert(grant);
    }

    pub fn contains(&self, grant: &HostCapabilityGrant) -> bool {
        self.grants.contains(grant)
    }

    pub fn is_subset_of(&self, other: &Self) -> bool {
        self.grants.iter().all(|grant| other.contains(grant))
    }

    pub fn len(&self) -> usize {
        self.grants.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &HostCapabilityGrant> {
        self.grants.iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    pub max_runtime: Duration,
    pub max_memory_bytes: u64,
    pub max_outbound_requests: u32,
    pub max_outbound_response_bytes: u64,
    pub max_storage_writes: u32,
    pub max_storage_bytes: u64,
    pub max_concurrency: u16,
}

impl ResourceLimits {
    pub const fn new(
        max_runtime: Duration,
        max_memory_bytes: u64,
        max_outbound_requests: u32,
        max_outbound_response_bytes: u64,
        max_storage_writes: u32,
        max_storage_bytes: u64,
        max_concurrency: u16,
    ) -> Self {
        Self {
            max_runtime,
            max_memory_bytes,
            max_outbound_requests,
            max_outbound_response_bytes,
            max_storage_writes,
            max_storage_bytes,
            max_concurrency,
        }
    }

    pub fn baseline_for(point: ExtensionPointKind) -> Self {
        match point {
            ExtensionPointKind::Page
            | ExtensionPointKind::Api
            | ExtensionPointKind::AdminWidget
            | ExtensionPointKind::RenderHook => Self::new(
                Duration::from_secs(2),
                64 * 1024 * 1024,
                4,
                4 * 1024 * 1024,
                2,
                8 * 1024 * 1024,
                32,
            ),
            ExtensionPointKind::Webhook => Self::new(
                Duration::from_secs(5),
                64 * 1024 * 1024,
                6,
                8 * 1024 * 1024,
                2,
                8 * 1024 * 1024,
                16,
            ),
            ExtensionPointKind::Job | ExtensionPointKind::ScheduledJob => Self::new(
                Duration::from_secs(30),
                128 * 1024 * 1024,
                20,
                16 * 1024 * 1024,
                16,
                64 * 1024 * 1024,
                4,
            ),
        }
    }

    pub fn validate(&self) -> Result<(), WasmModelError> {
        if self.max_runtime.is_zero() {
            return Err(WasmModelError::ZeroLimit {
                field: "max_runtime",
            });
        }
        if self.max_memory_bytes == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_memory_bytes",
            });
        }
        if self.max_outbound_requests == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_outbound_requests",
            });
        }
        if self.max_outbound_response_bytes == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_outbound_response_bytes",
            });
        }
        if self.max_storage_writes == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_storage_writes",
            });
        }
        if self.max_storage_bytes == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_storage_bytes",
            });
        }
        if self.max_concurrency == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_concurrency",
            });
        }

        Ok(())
    }

    pub(crate) fn ensure_no_looser_than(
        &self,
        declared: &Self,
        handler_id: &crate::ids::HandlerId,
    ) -> Result<(), WasmModelError> {
        let checks = [
            (self.max_runtime <= declared.max_runtime, "max_runtime"),
            (
                self.max_memory_bytes <= declared.max_memory_bytes,
                "max_memory_bytes",
            ),
            (
                self.max_outbound_requests <= declared.max_outbound_requests,
                "max_outbound_requests",
            ),
            (
                self.max_outbound_response_bytes <= declared.max_outbound_response_bytes,
                "max_outbound_response_bytes",
            ),
            (
                self.max_storage_writes <= declared.max_storage_writes,
                "max_storage_writes",
            ),
            (
                self.max_storage_bytes <= declared.max_storage_bytes,
                "max_storage_bytes",
            ),
            (
                self.max_concurrency <= declared.max_concurrency,
                "max_concurrency",
            ),
        ];

        for (passes, field) in checks {
            if !passes {
                return Err(WasmModelError::LimitOverrideExceedsDeclared {
                    handler_id: handler_id.to_string(),
                    field,
                });
            }
        }

        Ok(())
    }
}
