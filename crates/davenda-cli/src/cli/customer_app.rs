use crate::CliRunError;
use davenda_admin::AdminModule;
use davenda_app::CustomerAppManifest;
use davenda_cms::CmsModule;
use davenda_commerce::CommerceModule;
use davenda_config::PlatformConfig;
use davenda_core::{ModuleManifest, PlatformModule};
use davenda_events::EventsModule;
use davenda_media::MediaModule;
use davenda_memberships::MembershipsModule;
use davenda_ops::OpsModule;
use std::path::{Path, PathBuf};

pub(crate) struct CustomerAppContext {
    pub config: PlatformConfig,
    pub manifest: CustomerAppManifest,
    pub modules: Vec<Box<dyn PlatformModule>>,
    pub module_manifests: Vec<ModuleManifest>,
}

pub(crate) fn load_customer_app_context(
    config_path: impl AsRef<Path>,
) -> Result<CustomerAppContext, CliRunError> {
    let config_path = config_path.as_ref();
    let config = PlatformConfig::from_file(config_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to load platform config from `{}`: {error}",
            config_path.display()
        ))
    })?;

    let app_root = resolve_customer_app_root(config_path, &config.app.name)?;
    let manifest_path = app_root.join("app.toml");
    let manifest = CustomerAppManifest::from_file(&manifest_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to load customer app manifest from `{}`: {error}",
            manifest_path.display()
        ))
    })?;
    manifest
        .validate_runtime_config_alignment(&config)
        .map_err(|error| {
            CliRunError::execution(format!(
                "customer app `{}` does not align with platform config `{}`: {error}",
                manifest.id,
                config_path.display()
            ))
        })?;
    let modules = load_official_modules(&config)?;
    let module_manifests = modules
        .iter()
        .map(|module| module.manifest().clone())
        .collect();

    Ok(CustomerAppContext {
        config,
        manifest,
        modules,
        module_manifests,
    })
}

pub(crate) fn load_official_modules(
    config: &PlatformConfig,
) -> Result<Vec<Box<dyn PlatformModule>>, CliRunError> {
    let mut modules = Vec::with_capacity(config.modules.enabled.len());

    for module_name in &config.modules.enabled {
        let module: Box<dyn PlatformModule> = match module_name.as_str() {
            "admin" => Box::new(AdminModule::new()),
            "commerce" => Box::new(CommerceModule::new()),
            "cms" => Box::new(CmsModule::new()),
            "events" => Box::new(EventsModule::new()),
            "media" => Box::new(MediaModule::new()),
            "memberships" => Box::new(MembershipsModule::new()),
            "ops" => Box::new(OpsModule::new()),
            other => {
                return Err(CliRunError::execution(format!(
                    "unsupported module `{other}` in customer app workflow"
                )));
            }
        };
        modules.push(module);
    }

    Ok(modules)
}

fn resolve_customer_app_root(
    config_path: &Path,
    app_name: &str,
) -> Result<PathBuf, CliRunError> {
    let mut candidates = Vec::new();
    if let Some(parent) = config_path.parent() {
        candidates.push(parent.to_path_buf());
        if let Some(repo_root) = parent.parent() {
            candidates.push(repo_root.join("apps").join(app_name));
            candidates.push(repo_root.to_path_buf());
        }
        candidates.push(parent.join("apps").join(app_name));
    }
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("apps").join(app_name));
        candidates.push(current_dir);
    }

    for candidate in candidates {
        if candidate.join("app.toml").is_file() {
            return Ok(candidate);
        }
    }

    Err(CliRunError::execution(format!(
        "failed to resolve customer app root for `{app_name}` from `{}`; expected a directory containing `app.toml` under one of the checked locations",
        config_path.display()
    )))
}
