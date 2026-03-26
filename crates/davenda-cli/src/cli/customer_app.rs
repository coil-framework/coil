use crate::CliRunError;
use davenda_admin::AdminModule;
use davenda_app::CustomerAppManifest;
use davenda_cms::CmsModule;
use davenda_commerce::{CommerceModule, CommercePaymentsStripeModule};
use davenda_config::PlatformConfig;
use davenda_core::{ModuleManifest, PlatformModule};
use davenda_events::EventsModule;
use davenda_media::MediaModule;
use davenda_memberships::MembershipsModule;
use davenda_ops::OpsModule;
use std::path::{Path, PathBuf};

pub(crate) struct CustomerAppContext {
    pub app_root: PathBuf,
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
        app_root,
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
            "commerce-payments-stripe" => Box::new(CommercePaymentsStripeModule::new()),
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

pub(crate) fn resolve_customer_app_root(
    config_path: &Path,
    app_name: &str,
) -> Result<PathBuf, CliRunError> {
    let mut candidates = Vec::new();
    let current_dir = std::env::current_dir().ok();
    if let Some(parent) = config_path.parent() {
        let parent = if parent.as_os_str().is_empty() {
            current_dir.clone().unwrap_or_else(|| PathBuf::from("."))
        } else {
            parent.to_path_buf()
        };
        candidates.push(parent.clone());
        if let Some(repo_root) = parent.parent() {
            candidates.push(repo_root.join("apps").join(app_name));
            candidates.push(repo_root.to_path_buf());
        }
        candidates.push(parent.join("apps").join(app_name));
    }
    if let Some(current_dir) = current_dir {
        candidates.push(current_dir.join("apps").join(app_name));
        candidates.push(current_dir);
    }

    for candidate in candidates {
        if candidate.join("app.toml").is_file() {
            return candidate.canonicalize().map_err(|error| {
                CliRunError::execution(format!(
                    "failed to canonicalize customer app root `{}`: {error}",
                    candidate.display()
                ))
            });
        }
    }

    Err(CliRunError::execution(format!(
        "failed to resolve customer app root for `{app_name}` from `{}`; expected a directory containing `app.toml` under one of the checked locations",
        config_path.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::resolve_customer_app_root;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn resolve_customer_app_root_uses_current_directory_for_relative_config_in_app_root() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_root = std::env::temp_dir().join(format!("davenda-cli-customer-app-{unique}"));
        let app_root = temp_root.join("apps").join("harbor-shop");
        fs::create_dir_all(&app_root).unwrap();
        fs::write(app_root.join("app.toml"), "id = \"harbor-shop\"\n").unwrap();
        fs::write(
            app_root.join("platform.dev.toml"),
            "[app]\nname = \"harbor-shop\"\n",
        )
        .unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&app_root).unwrap();
        let resolved =
            resolve_customer_app_root(Path::new("platform.dev.toml"), "harbor-shop").unwrap();
        std::env::set_current_dir(original_dir).unwrap();
        let expected = app_root.canonicalize().unwrap();
        let _ = fs::remove_dir_all(&temp_root);

        assert_eq!(resolved, expected);
    }
}
