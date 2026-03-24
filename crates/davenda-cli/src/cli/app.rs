use crate::CliModelError;
use crate::cli::args::{CliInput, DevServerInvocation, parse};
use crate::cli::auth::AuthExplainResult;
use crate::cli::backend::{AuthExplainBackend, LiveAuthExplainBackend};
use crate::cli::error::CliRunError;
use crate::cli::render::{render_auth_explain, render_command_report};
use crate::registry::CliRuntime;
use crate::{CommandReport, ReportRow};
use davenda_admin::AdminModule;
use davenda_auth::{AuthModelPackage, configured_auth_model_package};
use davenda_cms::CmsModule;
use davenda_commerce::CommerceModule;
use davenda_config::PlatformConfig;
use davenda_events::EventsModule;
use davenda_import::ImportManifest;
use davenda_media::MediaModule;
use davenda_memberships::MembershipsModule;
use davenda_runtime::{EnvironmentSecretResolver, RuntimeBuilder};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliApplication {
    runtime: CliRuntime,
}

impl CliApplication {
    pub fn new(customer_app: impl Into<String>) -> Result<Self, CliModelError> {
        Ok(Self {
            runtime: CliRuntime::baseline(customer_app)?,
        })
    }

    pub fn runtime(&self) -> &CliRuntime {
        &self.runtime
    }
}

pub fn run_from_args(args: impl IntoIterator<Item = String>) -> Result<String, CliRunError> {
    let input = parse(args)?;
    match input {
        CliInput::Help => Ok(usage()),
        CliInput::DevServer { invocation } => {
            run_dev_server(&invocation)?;
            Ok(String::new())
        }
        CliInput::ConfigValidate {
            output_mode,
            invocation,
        } => {
            let config = PlatformConfig::from_file(&invocation.config_path).map_err(|error| {
                CliRunError::execution(format!(
                    "failed to load platform config from `{}`: {error}",
                    invocation.config_path.display()
                ))
            })?;

            let mut report = CommandReport::new(
                ["config", "validate"],
                format!(
                    "Validated effective platform configuration `{}`",
                    invocation.config_path.display()
                ),
            )
            .map_err(|error| {
                CliRunError::execution(format!("failed to build config report: {error}"))
            })?
            .with_columns([
                "app",
                "environment",
                "auth_package",
                "modules",
                "deployment",
            ])
            .map_err(|error| {
                CliRunError::execution(format!("failed to build config report: {error}"))
            })?;
            report.push_row(
                ReportRow::new()
                    .with_cell("app", config.app.name.clone())
                    .map_err(|error| {
                        CliRunError::execution(format!("failed to build config report: {error}"))
                    })?
                    .with_cell("environment", environment_label(config.app.environment))
                    .map_err(|error| {
                        CliRunError::execution(format!("failed to build config report: {error}"))
                    })?
                    .with_cell("auth_package", config.auth.package.clone())
                    .map_err(|error| {
                        CliRunError::execution(format!("failed to build config report: {error}"))
                    })?
                    .with_cell(
                        "modules",
                        if config.modules.enabled.is_empty() {
                            "none".to_string()
                        } else {
                            config.modules.enabled.join(",")
                        },
                    )
                    .map_err(|error| {
                        CliRunError::execution(format!("failed to build config report: {error}"))
                    })?
                    .with_cell(
                        "deployment",
                        storage_deployment_label(config.storage.deployment),
                    )
                    .map_err(|error| {
                        CliRunError::execution(format!("failed to build config report: {error}"))
                    })?,
            );

            render_command_report(&report, output_mode)
        }
        CliInput::AuthExplain {
            output_mode,
            invocation,
        } => {
            // Auth explain is deployment-configured and always goes through the live backend.
            let config = PlatformConfig::from_file(&invocation.config_path).map_err(|error| {
                CliRunError::execution(format!(
                    "failed to load platform config from `{}`: {error}",
                    invocation.config_path.display()
                ))
            })?;
            let backend = LiveAuthExplainBackend::from_config(&config)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to start the CLI async runtime: {error}"
                    ))
                })?;
            let explanation = runtime.block_on(async { backend.explain(&invocation).await })?;
            let result = AuthExplainResult {
                invocation,
                explanation,
            };
            render_auth_explain(&result, output_mode)
        }
        CliInput::ImportRun {
            output_mode,
            dry_run,
            invocation,
        } => {
            if !dry_run {
                return Err(CliRunError::usage(
                    "`import run` only supports `--dry-run` in this build",
                ));
            }

            let manifest =
                ImportManifest::from_file(&invocation.manifest_path).map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to load import manifest from `{}`: {error}",
                        invocation.manifest_path.display()
                    ))
                })?;
            let plan = manifest.plan().map_err(|error| {
                CliRunError::execution(format!(
                    "failed to plan import manifest `{}`: {error}",
                    invocation.manifest_path.display()
                ))
            })?;

            let report = plan.command_report().map_err(|error| {
                CliRunError::execution(format!(
                    "failed to render import plan `{}`: {error}",
                    invocation.manifest_path.display()
                ))
            })?;
            render_command_report(&report, output_mode)
        }
    }
}

pub fn run_from_env() -> i32 {
    match run_from_args(std::env::args().skip(1)) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

fn usage() -> String {
    [
        "Usage:",
        "  platform dev server [--config <path>]",
        "  platform config validate [--config <path>] [--json]",
        "  platform auth explain [--config <path>] --subject <subject> --capability <capability> --resource <namespace:id> [--json]",
        "  platform import run <manifest-path> --dry-run [--json]",
        "",
        "Examples:",
        "  platform dev server --config config/platform.toml",
        "  platform config validate --config config/platform.toml",
        "  platform auth explain --subject user:alice --capability cms.page.publish --resource page:homepage",
        "  platform import run imports/wordpress-events.toml --dry-run",
        "",
        "Environment:",
        "  DAVENDA_COOKIE_SECRET and DAVENDA_CSRF_SECRET are required for `dev server`",
        "  DATABASE_URL and OBJECT_STORE_URL are required by `config/platform.toml`",
    ]
    .join("\n")
}

fn run_dev_server(invocation: &DevServerInvocation) -> Result<(), CliRunError> {
    let config = PlatformConfig::from_file(&invocation.config_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to load platform config from `{}`: {error}",
            invocation.config_path.display()
        ))
    })?;

    let cookie_secret = read_runtime_secret("DAVENDA_COOKIE_SECRET")?;
    let csrf_secret = read_runtime_secret("DAVENDA_CSRF_SECRET")?;
    let bind = config.server.bind.clone();
    let auth_package_name = config.auth.package.clone();
    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| CliRunError::execution(format!("failed to start runtime: {error}")))?;

    tokio_runtime.block_on(async move {
        let builder = RuntimeBuilder::new(
            config.clone(),
            configured_auth_model_package(auth_package_name),
        );
        let builder = install_official_modules(builder, &config)?;
        let plan = builder.build().map_err(|error| {
            CliRunError::execution(format!("failed to build runtime plan: {error}"))
        })?;
        let server = plan
            .server_host(
                &EnvironmentSecretResolver,
                cookie_secret.as_bytes(),
                csrf_secret.as_bytes(),
            )
            .map_err(|error| {
                CliRunError::execution(format!("failed to build dev server host: {error}"))
            })?;
        let listener = tokio::net::TcpListener::bind(&bind)
            .await
            .map_err(|error| {
                CliRunError::execution(format!("failed to bind dev server on `{bind}`: {error}"))
            })?;

        println!("Serving `{}` on http://{bind}", plan.config.app.name);
        server.serve(listener).await.map_err(|error| {
            CliRunError::execution(format!("dev server stopped unexpectedly: {error}"))
        })
    })
}

fn read_runtime_secret(var: &str) -> Result<String, CliRunError> {
    std::env::var(var).map_err(|_| {
        CliRunError::execution(format!(
            "missing `{var}`; set it before starting `dev server`"
        ))
    })
}

fn install_official_modules<P>(
    mut builder: davenda_runtime::RuntimeBuilder<P>,
    config: &PlatformConfig,
) -> Result<davenda_runtime::RuntimeBuilder<P>, CliRunError>
where
    P: AuthModelPackage + 'static,
{
    for module_name in &config.modules.enabled {
        builder = match module_name.as_str() {
            "admin" => builder.with_module(AdminModule::new()),
            "commerce" => builder.with_module(CommerceModule::new()),
            "cms" => builder.with_module(CmsModule::new()),
            "events" => builder.with_module(EventsModule::new()),
            "media" => builder.with_module(MediaModule::new()),
            "memberships" => builder.with_module(MembershipsModule::new()),
            other => {
                return Err(CliRunError::execution(format!(
                    "unsupported module `{other}` in `dev server` config"
                )));
            }
        };
    }

    Ok(builder)
}

fn environment_label(environment: davenda_config::Environment) -> &'static str {
    match environment {
        davenda_config::Environment::Development => "development",
        davenda_config::Environment::Staging => "staging",
        davenda_config::Environment::Production => "production",
    }
}

fn storage_deployment_label(deployment: davenda_config::StorageDeployment) -> &'static str {
    match deployment {
        davenda_config::StorageDeployment::Distributed => "distributed",
        davenda_config::StorageDeployment::SingleNode => "single_node",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    const DISABLED_EXPLAIN_CONFIG: &str = r#"
[app]
name = "showcase-events"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = []

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
default_class = "public_upload"
deployment = "single_node"
single_node_escape_hatch = "explicit_single_node"
local_root = "/tmp/davenda-cli"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en"
supported_locales = ["en"]
fallback_locale = "en"
localized_routes = false

[seo]
canonical_host = "example.com"
emit_json_ld = true
sitemap_enabled = true

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 1

[modules]
enabled = ["cms"]

[wasm]
directory = "wasm"
default_time_limit_ms = 1000
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = false
tracing = false

[assets]
publish_manifest = false
"#;

    #[test]
    fn run_from_args_returns_usage_for_help() {
        let rendered = run_from_args(["--help".to_string()]).unwrap();
        assert!(rendered.contains("platform config validate [--config <path>]"));
        assert!(rendered.contains("platform auth explain [--config <path>]"));
        assert!(rendered.contains("platform import run <manifest-path> --dry-run"));
    }

    #[test]
    fn run_from_args_reports_disabled_auth_explain_from_live_config() {
        let config_path = PathBuf::from("/tmp/davenda-cli-disabled.toml");
        fs::write(&config_path, DISABLED_EXPLAIN_CONFIG).unwrap();

        let error = run_from_args([
            "auth".to_string(),
            "explain".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--capability".to_string(),
            "cms.page.read".to_string(),
            "--resource".to_string(),
            "page:homepage".to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn run_from_args_uses_the_live_backend_when_deployment_enables_auth_explain() {
        let config_path = PathBuf::from("/tmp/davenda-cli-enabled.toml");
        let enabled_config = DISABLED_EXPLAIN_CONFIG
            .replace("explain_api = false", "explain_api = true")
            .replace(
                "package = \"platform-default-auth\"",
                "package = \"platform-extended-auth\"",
            );
        fs::write(&config_path, enabled_config).unwrap();

        let error = run_from_args([
            "auth".to_string(),
            "explain".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--capability".to_string(),
            "cms.page.read".to_string(),
            "--resource".to_string(),
            "page:homepage".to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 1);
        let message = error.to_string();
        assert!(
            message.contains("failed to initialize the live auth explain backend")
                || message.contains("failed to build the auth explanation"),
            "{message}"
        );
        assert!(!message.contains("auth explain API is disabled"));
    }

    #[test]
    fn run_from_args_plans_import_runs_from_a_manifest() {
        let manifest_path = PathBuf::from("/tmp/davenda-cli-import.toml");
        fs::write(
            &manifest_path,
            r#"
run_id = "wordpress-events"
source_system = "wordpress"
snapshot_at = "2026-03-19T00:00:00Z"
customer_app_id = "harbor-shop"
modules = ["cms", "events"]

[[importers]]
id = "users"
phase = 10
resource_kind = "user"
description = "Import users"

[[importers]]
id = "events"
phase = 20
resource_kind = "event"
description = "Import events"
dependencies = ["users"]
"#,
        )
        .unwrap();

        let rendered = run_from_args([
            "import".to_string(),
            "run".to_string(),
            manifest_path.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("Planned import run `wordpress-events`"));
        assert!(rendered.contains("users"));
        assert!(rendered.contains("events"));
    }

    #[test]
    fn run_from_args_validates_config_and_renders_a_report() {
        let config_path = PathBuf::from("/tmp/davenda-cli-config-validate.toml");
        fs::write(&config_path, DISABLED_EXPLAIN_CONFIG).unwrap();

        let rendered = run_from_args([
            "config".to_string(),
            "validate".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("config validate"));
        assert!(rendered.contains("Validated effective platform configuration"));
        assert!(rendered.contains("showcase-events"));
    }
}
