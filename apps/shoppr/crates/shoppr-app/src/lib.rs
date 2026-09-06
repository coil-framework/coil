#![forbid(unsafe_code)]

mod extensions;
pub mod fission_app;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use coil::official_modules_from_config;
use coil_app::{
    CustomerAppComposition, CustomerAppManifest, CustomerAppRuntimePlan, MigrationPlanEntry,
    MigrationPlanOwner,
};
use coil_auth::load_auth_model_package_at;
use coil_config::{Environment, PlatformConfig};
use coil_customer_sdk::CustomerBackendPlugin;
use coil_data::{MigrationPlan, MigrationRegistry};
use coil_runtime::{EnvironmentSecretResolver, HttpServerHost, SecretResolver};
pub use shoppr_backend::ShopprLinkedPluginSummary;

#[derive(Debug, Clone)]
pub struct ShopprWorkspace {
    app_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ShopprBootstrap {
    pub app_root: PathBuf,
    pub config_path: PathBuf,
    pub manifest: CustomerAppManifest,
    pub composition: CustomerAppComposition,
    pub runtime_plan: CustomerAppRuntimePlan,
}

#[derive(Debug, Clone)]
pub struct ShopprSummary {
    pub app_root: PathBuf,
    pub config_path: PathBuf,
    pub manifest: CustomerAppManifest,
    pub config: PlatformConfig,
    pub linked_plugins: Vec<ShopprLinkedPluginSummary>,
    pub linked_plugin_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ShopprLifecycleValidation {
    pub app_root: PathBuf,
    pub config_path: PathBuf,
    pub app_id: String,
    pub module_ids: Vec<String>,
    pub linked_plugin_ids: Vec<String>,
    pub route_surface_count: usize,
    pub job_count: usize,
    pub migration_contract_count: usize,
    pub manual_customer_migration_entries: Vec<MigrationPlanEntry>,
}

#[derive(Debug, Clone)]
pub struct ShopprMigrationApplyReport {
    pub app_id: String,
    pub config_path: PathBuf,
    pub dry_run: bool,
    pub executable_steps: usize,
    pub pending_steps: usize,
    pub already_applied_steps: usize,
    pub executed_statements: usize,
    pub manual_customer_migration_entries: Vec<MigrationPlanEntry>,
}

#[derive(Debug, Clone)]
pub struct ShopprAssetPublicationReport {
    pub app_id: String,
    pub config_path: PathBuf,
    pub asset_roots: Vec<String>,
    pub published: bool,
    pub release_id: Option<String>,
    pub asset_entries: usize,
    pub writes: usize,
}

impl ShopprWorkspace {
    pub fn default() -> Result<Self> {
        if let Ok(app_root) = std::env::var("HARBOUR_SHOP_APP_ROOT") {
            return Self::at(app_root);
        }
        if let Ok(app_root) = std::env::var("HARBOR_SHOP_APP_ROOT") {
            return Self::at(app_root);
        }
        if let Some(app_root) = discover_workspace_root(std::env::current_dir().ok().as_deref()) {
            return Self::at(app_root);
        }
        Self::at(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    pub fn at(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if !path.exists() {
            bail!("Shoppr app root `{}` does not exist", path.display());
        }
        if !path.is_dir() {
            bail!(
                "Shoppr app root `{}` is not a directory",
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
        let manifest_path = self.manifest_path();
        let manifest = CustomerAppManifest::from_file(&manifest_path).with_context(|| {
            format!(
                "failed to load Shoppr manifest at `{}`",
                manifest_path.display()
            )
        })?;
        extensions::augment_manifest_with_extensions(&manifest_path, manifest)
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

    pub fn build_bootstrap(&self, config_path: impl AsRef<Path>) -> Result<ShopprBootstrap> {
        let manifest = self.load_manifest()?;
        let (config_path, mut config) = self.load_platform_config(config_path)?;
        config.wasm.directory = self
            .resolve_path(&config.wasm.directory)
            .display()
            .to_string();
        manifest
            .validate_runtime_config_alignment(&config)
            .context("Shoppr manifest/config alignment failed")?;

        let auth_package = load_auth_model_package_at(&manifest.auth.package_name, &self.app_root)
            .with_context(|| {
                format!(
                    "failed to load Shoppr auth package `{}` from `{}`",
                    manifest.auth.package_name,
                    self.app_root.display()
                )
            })?;

        let modules = official_modules_from_config(&config)
            .context("failed to resolve Shoppr modules")?;
        let extension_packages = extensions::load_extension_packages(
            &self.app_root,
            Path::new(&config.wasm.directory),
            &self.manifest_path(),
        )
        .context("failed to resolve Shoppr runtime-installed extension packages")?;
        let customer_plugins: Vec<Box<dyn CustomerBackendPlugin>> =
            vec![Box::new(shoppr_backend::plugin())];
        let runtime_plan = manifest
            .build_customer_root_runtime_plan_with_extensions_and_customer_plugins_at(
                config,
                auth_package,
                modules,
                extension_packages,
                customer_plugins,
                &self.app_root,
            )
            .context("Shoppr runtime build failed")?;
        let composition = runtime_plan.composition.clone();

        Ok(ShopprBootstrap {
            app_root: self.app_root.clone(),
            config_path,
            manifest,
            composition,
            runtime_plan,
        })
    }

    pub fn validate(&self, config_path: impl AsRef<Path>) -> Result<ShopprLifecycleValidation> {
        validate_workspace_layout(&self.app_root)?;
        let manifest = self.load_manifest()?;
        let (config_path, config) = self.load_platform_config(config_path)?;
        manifest
            .validate_runtime_config_alignment(&config)
            .context("Shoppr manifest/config alignment failed")?;

        let auth_package = load_auth_model_package_at(&manifest.auth.package_name, &self.app_root)
            .with_context(|| {
                format!(
                    "failed to load Shoppr auth package `{}` from `{}`",
                    manifest.auth.package_name,
                    self.app_root.display()
                )
            })?;
        let modules = official_modules_from_config(&config)
            .context("failed to resolve Shoppr modules")?;
        let manifests = modules
            .iter()
            .map(|module| module.manifest().clone())
            .collect::<Vec<_>>();
        let composition = manifest
            .compose(&auth_package, &manifests)
            .context("Shoppr customer composition failed")?;
        let manual_customer_migration_entries = manual_customer_migration_entries_from_composition(
            &composition,
            &manifest.id.to_string(),
        );

        Ok(ShopprLifecycleValidation {
            app_root: self.app_root.clone(),
            config_path,
            app_id: manifest.id.to_string(),
            module_ids: composition
                .installed_modules
                .iter()
                .map(|module| module.id.to_string())
                .collect(),
            linked_plugin_ids: vec![shoppr_backend::plugin().descriptor().id],
            route_surface_count: composition.route_surfaces.len(),
            job_count: composition.jobs.len(),
            migration_contract_count: composition.migrations.len(),
            manual_customer_migration_entries,
        })
    }

    pub fn migrate_apply(
        &self,
        config_path: impl AsRef<Path>,
        dry_run: bool,
    ) -> Result<ShopprMigrationApplyReport> {
        let bootstrap = self.build_bootstrap(config_path)?;
        bootstrap.apply_migrations(dry_run)
    }

    pub fn publish_assets(
        &self,
        config_path: impl AsRef<Path>,
    ) -> Result<ShopprAssetPublicationReport> {
        let bootstrap = self.build_bootstrap(config_path)?;
        Ok(bootstrap.asset_publication_report())
    }

    pub fn describe(&self, config_path: impl AsRef<Path>) -> Result<ShopprSummary> {
        let manifest = self.load_manifest()?;
        let (config_path, config) = self.load_platform_config(config_path)?;
        manifest
            .validate_runtime_config_alignment(&config)
            .context("Shoppr manifest/config alignment failed")?;

        let linked_plugins = vec![shoppr_backend::linked_plugin_summary()];
        Ok(ShopprSummary {
            app_root: self.app_root.clone(),
            config_path,
            manifest,
            config,
            linked_plugin_ids: linked_plugins
                .iter()
                .map(|plugin| plugin.id.clone())
                .collect(),
            linked_plugins,
        })
    }
}

pub fn shoppr_waitlist_tools_demo_sha256(app_root: impl AsRef<Path>) -> Result<String> {
    extensions::compiled_demo_artifact_sha256(app_root.as_ref(), "shoppr-waitlist-tools")
}

pub fn shoppr_waitlist_ops_widget_demo_sha256(app_root: impl AsRef<Path>) -> Result<String> {
    extensions::compiled_demo_artifact_sha256(app_root.as_ref(), "shoppr-waitlist-ops-widget")
}

fn discover_workspace_root(start: Option<&Path>) -> Option<PathBuf> {
    let mut current = start?.to_path_buf();
    loop {
        if current.join("app.toml").is_file()
            && (current.join("platform.dev.toml").is_file()
                || current.join("platform.toml").is_file())
        {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

impl ShopprBootstrap {
    pub fn server_host<R: SecretResolver>(
        &self,
        resolver: &R,
        cookie_secret: &[u8],
        csrf_secret: &[u8],
    ) -> Result<HttpServerHost> {
        self.runtime_plan
            .runtime
            .server_host(resolver, cookie_secret, csrf_secret)
            .context("failed to build Shoppr server host")
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

    pub fn apply_migrations(&self, dry_run: bool) -> Result<ShopprMigrationApplyReport> {
        let executable_plan = &self.runtime_plan.runtime.install_migrations;
        let manual_customer_migration_entries = self.manual_customer_migration_entries();

        if dry_run {
            return Ok(ShopprMigrationApplyReport {
                app_id: self.manifest.id.to_string(),
                config_path: self.config_path.clone(),
                dry_run: true,
                executable_steps: executable_plan.ordered_steps().len(),
                pending_steps: executable_plan.ordered_steps().len(),
                already_applied_steps: 0,
                executed_statements: 0,
                manual_customer_migration_entries,
            });
        }

        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to start Shoppr migration runtime")?;
        let _runtime_guard = tokio_runtime.enter();
        let client = self
            .runtime_plan
            .runtime
            .data
            .connect_lazy_postgres()
            .context("failed to connect to the Shoppr migration database")?;
        let applied_keys = tokio_runtime
            .block_on(async { client.applied_migration_keys().await })
            .context("failed to read applied Shoppr migrations")?;
        let pending_plan = pending_migration_plan(executable_plan, &applied_keys)?;
        let pending_steps = pending_plan.ordered_steps().len();
        let executed_statements = if pending_steps == 0 {
            0
        } else {
            let mut registry = MigrationRegistry::new();
            registry
                .register(&pending_plan)
                .context("failed to register Shoppr executable migrations")?;
            let batch = self
                .runtime_plan
                .runtime
                .data
                .compile_migrations(&registry)
                .context("failed to compile Shoppr executable migrations")?;
            let execution = tokio_runtime
                .block_on(async { client.apply_migrations(&batch).await })
                .context("failed to apply Shoppr migrations")?;
            execution.statements_executed
        };
        let catalog_repository =
            fission_app::PostgresCatalogRepository::connect(&self.runtime_plan.runtime.data)
                .context("failed to prepare the Shoppr catalogue repository")?;
        tokio_runtime
            .block_on(catalog_repository.seed_if_empty(
                &self.runtime_plan.runtime.storefront_catalog,
                &self.runtime_plan.runtime.config.sites,
            ))
            .map_err(anyhow::Error::msg)
            .context("failed to seed the Shoppr catalogue authority")?;

        Ok(ShopprMigrationApplyReport {
            app_id: self.manifest.id.to_string(),
            config_path: self.config_path.clone(),
            dry_run: false,
            executable_steps: executable_plan.ordered_steps().len(),
            pending_steps,
            already_applied_steps: executable_plan.ordered_steps().len() - pending_steps,
            executed_statements,
            manual_customer_migration_entries,
        })
    }

    pub fn asset_publication_report(&self) -> ShopprAssetPublicationReport {
        let asset_roots = self
            .manifest
            .theme
            .asset_roots()
            .iter()
            .map(|root| root.source_root().to_string())
            .collect::<Vec<_>>();
        let (published, release_id, asset_entries, writes) =
            if let Some(publication) = &self.runtime_plan.theme_publication {
                (
                    true,
                    Some(publication.manifest().release_id().to_string()),
                    publication.manifest().entries().len(),
                    publication.writes().len(),
                )
            } else {
                (false, None, 0, 0)
            };

        ShopprAssetPublicationReport {
            app_id: self.manifest.id.to_string(),
            config_path: self.config_path.clone(),
            asset_roots,
            published,
            release_id,
            asset_entries,
            writes,
        }
    }

    pub fn serve_from_env(&self, bind_override: Option<String>) -> Result<()> {
        self.runtime_plan
            .runtime
            .clone()
            .serve_from_env(bind_override)
            .context("failed to serve Shoppr from the customer runtime")
    }

    fn manual_customer_migration_entries(&self) -> Vec<MigrationPlanEntry> {
        self.runtime_plan
            .migration_summary
            .entries()
            .iter()
            .filter(|entry| matches!(&entry.owner, MigrationPlanOwner::CustomerApp(_)))
            .cloned()
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

fn validate_workspace_layout(app_root: &Path) -> Result<()> {
    if !app_root.join("app.toml").is_file() {
        bail!(
            "Shoppr manifest `{}` is missing",
            app_root.join("app.toml").display()
        );
    }
    if !app_root.join("templates").is_dir() {
        bail!(
            "Shoppr templates directory `{}` is missing",
            app_root.join("templates").display()
        );
    }
    Ok(())
}

fn manual_customer_migration_entries_from_composition(
    composition: &CustomerAppComposition,
    app_id: &str,
) -> Vec<MigrationPlanEntry> {
    composition
        .migrations
        .iter()
        .filter(|contract| {
            contract.owner == app_id || contract.owner == format!("customer_app:{app_id}")
        })
        .map(|contract| MigrationPlanEntry {
            owner: MigrationPlanOwner::CustomerApp(app_id.to_string()),
            step_id: None,
            order: contract.order,
            description: contract.description.clone(),
            online_safe: true,
        })
        .collect()
}

fn pending_migration_plan(
    plan: &MigrationPlan,
    applied_keys: &BTreeSet<(String, String)>,
) -> Result<MigrationPlan> {
    let mut pending = MigrationPlan::new();
    for step in plan.ordered_steps() {
        if applied_keys.contains(&(step.owner.to_string(), step.id.to_string())) {
            continue;
        }
        pending.insert(step.clone()).with_context(|| {
            format!(
                "failed to stage pending Shoppr migration `{}`",
                step.id
            )
        })?;
    }
    Ok(pending)
}
