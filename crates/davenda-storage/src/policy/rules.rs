use davenda_config::StorageClass;
use thiserror::Error;

use super::{StoragePolicy, StoragePolicyOverride};
use crate::policy::paths::{normalize_relative_path, normalize_rule_prefix};

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
