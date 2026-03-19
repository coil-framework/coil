use std::fmt;

use davenda_config::{ObjectStoreKind, PlatformConfig, StorageClass, StorageDeployment};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageBackendKind {
    LocalDisk,
    S3Compatible,
}

impl fmt::Display for StorageBackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalDisk => f.write_str("local_disk"),
            Self::S3Compatible => f.write_str("s3_compatible"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DurableStore {
    LocalDisk,
    ObjectStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeliveryMode {
    PublicCdn,
    SignedUrl,
    AppProxy,
    LocalOnly,
}

impl fmt::Display for DeliveryMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublicCdn => f.write_str("public_cdn"),
            Self::SignedUrl => f.write_str("signed_url"),
            Self::AppProxy => f.write_str("app_proxy"),
            Self::LocalOnly => f.write_str("local_only"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncMode {
    ObjectStore,
    LocalOnly,
}

impl fmt::Display for SyncMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectStore => f.write_str("object_store"),
            Self::LocalOnly => f.write_str("local_only"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sensitivity {
    Public,
    Internal,
    Restricted,
    Secret,
}

impl fmt::Display for Sensitivity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public => f.write_str("public"),
            Self::Internal => f.write_str("internal"),
            Self::Restricted => f.write_str("restricted"),
            Self::Secret => f.write_str("secret"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoragePolicy {
    pub delivery_mode: DeliveryMode,
    pub sync_mode: SyncMode,
    pub sensitivity: Sensitivity,
}

impl StoragePolicy {
    pub const fn new(
        delivery_mode: DeliveryMode,
        sync_mode: SyncMode,
        sensitivity: Sensitivity,
    ) -> Self {
        Self {
            delivery_mode,
            sync_mode,
            sensitivity,
        }
    }

    pub const fn public_asset() -> Self {
        Self::new(
            DeliveryMode::PublicCdn,
            SyncMode::ObjectStore,
            Sensitivity::Public,
        )
    }

    pub const fn public_upload() -> Self {
        Self::new(
            DeliveryMode::PublicCdn,
            SyncMode::ObjectStore,
            Sensitivity::Public,
        )
    }

    pub const fn private_shared() -> Self {
        Self::new(
            DeliveryMode::SignedUrl,
            SyncMode::ObjectStore,
            Sensitivity::Restricted,
        )
    }

    pub const fn local_only_sensitive() -> Self {
        Self::new(
            DeliveryMode::LocalOnly,
            SyncMode::LocalOnly,
            Sensitivity::Secret,
        )
    }

    pub fn validate(&self) -> Result<(), StoragePolicyError> {
        match (self.delivery_mode, self.sync_mode, self.sensitivity) {
            (DeliveryMode::PublicCdn, SyncMode::LocalOnly, _) => {
                Err(StoragePolicyError::InvalidCombination {
                    detail: "public_cdn delivery requires object_store sync".to_string(),
                })
            }
            (DeliveryMode::SignedUrl, SyncMode::LocalOnly, _) => {
                Err(StoragePolicyError::InvalidCombination {
                    detail: "signed_url delivery requires object_store sync".to_string(),
                })
            }
            (
                DeliveryMode::PublicCdn,
                _,
                Sensitivity::Internal | Sensitivity::Restricted | Sensitivity::Secret,
            ) => Err(StoragePolicyError::InvalidCombination {
                detail: "public_cdn delivery is only valid for public content".to_string(),
            }),
            (DeliveryMode::LocalOnly, SyncMode::ObjectStore, _) => {
                Err(StoragePolicyError::InvalidCombination {
                    detail: "local_only delivery cannot use object_store sync".to_string(),
                })
            }
            (DeliveryMode::SignedUrl, _, Sensitivity::Public) => {
                Err(StoragePolicyError::InvalidCombination {
                    detail: "signed_url delivery is for non-public content".to_string(),
                })
            }
            (_, _, _) => Ok(()),
        }
    }

    pub const fn durable_store(&self) -> DurableStore {
        match self.sync_mode {
            SyncMode::ObjectStore => DurableStore::ObjectStore,
            SyncMode::LocalOnly => DurableStore::LocalDisk,
        }
    }

    pub const fn is_public_delivery_eligible(&self) -> bool {
        matches!(
            (self.delivery_mode, self.sensitivity),
            (DeliveryMode::PublicCdn, Sensitivity::Public)
        )
    }
}

impl From<StorageClass> for StoragePolicy {
    fn from(value: StorageClass) -> Self {
        match value {
            StorageClass::PublicAsset => Self::public_asset(),
            StorageClass::PublicUpload => Self::public_upload(),
            StorageClass::PrivateShared => Self::private_shared(),
            StorageClass::LocalOnlySensitive => Self::local_only_sensitive(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoragePolicyOverride {
    pub delivery_mode: Option<DeliveryMode>,
    pub sync_mode: Option<SyncMode>,
    pub sensitivity: Option<Sensitivity>,
}

impl StoragePolicyOverride {
    pub fn apply_to(&self, base: StoragePolicy) -> StoragePolicy {
        StoragePolicy {
            delivery_mode: self.delivery_mode.unwrap_or(base.delivery_mode),
            sync_mode: self.sync_mode.unwrap_or(base.sync_mode),
            sensitivity: self.sensitivity.unwrap_or(base.sensitivity),
        }
    }

    pub fn force_local_only() -> Self {
        Self {
            delivery_mode: Some(DeliveryMode::LocalOnly),
            sync_mode: Some(SyncMode::LocalOnly),
            sensitivity: Some(Sensitivity::Secret),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStoreTarget {
    pub kind: ObjectStoreKind,
}

impl ObjectStoreTarget {
    pub const fn backend_kind(&self) -> StorageBackendKind {
        match self.kind {
            ObjectStoreKind::S3 => StorageBackendKind::S3Compatible,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageTopology {
    pub local_root: String,
    pub default_class: StorageClass,
    pub deployment: StorageDeployment,
    pub object_store: Option<ObjectStoreTarget>,
}

impl StorageTopology {
    pub fn from_config(config: &PlatformConfig) -> Self {
        Self {
            local_root: trim_trailing_separator(&config.storage.local_root),
            default_class: config.storage.default_class,
            deployment: config.storage.deployment,
            object_store: config
                .storage
                .object_store
                .map(|kind| ObjectStoreTarget { kind }),
        }
    }

    pub fn supports_object_store(&self) -> bool {
        self.object_store.is_some()
    }

    pub const fn allows_local_only(&self) -> bool {
        matches!(self.deployment, StorageDeployment::SingleNode)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathPolicyRule {
    pub path_prefix: String,
    pub storage_class: Option<StorageClass>,
    pub policy: StoragePolicy,
    pub object_prefix: Option<String>,
    pub local_subdir: Option<String>,
}

impl PathPolicyRule {
    pub fn new(
        path_prefix: impl Into<String>,
        storage_class: Option<StorageClass>,
        policy: StoragePolicy,
    ) -> Result<Self, StoragePolicyError> {
        let path_prefix = normalize_rule_prefix(&path_prefix.into())?;
        policy.validate()?;
        Ok(Self {
            path_prefix,
            storage_class,
            policy,
            object_prefix: None,
            local_subdir: None,
        })
    }

    pub fn with_object_prefix(
        mut self,
        prefix: impl Into<String>,
    ) -> Result<Self, StoragePolicyError> {
        self.object_prefix = Some(normalize_rule_prefix(&prefix.into())?);
        Ok(self)
    }

    pub fn with_local_subdir(
        mut self,
        subdir: impl Into<String>,
    ) -> Result<Self, StoragePolicyError> {
        self.local_subdir = Some(normalize_rule_prefix(&subdir.into())?);
        Ok(self)
    }

    pub(crate) fn matches(&self, logical_path: &str) -> bool {
        self.path_prefix.is_empty()
            || logical_path == self.path_prefix
            || logical_path.starts_with(&format!("{}/", self.path_prefix))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStoragePolicy {
    pub storage_class: StorageClass,
    pub policy: StoragePolicy,
    pub matched_rule_prefix: Option<String>,
    pub object_prefix: Option<String>,
    pub local_subdir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePolicySet {
    rules: Vec<PathPolicyRule>,
}

impl StoragePolicySet {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn with_rule(mut self, rule: PathPolicyRule) -> Self {
        self.rules.push(rule);
        self.rules
            .sort_by(|left, right| right.path_prefix.len().cmp(&left.path_prefix.len()));
        self
    }

    pub fn resolve(
        &self,
        storage_class: StorageClass,
        logical_path: &str,
        override_policy: Option<&StoragePolicyOverride>,
    ) -> Result<ResolvedStoragePolicy, StoragePolicyError> {
        let logical_path = normalize_relative_path(logical_path)?;
        let matched_rule = self.rules.iter().find(|rule| rule.matches(&logical_path));

        let derived_class = matched_rule
            .and_then(|rule| rule.storage_class)
            .unwrap_or(storage_class);

        let base_policy = matched_rule
            .map(|rule| rule.policy)
            .unwrap_or_else(|| derived_class.into());
        let policy = override_policy
            .map(|policy_override| policy_override.apply_to(base_policy))
            .unwrap_or(base_policy);
        policy.validate()?;

        Ok(ResolvedStoragePolicy {
            storage_class: derived_class,
            policy,
            matched_rule_prefix: matched_rule.map(|rule| rule.path_prefix.clone()),
            object_prefix: matched_rule.and_then(|rule| rule.object_prefix.clone()),
            local_subdir: matched_rule.and_then(|rule| rule.local_subdir.clone()),
        })
    }
}

impl Default for StoragePolicySet {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StoragePolicyError {
    #[error("storage policy contains an invalid combination: {detail}")]
    InvalidCombination { detail: String },
    #[error("storage paths must be relative and non-empty, got `{path}`")]
    InvalidRelativePath { path: String },
    #[error("storage paths cannot traverse parent segments, got `{path}`")]
    ParentTraversal { path: String },
}

pub(crate) fn normalize_relative_path(input: &str) -> Result<String, StoragePolicyError> {
    let trimmed = input.trim();

    if trimmed.is_empty() || trimmed.starts_with('/') {
        return Err(StoragePolicyError::InvalidRelativePath {
            path: input.to_string(),
        });
    }

    let mut segments = Vec::new();
    for segment in trimmed.split('/') {
        match segment {
            "" | "." => continue,
            ".." => {
                return Err(StoragePolicyError::ParentTraversal {
                    path: input.to_string(),
                });
            }
            _ => segments.push(segment),
        }
    }

    if segments.is_empty() {
        return Err(StoragePolicyError::InvalidRelativePath {
            path: input.to_string(),
        });
    }

    Ok(segments.join("/"))
}

pub(crate) fn normalize_rule_prefix(input: &str) -> Result<String, StoragePolicyError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    normalize_relative_path(trimmed)
}

pub(crate) fn join_relative(prefix: Option<&str>, logical_path: &str) -> String {
    let mut parts = Vec::new();

    if let Some(prefix) = prefix {
        let normalized = prefix.trim().trim_matches('/');
        if !normalized.is_empty() {
            parts.push(normalized);
        }
    }

    let normalized_path = logical_path.trim().trim_matches('/');
    if !normalized_path.is_empty() {
        parts.push(normalized_path);
    }

    parts.join("/")
}

pub(crate) fn join_local_path(root: &str, subdir: Option<&str>, logical_path: &str) -> String {
    let root = root.trim_end_matches('/');
    let root_is_absolute = root.starts_with('/');
    let mut parts = Vec::new();

    if !root.is_empty() {
        parts.push(if root_is_absolute {
            root.trim_start_matches('/').to_string()
        } else {
            root.to_string()
        });
    }

    if let Some(subdir) = subdir {
        let normalized = subdir.trim().trim_matches('/');
        if !normalized.is_empty() {
            parts.push(normalized.to_string());
        }
    }

    let normalized_path = logical_path.trim().trim_matches('/');
    if !normalized_path.is_empty() {
        parts.push(normalized_path.to_string());
    }

    let joined = parts.join("/");
    if root_is_absolute {
        format!("/{joined}")
    } else {
        joined
    }
}

fn trim_trailing_separator(input: &str) -> String {
    let trimmed = input.trim_end_matches('/');
    if trimmed.is_empty() {
        input.to_string()
    } else {
        trimmed.to_string()
    }
}
