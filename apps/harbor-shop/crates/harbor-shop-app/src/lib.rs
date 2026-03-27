#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use davenda_all::{load_auth_model_package_at, official_modules_from_config};
use davenda_app::{CustomerAppComposition, CustomerAppManifest, CustomerAppRuntimePlan};
use davenda_config::{Environment, PlatformConfig};
use davenda_customer_sdk::CustomerBackendPlugin;
use davenda_runtime::{EnvironmentSecretResolver, HttpServerHost, SecretResolver};

#[derive(Debug, Clone)]
pub struct HarborShopWorkspace {
    app_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct HarborShopBootstrap {
    pub app_root: PathBuf,
    pub config_path: PathBuf,
    pub manifest: CustomerAppManifest,
    pub composition: CustomerAppComposition,
    pub runtime_plan: CustomerAppRuntimePlan,
}

#[derive(Debug, Clone)]
pub struct HarborShopSummary {
    pub app_root: PathBuf,
    pub config_path: PathBuf,
    pub manifest: CustomerAppManifest,
    pub config: PlatformConfig,
    pub linked_plugin_ids: Vec<String>,
}

impl HarborShopWorkspace {
    pub fn default() -> Result<Self> {
        Self::at(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    pub fn at(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if !path.exists() {
            bail!("Harbor Shop app root `{}` does not exist", path.display());
        }
        if !path.is_dir() {
            bail!(
                "Harbor Shop app root `{}` is not a directory",
                path.display()
            );
        }

        let app_root = path.canonicalize().unwrap_or(path);
        Ok(Self { app_root })
    }

    pub fn app_root(&self) -> &Path {
        &self.app_root
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.app_root.join("app.toml")
    }

    pub fn default_config_path(&self) -> PathBuf {
        self.app_root.join("platform.dev.toml")
    }

    pub fn resolve_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.app_root.join(path)
        }
    }

    pub fn load_manifest(&self) -> Result<CustomerAppManifest> {
        CustomerAppManifest::from_file(self.manifest_path()).with_context(|| {
            format!(
                "failed to load Harbor Shop manifest at `{}`",
                self.manifest_path().display()
            )
        })
    }

    pub fn load_platform_config(
        &self,
        config_path: impl AsRef<Path>,
    ) -> Result<(PathBuf, PlatformConfig)> {
        let config_path = self.resolve_path(config_path);
        let input = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config `{}`", config_path.display()))?;
        let config = PlatformConfig::from_toml_str(&input)
            .with_context(|| format!("failed to parse config `{}`", config_path.display()))?;
        Ok((config_path, config))
    }

    pub fn build_bootstrap(&self, config_path: impl AsRef<Path>) -> Result<HarborShopBootstrap> {
        let manifest = self.load_manifest()?;
        let (config_path, config) = self.load_platform_config(config_path)?;
        manifest
            .validate_runtime_config_alignment(&config)
            .context("Harbor Shop manifest/config alignment failed")?;

        let auth_package = load_auth_model_package_at(&manifest.auth.package_name, &self.app_root)
            .with_context(|| {
                format!(
                    "failed to load Harbor Shop auth package `{}` from `{}`",
                    manifest.auth.package_name,
                    self.app_root.display()
                )
            })?;

        let modules = official_modules_from_config(&config)
            .context("failed to resolve Harbor Shop modules")?;
        let customer_plugins: Vec<Box<dyn CustomerBackendPlugin>> =
            vec![Box::new(harbor_shop_backend::plugin())];
        let runtime_plan = manifest
            .build_runtime_plan_with_customer_plugins(
                config,
                auth_package,
                modules,
                customer_plugins,
                &self.app_root,
            )
            .context("Harbor Shop runtime build failed")?;
        let composition = runtime_plan.composition.clone();

        Ok(HarborShopBootstrap {
            app_root: self.app_root.clone(),
            config_path,
            manifest,
            composition,
            runtime_plan,
        })
    }

    pub fn describe(&self, config_path: impl AsRef<Path>) -> Result<HarborShopSummary> {
        let manifest = self.load_manifest()?;
        let (config_path, config) = self.load_platform_config(config_path)?;
        manifest
            .validate_runtime_config_alignment(&config)
            .context("Harbor Shop manifest/config alignment failed")?;

        Ok(HarborShopSummary {
            app_root: self.app_root.clone(),
            config_path,
            manifest,
            config,
            linked_plugin_ids: vec![harbor_shop_backend::plugin().descriptor().id],
        })
    }
}

impl HarborShopBootstrap {
    pub fn server_host<R: SecretResolver>(
        &self,
        resolver: &R,
        cookie_secret: &[u8],
        csrf_secret: &[u8],
    ) -> Result<HttpServerHost> {
        self.runtime_plan
            .runtime
            .server_host(resolver, cookie_secret, csrf_secret)
            .context("failed to build Harbor Shop server host")
    }

    pub fn module_ids(&self) -> Vec<String> {
        self.composition
            .installed_modules
            .iter()
            .map(|module| module.id.to_string())
            .collect()
    }

    pub fn linked_plugin_ids(&self) -> Vec<String> {
        self.runtime_plan
            .runtime
            .linked_customer_plugins
            .iter()
            .map(|plugin| plugin.plugin_id.clone())
            .collect()
    }
}

pub fn default_cookie_secret(environment: Environment) -> Option<&'static str> {
    matches!(environment, Environment::Development).then_some("01234567012345670123456701234567")
}

pub fn default_csrf_secret(environment: Environment) -> Option<&'static str> {
    matches!(environment, Environment::Development).then_some("76543210765432107654321076543210")
}

pub fn environment_secret_resolver() -> EnvironmentSecretResolver {
    EnvironmentSecretResolver
}
