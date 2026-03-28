use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectProduct {
    Storefront,
}

impl Default for ProjectProduct {
    fn default() -> Self {
        Self::Storefront
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectDescriptor {
    pub project: ProjectSection,
    #[serde(default)]
    pub modules: ModulesSection,
    pub i18n: I18nSection,
    #[serde(default)]
    pub sites: Vec<SiteDescriptor>,
    #[serde(default)]
    pub tooling: ToolingSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectSection {
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub product: ProjectProduct,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModulesSection {
    pub enabled: Vec<String>,
}

impl Default for ModulesSection {
    fn default() -> Self {
        Self {
            enabled: vec![
                "cms".to_string(),
                "media".to_string(),
                "commerce".to_string(),
                "admin".to_string(),
                "ops".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct I18nSection {
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    #[serde(default = "default_true")]
    pub localized_routes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteDescriptor {
    pub id: String,
    pub display_name: String,
    pub brand_name: String,
    pub canonical_domain: String,
    #[serde(default)]
    pub additional_domains: Vec<String>,
    pub default_locale: String,
    pub supported_locales: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolingSection {
    #[serde(default = "default_coil_version")]
    pub coil_version: String,
    #[serde(default)]
    pub dependency_source: DependencySource,
    #[serde(default = "default_true")]
    pub linked_rust_backend: bool,
    #[serde(default = "default_true")]
    pub wasm_directory: bool,
}

impl Default for ToolingSection {
    fn default() -> Self {
        Self {
            coil_version: default_coil_version(),
            dependency_source: DependencySource::default(),
            linked_rust_backend: true,
            wasm_directory: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DependencySource {
    #[default]
    CratesIo,
    Path { repo_root: String },
}

impl ProjectDescriptor {
    pub fn new(name: String, display_name: String, default_locale: String) -> Self {
        let supported_locales = vec![default_locale.clone()];
        Self {
            project: ProjectSection {
                name: name.clone(),
                display_name: display_name.clone(),
                product: ProjectProduct::Storefront,
            },
            modules: ModulesSection::default(),
            i18n: I18nSection {
                default_locale: default_locale.clone(),
                supported_locales: supported_locales.clone(),
                localized_routes: true,
            },
            sites: vec![SiteDescriptor {
                id: name.clone(),
                display_name: display_name.clone(),
                brand_name: display_name,
                canonical_domain: format!("{name}.localhost"),
                additional_domains: vec![format!("www.{name}.localhost")],
                default_locale,
                supported_locales,
            }],
            tooling: ToolingSection::default(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.project.name.trim().is_empty() {
            bail!("project.name must not be empty");
        }
        if self.project.display_name.trim().is_empty() {
            bail!("project.display_name must not be empty");
        }
        if self.modules.enabled.is_empty() {
            bail!("modules.enabled must not be empty");
        }
        if self.i18n.default_locale.trim().is_empty() {
            bail!("i18n.default_locale must not be empty");
        }
        if self.i18n.supported_locales.is_empty() {
            bail!("i18n.supported_locales must not be empty");
        }
        if !self
            .i18n
            .supported_locales
            .iter()
            .any(|locale| locale == &self.i18n.default_locale)
        {
            bail!("i18n.default_locale must be included in i18n.supported_locales");
        }
        if self.sites.is_empty() {
            bail!("at least one site must be defined");
        }

        let mut site_ids = BTreeSet::new();
        let supported_locales: BTreeSet<&str> = self
            .i18n
            .supported_locales
            .iter()
            .map(String::as_str)
            .collect();

        for site in &self.sites {
            if site.id.trim().is_empty() {
                bail!("site.id must not be empty");
            }
            if !site_ids.insert(site.id.as_str()) {
                bail!("duplicate site id `{}`", site.id);
            }
            if site.canonical_domain.trim().is_empty() {
                bail!("site `{}` must define canonical_domain", site.id);
            }
            if site.default_locale.trim().is_empty() {
                bail!("site `{}` must define default_locale", site.id);
            }
            if site.supported_locales.is_empty() {
                bail!("site `{}` must define supported_locales", site.id);
            }
            if !site
                .supported_locales
                .iter()
                .any(|locale| locale == &site.default_locale)
            {
                bail!(
                    "site `{}` default_locale must be included in supported_locales",
                    site.id
                );
            }
            for locale in &site.supported_locales {
                if !supported_locales.contains(locale.as_str()) {
                    bail!(
                        "site `{}` locale `{}` must be present in i18n.supported_locales",
                        site.id,
                        locale
                    );
                }
            }
        }

        match &self.tooling.dependency_source {
            DependencySource::CratesIo => {}
            DependencySource::Path { repo_root } => {
                if repo_root.trim().is_empty() {
                    bail!("tooling.dependency_source.path.repo_root must not be empty");
                }
            }
        }

        Ok(())
    }

    pub fn bin_crate_package_name(&self) -> &str {
        &self.project.name
    }

    pub fn bin_crate_dir_name(&self) -> String {
        format!("{}-bin", self.project.name)
    }

    pub fn backend_crate_name(&self) -> String {
        format!("{}-backend", self.project.name)
    }

    pub fn project_slug(&self) -> &str {
        &self.project.name
    }

    pub fn default_site(&self) -> &SiteDescriptor {
        &self.sites[0]
    }

    pub fn add_locale(&mut self, locale: String, site_id: &str) -> Result<()> {
        if !self.i18n.supported_locales.contains(&locale) {
            self.i18n.supported_locales.push(locale.clone());
        }
        let site = self
            .sites
            .iter_mut()
            .find(|site| site.id == site_id)
            .ok_or_else(|| anyhow!("site `{site_id}` does not exist"))?;
        if !site.supported_locales.contains(&locale) {
            site.supported_locales.push(locale);
        }
        self.validate()
    }

    pub fn add_site(&mut self, site: SiteDescriptor) -> Result<()> {
        self.sites.push(site);
        self.validate()
    }
}

fn default_true() -> bool {
    true
}

fn default_coil_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
