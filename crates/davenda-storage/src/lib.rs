use std::fmt;

use davenda_config::{ObjectStoreKind, PlatformConfig, StorageClass};
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
    pub object_store: Option<ObjectStoreTarget>,
}

impl StorageTopology {
    pub fn from_config(config: &PlatformConfig) -> Self {
        Self {
            local_root: trim_trailing_separator(&config.storage.local_root),
            default_class: config.storage.default_class,
            object_store: config
                .storage
                .object_store
                .map(|kind| ObjectStoreTarget { kind }),
        }
    }

    pub fn supports_object_store(&self) -> bool {
        self.object_store.is_some()
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

    fn matches(&self, logical_path: &str) -> bool {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePlanRequest {
    pub logical_path: String,
    pub storage_class: Option<StorageClass>,
    pub override_policy: Option<StoragePolicyOverride>,
}

impl StoragePlanRequest {
    pub fn new(logical_path: impl Into<String>) -> Self {
        Self {
            logical_path: logical_path.into(),
            storage_class: None,
            override_policy: None,
        }
    }

    pub fn with_storage_class(mut self, storage_class: StorageClass) -> Self {
        self.storage_class = Some(storage_class);
        self
    }

    pub fn with_override(mut self, override_policy: StoragePolicyOverride) -> Self {
        self.override_policy = Some(override_policy);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteTargetKind {
    Primary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteTarget {
    pub backend: StorageBackendKind,
    pub locator: String,
    pub kind: WriteTargetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoragePlanWarning {
    LocalOnlyBreaksMultiNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePlan {
    pub logical_path: String,
    pub storage_class: StorageClass,
    pub policy: StoragePolicy,
    pub durable_store: DurableStore,
    pub object_key: Option<String>,
    pub local_path: Option<String>,
    pub matched_rule_prefix: Option<String>,
    pub write_targets: Vec<WriteTarget>,
    pub warnings: Vec<StoragePlanWarning>,
}

impl StoragePlan {
    pub fn primary_write_target(&self) -> Option<&WriteTarget> {
        self.write_targets
            .iter()
            .find(|target| target.kind == WriteTargetKind::Primary)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePlanner {
    topology: StorageTopology,
    policies: StoragePolicySet,
}

impl StoragePlanner {
    pub fn from_config(config: &PlatformConfig) -> Self {
        Self {
            topology: StorageTopology::from_config(config),
            policies: StoragePolicySet::default(),
        }
    }

    pub fn new(topology: StorageTopology, policies: StoragePolicySet) -> Self {
        Self { topology, policies }
    }

    pub fn topology(&self) -> &StorageTopology {
        &self.topology
    }

    pub fn policies(&self) -> &StoragePolicySet {
        &self.policies
    }

    pub fn plan_write(
        &self,
        request: StoragePlanRequest,
    ) -> Result<StoragePlan, StoragePlanningError> {
        let logical_path = normalize_relative_path(&request.logical_path)?;
        let storage_class = request.storage_class.unwrap_or(self.topology.default_class);
        let resolved = self.policies.resolve(
            storage_class,
            &logical_path,
            request.override_policy.as_ref(),
        )?;

        let mut warnings = Vec::new();
        let policy = resolved.policy;
        let durable_store = policy.durable_store();

        let object_key = match policy.sync_mode {
            SyncMode::ObjectStore => {
                if self.topology.object_store.is_none() {
                    return Err(StoragePlanningError::ObjectStoreRequired {
                        logical_path,
                        policy,
                    });
                }

                Some(join_relative(
                    resolved.object_prefix.as_deref(),
                    &logical_path,
                ))
            }
            SyncMode::LocalOnly => None,
        };

        let local_path = match policy.sync_mode {
            SyncMode::ObjectStore => None,
            SyncMode::LocalOnly => {
                warnings.push(StoragePlanWarning::LocalOnlyBreaksMultiNode);
                Some(join_local_path(
                    &self.topology.local_root,
                    resolved.local_subdir.as_deref(),
                    &logical_path,
                ))
            }
        };

        let write_targets = match policy.sync_mode {
            SyncMode::ObjectStore => vec![WriteTarget {
                backend: self
                    .topology
                    .object_store
                    .as_ref()
                    .expect("object store availability checked")
                    .backend_kind(),
                locator: object_key
                    .clone()
                    .expect("object key is present for object store policies"),
                kind: WriteTargetKind::Primary,
            }],
            SyncMode::LocalOnly => vec![WriteTarget {
                backend: StorageBackendKind::LocalDisk,
                locator: local_path
                    .clone()
                    .expect("local path is present for local policies"),
                kind: WriteTargetKind::Primary,
            }],
        };

        Ok(StoragePlan {
            logical_path,
            storage_class: resolved.storage_class,
            policy,
            durable_store,
            object_key,
            local_path,
            matched_rule_prefix: resolved.matched_rule_prefix,
            write_targets,
            warnings,
        })
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StoragePlanningError {
    #[error(transparent)]
    Policy(#[from] StoragePolicyError),
    #[error(
        "storage plan for `{logical_path}` requires object storage but no object-store backend is configured for policy {policy:?}"
    )]
    ObjectStoreRequired {
        logical_path: String,
        policy: StoragePolicy,
    },
}

fn normalize_relative_path(input: &str) -> Result<String, StoragePolicyError> {
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

fn normalize_rule_prefix(input: &str) -> Result<String, StoragePolicyError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    normalize_relative_path(trimmed)
}

fn join_relative(prefix: Option<&str>, logical_path: &str) -> String {
    join_relative_many(prefix, logical_path)
}

fn join_relative_many(prefix: Option<&str>, logical_path: &str) -> String {
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

fn join_local_path(root: &str, subdir: Option<&str>, logical_path: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_policy_combinations() {
        let policy = StoragePolicy::new(
            DeliveryMode::PublicCdn,
            SyncMode::LocalOnly,
            Sensitivity::Public,
        );

        assert_eq!(
            policy.validate(),
            Err(StoragePolicyError::InvalidCombination {
                detail: "public_cdn delivery requires object_store sync".to_string(),
            })
        );
    }

    #[test]
    fn resolves_most_specific_path_policy() {
        let policies = StoragePolicySet::default()
            .with_rule(
                PathPolicyRule::new(
                    "uploads",
                    Some(StorageClass::PrivateShared),
                    StoragePolicy::private_shared(),
                )
                .expect("valid root uploads rule"),
            )
            .with_rule(
                PathPolicyRule::new(
                    "uploads/marketing",
                    Some(StorageClass::PublicUpload),
                    StoragePolicy::public_upload(),
                )
                .expect("valid nested marketing rule")
                .with_object_prefix("public/marketing")
                .expect("valid object prefix"),
            );

        let resolved = policies
            .resolve(
                StorageClass::PrivateShared,
                "uploads/marketing/brochure.pdf",
                None,
            )
            .expect("path policy resolves");

        assert_eq!(resolved.storage_class, StorageClass::PublicUpload);
        assert_eq!(resolved.policy, StoragePolicy::public_upload());
        assert_eq!(resolved.object_prefix.as_deref(), Some("public/marketing"));
        assert_eq!(
            resolved.matched_rule_prefix.as_deref(),
            Some("uploads/marketing")
        );
    }

    #[test]
    fn object_store_policies_plan_write_through_storage() {
        let config = test_config();
        let planner = StoragePlanner::new(
            StorageTopology::from_config(&config),
            StoragePolicySet::default().with_rule(
                PathPolicyRule::new(
                    "uploads/marketing",
                    Some(StorageClass::PublicUpload),
                    StoragePolicy::public_upload(),
                )
                .expect("valid marketing rule")
                .with_object_prefix("public")
                .expect("valid object prefix"),
            ),
        );

        let plan = planner
            .plan_write(
                StoragePlanRequest::new("uploads/marketing/hero.webp")
                    .with_storage_class(StorageClass::PublicUpload),
            )
            .expect("public uploads should plan against object storage");

        assert_eq!(plan.durable_store, DurableStore::ObjectStore);
        assert_eq!(
            plan.object_key.as_deref(),
            Some("public/uploads/marketing/hero.webp")
        );
        assert_eq!(plan.local_path, None);
        assert!(plan.warnings.is_empty());
        assert_eq!(
            plan.primary_write_target()
                .expect("primary write target")
                .backend,
            StorageBackendKind::S3Compatible
        );
    }

    #[test]
    fn local_only_override_keeps_sensitive_files_on_server() {
        let planner = StoragePlanner::from_config(&test_config());

        let plan = planner
            .plan_write(
                StoragePlanRequest::new("secure/reports/march.csv")
                    .with_storage_class(StorageClass::PrivateShared)
                    .with_override(StoragePolicyOverride::force_local_only()),
            )
            .expect("local-only override should succeed");

        assert_eq!(plan.policy, StoragePolicy::local_only_sensitive());
        assert_eq!(plan.durable_store, DurableStore::LocalDisk);
        assert_eq!(
            plan.local_path.as_deref(),
            Some("var/davenda/storage/secure/reports/march.csv")
        );
        assert_eq!(plan.object_key, None);
        assert_eq!(
            plan.warnings,
            vec![StoragePlanWarning::LocalOnlyBreaksMultiNode]
        );
    }

    #[test]
    fn rejects_parent_traversal() {
        let planner = StoragePlanner::from_config(&test_config());

        let error = planner
            .plan_write(StoragePlanRequest::new("../secrets.txt"))
            .expect_err("parent traversal must be rejected");

        assert_eq!(
            error,
            StoragePlanningError::Policy(StoragePolicyError::ParentTraversal {
                path: "../secrets.txt".to_string(),
            })
        );
    }

    #[test]
    fn object_store_sync_requires_backend_configuration() {
        let mut config = test_config();
        config.storage.object_store = None;
        let planner = StoragePlanner::from_config(&config);

        let error = planner
            .plan_write(
                StoragePlanRequest::new("uploads/catalog/item.jpg")
                    .with_storage_class(StorageClass::PublicUpload),
            )
            .expect_err("public uploads should require object storage");

        assert_eq!(
            error,
            StoragePlanningError::ObjectStoreRequired {
                logical_path: "uploads/catalog/item.jpg".to_string(),
                policy: StoragePolicy::public_upload(),
            }
        );
    }

    fn test_config() -> PlatformConfig {
        PlatformConfig::from_toml_str(
            r#"
[app]
name = "davenda"
environment = "development"

[server]
bind = "127.0.0.1:3000"

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "external"

[storage]
default_class = "private_shared"
object_store = "s3"
local_root = "var/davenda/storage"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "en-US"]
fallback_locale = "en-GB"
localized_routes = true

[seo]
canonical_host = "https://example.com"
emit_json_ld = true
sitemap_enabled = true

[auth]
package = "platform-default-auth"
explain_api = true
tenant_id = 101

[modules]
enabled = ["davenda-cms", "davenda-commerce"]

[wasm]
directory = "wasm"
default_time_limit_ms = 25
allow_network = false

[jobs]
backend = "redis"
retry_limit = 5

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#,
        )
        .expect("valid test config")
    }
}
