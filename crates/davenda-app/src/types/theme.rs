use super::*;
use std::path::{Component, Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeAssetRoot {
    source_root: String,
}

impl ThemeAssetRoot {
    pub fn new(source_root: impl Into<String>) -> Result<Self, AppModelError> {
        let source_root = require_relative_theme_asset_root(source_root.into())?;
        Ok(Self { source_root })
    }

    pub fn source_root(&self) -> &str {
        &self.source_root
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeProfile {
    pub active: ThemeId,
    pub template_namespaces: Vec<TemplateNamespace>,
    pub asset_roots: Vec<ThemeAssetRoot>,
}

impl ThemeProfile {
    pub fn new(
        active: ThemeId,
        template_namespaces: Vec<TemplateNamespace>,
    ) -> Result<Self, AppModelError> {
        if template_namespaces.is_empty() {
            return Err(AppModelError::EmptyField {
                field: "template_namespaces",
            });
        }

        let mut seen = BTreeSet::new();
        for namespace in &template_namespaces {
            if !seen.insert(namespace.to_string()) {
                return Err(AppModelError::DuplicateThemeNamespace {
                    namespace: namespace.to_string(),
                });
            }
        }

        Ok(Self {
            active,
            template_namespaces,
            asset_roots: Vec::new(),
        })
    }

    pub fn asset_roots(&self) -> &[ThemeAssetRoot] {
        &self.asset_roots
    }

    pub fn with_asset_root(mut self, asset_root: impl Into<String>) -> Result<Self, AppModelError> {
        self.asset_roots.push(ThemeAssetRoot::new(asset_root)?);
        Ok(self)
    }

    pub fn publication_plan<P>(
        &self,
        release_id: davenda_assets::ReleaseId,
        app_root: P,
    ) -> Result<davenda_assets::ThemeAssetPublicationPlan, AppModelError>
    where
        P: AsRef<Path>,
    {
        davenda_assets::ThemeAssetPublicationPlan::from_roots(
            release_id,
            app_root,
            self.asset_roots.iter().map(ThemeAssetRoot::source_root),
        )
        .map_err(AppModelError::from)
    }
}

fn require_relative_theme_asset_root(value: String) -> Result<String, AppModelError> {
    if value.trim().is_empty() {
        return Err(AppModelError::EmptyField {
            field: "theme_asset_root",
        });
    }

    let path = Path::new(&value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppModelError::InvalidRelativePath {
            field: "theme_asset_root",
            value,
        });
    }

    Ok(value)
}
