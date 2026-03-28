#![forbid(unsafe_code)]

mod extensions;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use davenda_all::official_module;
use davenda_app::{
    CustomerAppComposition, CustomerAppManifest, CustomerAppRuntimePlan, MigrationPlanEntry,
    MigrationPlanOwner,
};
use davenda_auth::load_auth_model_package_at;
use davenda_config::{Environment, PlatformConfig};
use davenda_core::{
    ExtensionSlotDescriptor, ExtensionSlotKind, ModuleManifest, PlatformModule, RegistrationError,
    ServiceRegistry,
};
use davenda_customer_sdk::CustomerBackendPlugin;
use davenda_data::{MigrationPlan, MigrationRegistry};
use davenda_runtime::{
    EnvironmentSecretResolver, HandlerDefinition, HttpMethod, HttpServerHost, RouteArea,
    RouteDefinition, RuntimePlan, SecretResolver,
};
pub use gitly_backend::GitlyLinkedPluginSummary;

const SHOWCASE_MODULE_ID: &str = "gitly-showcase";

#[derive(Debug, Clone)]
pub struct GitlyWorkspace {
    app_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GitlyBootstrap {
    pub app_root: PathBuf,
    pub config_path: PathBuf,
    pub manifest: CustomerAppManifest,
    pub composition: CustomerAppComposition,
    pub runtime_plan: CustomerAppRuntimePlan,
}

#[derive(Debug, Clone)]
pub struct GitlySummary {
    pub app_root: PathBuf,
    pub config_path: PathBuf,
    pub manifest: CustomerAppManifest,
    pub config: PlatformConfig,
    pub linked_plugins: Vec<GitlyLinkedPluginSummary>,
    pub linked_plugin_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GitlyLifecycleValidation {
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
pub struct GitlyMigrationApplyReport {
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
pub struct GitlyAssetPublicationReport {
    pub app_id: String,
    pub config_path: PathBuf,
    pub asset_roots: Vec<String>,
    pub published: bool,
    pub release_id: Option<String>,
    pub asset_entries: usize,
    pub writes: usize,
}

#[derive(Debug, Clone, Default)]
struct GitlyShowcaseModule;

impl GitlyShowcaseModule {
    fn manifest_definition() -> ModuleManifest {
        ModuleManifest::new(SHOWCASE_MODULE_ID).with_extension_slots(vec![
            ExtensionSlotDescriptor::new(
                ExtensionSlotKind::Api,
                "/api/github/pulse",
                "Allows bounded third-party extensions to contribute GitHub-style community pulse API data",
            ),
            ExtensionSlotDescriptor::new(
                ExtensionSlotKind::ScheduledJob,
                "github.actions.refresh",
                "Allows bounded third-party scheduled jobs to simulate GitHub Actions refresh cycles",
            ),
        ])
    }
}

impl PlatformModule for GitlyShowcaseModule {
    fn manifest(&self) -> ModuleManifest {
        Self::manifest_definition()
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_customer_app_service(
            "gitly.showcase",
            "Customer-owned Gitly showcase routes, API, and extension surfaces",
        )
    }
}

impl GitlyWorkspace {
    pub fn default() -> Result<Self> {
        if let Ok(app_root) = std::env::var("OCTOHUB_APP_ROOT") {
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
            bail!("Gitly app root `{}` does not exist", path.display());
        }
        if !path.is_dir() {
            bail!("Gitly app root `{}` is not a directory", path.display());
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
                "failed to load Gitly manifest at `{}`",
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

    pub fn build_bootstrap(&self, config_path: impl AsRef<Path>) -> Result<GitlyBootstrap> {
        let manifest = self.load_manifest()?;
        let (config_path, mut config) = self.load_platform_config(config_path)?;
        config.wasm.directory = self
            .resolve_path(&config.wasm.directory)
            .display()
            .to_string();
        manifest
            .validate_runtime_config_alignment(&config)
            .context("Gitly manifest/config alignment failed")?;

        let auth_package = load_auth_model_package_at(&manifest.auth.package_name, &self.app_root)
            .with_context(|| {
                format!(
                    "failed to load Gitly auth package `{}` from `{}`",
                    manifest.auth.package_name,
                    self.app_root.display()
                )
            })?;

        let modules = resolve_modules_from_config(&config).context("failed to resolve Gitly modules")?;
        let extension_packages = extensions::load_extension_packages(
            &self.app_root,
            Path::new(&config.wasm.directory),
            &self.manifest_path(),
        )
        .context("failed to resolve Gitly runtime-installed extension packages")?;
        let customer_plugins: Vec<Box<dyn CustomerBackendPlugin>> =
            vec![Box::new(gitly_backend::plugin())];

        let mut runtime_plan = manifest
            .build_customer_root_runtime_plan_with_extensions_and_customer_plugins_at(
                config,
                auth_package,
                modules,
                extension_packages,
                customer_plugins,
                &self.app_root,
            )
            .context("Gitly runtime build failed")?;
        augment_runtime_plan(&mut runtime_plan.runtime)?;
        let composition = runtime_plan.composition.clone();

        Ok(GitlyBootstrap {
            app_root: self.app_root.clone(),
            config_path,
            manifest,
            composition,
            runtime_plan,
        })
    }

    pub fn validate(&self, config_path: impl AsRef<Path>) -> Result<GitlyLifecycleValidation> {
        validate_workspace_layout(&self.app_root)?;
        let manifest = self.load_manifest()?;
        let (config_path, config) = self.load_platform_config(config_path)?;
        manifest
            .validate_runtime_config_alignment(&config)
            .context("Gitly manifest/config alignment failed")?;

        let auth_package = load_auth_model_package_at(&manifest.auth.package_name, &self.app_root)
            .with_context(|| {
                format!(
                    "failed to load Gitly auth package `{}` from `{}`",
                    manifest.auth.package_name,
                    self.app_root.display()
                )
            })?;
        let manifests = resolve_modules_from_config(&config)
            .context("failed to resolve Gitly modules")?
            .iter()
            .map(|module| module.manifest().clone())
            .collect::<Vec<_>>();
        let composition = manifest
            .compose(&auth_package, &manifests)
            .context("Gitly customer composition failed")?;
        let manual_customer_migration_entries = manual_customer_migration_entries_from_composition(
            &composition,
            &manifest.id.to_string(),
        );

        Ok(GitlyLifecycleValidation {
            app_root: self.app_root.clone(),
            config_path,
            app_id: manifest.id.to_string(),
            module_ids: composition
                .installed_modules
                .iter()
                .map(|module| module.id.to_string())
                .collect(),
            linked_plugin_ids: vec![gitly_backend::plugin().descriptor().id],
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
    ) -> Result<GitlyMigrationApplyReport> {
        let bootstrap = self.build_bootstrap(config_path)?;
        bootstrap.apply_migrations(dry_run)
    }

    pub fn publish_assets(
        &self,
        config_path: impl AsRef<Path>,
    ) -> Result<GitlyAssetPublicationReport> {
        let bootstrap = self.build_bootstrap(config_path)?;
        Ok(bootstrap.asset_publication_report())
    }

    pub fn describe(&self, config_path: impl AsRef<Path>) -> Result<GitlySummary> {
        let manifest = self.load_manifest()?;
        let (config_path, config) = self.load_platform_config(config_path)?;
        manifest
            .validate_runtime_config_alignment(&config)
            .context("Gitly manifest/config alignment failed")?;

        let linked_plugins = vec![gitly_backend::linked_plugin_summary()];
        Ok(GitlySummary {
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

pub fn gitly_community_pulse_demo_sha256(app_root: impl AsRef<Path>) -> Result<String> {
    extensions::compiled_demo_artifact_sha256(app_root.as_ref(), "gitly-community-pulse")
}

pub fn gitly_actions_scheduler_demo_sha256(app_root: impl AsRef<Path>) -> Result<String> {
    extensions::compiled_demo_artifact_sha256(app_root.as_ref(), "gitly-actions-scheduler")
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

impl GitlyBootstrap {
    pub fn server_host<R: SecretResolver>(
        &self,
        resolver: &R,
        cookie_secret: &[u8],
        csrf_secret: &[u8],
    ) -> Result<HttpServerHost> {
        self.runtime_plan
            .runtime
            .server_host(resolver, cookie_secret, csrf_secret)
            .context("failed to build Gitly server host")
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

    pub fn apply_migrations(&self, dry_run: bool) -> Result<GitlyMigrationApplyReport> {
        let executable_plan = &self.runtime_plan.runtime.install_migrations;
        let manual_customer_migration_entries = self.manual_customer_migration_entries();

        if dry_run {
            return Ok(GitlyMigrationApplyReport {
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
            .context("failed to start Gitly migration runtime")?;
        let _runtime_guard = tokio_runtime.enter();
        let client = self
            .runtime_plan
            .runtime
            .data
            .connect_lazy_postgres()
            .context("failed to connect to the Gitly migration database")?;
        let applied_keys = tokio_runtime
            .block_on(async { client.applied_migration_keys().await })
            .context("failed to read applied Gitly migrations")?;
        let pending_plan = pending_migration_plan(executable_plan, &applied_keys)?;
        let pending_steps = pending_plan.ordered_steps().len();
        let executed_statements = if pending_steps == 0 {
            0
        } else {
            let mut registry = MigrationRegistry::new();
            registry
                .register(&pending_plan)
                .context("failed to register Gitly executable migrations")?;
            let batch = self
                .runtime_plan
                .runtime
                .data
                .compile_migrations(&registry)
                .context("failed to compile Gitly executable migrations")?;
            let execution = tokio_runtime
                .block_on(async { client.apply_migrations(&batch).await })
                .context("failed to apply Gitly migrations")?;
            execution.statements_executed
        };

        Ok(GitlyMigrationApplyReport {
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

    pub fn asset_publication_report(&self) -> GitlyAssetPublicationReport {
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

        GitlyAssetPublicationReport {
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
            .context("failed to serve Gitly from the customer runtime")
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
            "Gitly manifest `{}` is missing",
            app_root.join("app.toml").display()
        );
    }
    if !app_root.join("templates").is_dir() {
        bail!(
            "Gitly templates directory `{}` is missing",
            app_root.join("templates").display()
        );
    }
    Ok(())
}

fn resolve_modules_from_config(config: &PlatformConfig) -> Result<Vec<Box<dyn PlatformModule>>> {
    let mut modules: Vec<Box<dyn PlatformModule>> = Vec::new();
    for module_id in &config.modules.enabled {
        if module_id == SHOWCASE_MODULE_ID {
            modules.push(Box::new(GitlyShowcaseModule));
        } else {
            modules.push(
                official_module(module_id)
                    .with_context(|| format!("failed to resolve official module `{module_id}`"))?,
            );
        }
    }
    Ok(modules)
}

fn augment_runtime_plan(runtime: &mut RuntimePlan) -> Result<()> {
    for (route, template) in gitly_page_routes() {
        let route_name = route.name.clone();
        ensure_route(runtime, route)?;
        ensure_handler(runtime, HandlerDefinition::page(route_name, template)?)?;
    }

    for (route, payload) in gitly_api_routes() {
        let route_name = route.name.clone();
        ensure_route(runtime, route)?;
        ensure_handler(runtime, HandlerDefinition::json(route_name, payload)?)?;
    }

    Ok(())
}

fn ensure_route(runtime: &mut RuntimePlan, route: RouteDefinition) -> Result<()> {
    if runtime.http.routes.iter().any(|existing| existing.name == route.name) {
        bail!("duplicate Gitly route `{}`", route.name);
    }
    runtime.http.routes.push(route);
    Ok(())
}

fn ensure_handler(runtime: &mut RuntimePlan, handler: HandlerDefinition) -> Result<()> {
    let route_name = handler.route_name.clone();
    if runtime.handlers.contains_key(&route_name) {
        bail!("duplicate Gitly handler `{route_name}`");
    }
    runtime.handlers.insert(route_name, handler);
    Ok(())
}

fn gitly_page_routes() -> Vec<(RouteDefinition, &'static str)> {
    let pages = [
        ("home", "", "gitly/home"),
        ("explore", "/explore", "gitly/explore"),
        ("repo", "/octocorp/platform-ui", "gitly/repository"),
        ("issues", "/octocorp/platform-ui/issues", "gitly/issues"),
        ("pulls", "/octocorp/platform-ui/pulls", "gitly/pulls"),
        ("actions", "/octocorp/platform-ui/actions", "gitly/actions"),
        ("org", "/orgs/octocorp", "gitly/organization"),
        ("user", "/alexmariner", "gitly/profile"),
        ("search", "/search", "gitly/search"),
    ];
    let mut routes = Vec::new();
    for (code, prefix) in [("en", ""), ("fr", "/fr"), ("de", "/de")] {
        for (name, path, template) in pages {
            let full_path = if path.is_empty() {
                if prefix.is_empty() {
                    "/".to_string()
                } else {
                    prefix.to_string()
                }
            } else if prefix.is_empty() {
                path.to_string()
            } else {
                format!("{prefix}{path}")
            };
            let route = RouteDefinition::new(
                format!("gitly.{code}.{name}"),
                HttpMethod::Get,
                full_path,
            )
            .expect("static Gitly routes should be valid");
            routes.push((route, template));
        }
    }
    routes
}

fn gitly_api_routes() -> Vec<(RouteDefinition, BTreeMap<String, String>)> {
    let definitions = [
        (
            "gitly.api.repository",
            "/api/github/repository",
            gitly_backend::repo_api_payload(),
        ),
        (
            "gitly.api.pulls",
            "/api/github/pulls",
            gitly_backend::pulls_api_payload(),
        ),
        (
            "gitly.api.workflows",
            "/api/github/workflows",
            gitly_backend::workflow_api_payload(),
        ),
        (
            "gitly.api.organization",
            "/api/github/org",
            gitly_backend::organization_api_payload(),
        ),
        (
            "gitly.api.user",
            "/api/github/user",
            gitly_backend::user_api_payload(),
        ),
        (
            "gitly.api.pulse",
            "/api/github/pulse",
            BTreeMap::from([
                ("status".to_string(), "ok".to_string()),
                ("surface".to_string(), "gitly-community-pulse".to_string()),
            ]),
        ),
    ];

    definitions
        .into_iter()
        .map(|(name, path, payload)| {
            (
                RouteDefinition::new(name, HttpMethod::Get, path)
                    .expect("static Gitly API routes should be valid")
                    .with_area(RouteArea::Api),
                payload,
            )
        })
        .collect()
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
            format!("failed to stage pending Gitly migration `{}`", step.id)
        })?;
    }
    Ok(pending)
}
