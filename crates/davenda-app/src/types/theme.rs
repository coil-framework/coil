use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeProfile {
    pub active: ThemeId,
    pub template_namespaces: Vec<TemplateNamespace>,
    pub asset_roots: Vec<String>,
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

    pub fn with_asset_root(mut self, asset_root: impl Into<String>) -> Result<Self, AppModelError> {
        self.asset_roots
            .push(require_non_empty("theme_asset_root", asset_root.into())?);
        Ok(self)
    }
}
