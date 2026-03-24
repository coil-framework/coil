use crate::CliModelError;
use crate::cli::customer_app::{load_customer_app_context, load_official_modules};
use crate::cli::args::{CliInput, DevServerInvocation, parse};
use crate::cli::auth::AuthExplainResult;
use crate::cli::backend::{AuthExplainBackend, LiveAuthExplainBackend};
use crate::cli::error::CliRunError;
use crate::cli::render::{render_auth_explain, render_command_report};
use crate::registry::CliRuntime;
use crate::{CommandReport, ReportRow};
use davenda_auth::configured_auth_model_package;
use davenda_config::PlatformConfig;
use davenda_import::ImportManifest;
use std::path::{Path, PathBuf};
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
        CliInput::ModuleList {
            output_mode,
            config_path,
        } => {
            let context = load_customer_app_context(&config_path)?;
            let auth_package =
                configured_auth_model_package(context.config.auth.package.clone());
            let composition = context
                .manifest
                .compose(&auth_package, &context.module_manifests)
                .map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to compose customer app `{}` for module listing: {error}",
                        context.manifest.id
                    ))
                })?;
            let report = composition.module_list_report().map_err(|error| {
                CliRunError::execution(format!(
                    "failed to render module list for `{}`: {error}",
                    config_path.display()
                ))
            })?;
            render_command_report(&report, output_mode)
        }
        CliInput::MigratePlan {
            output_mode,
            config_path,
        } => {
            let context = load_customer_app_context(&config_path)?;
            let auth_package =
                configured_auth_model_package(context.config.auth.package.clone());
            let migration_summary =
                context
                    .manifest
                    .migration_summary(auth_package, &context.modules);
            let report = migration_summary.command_report().map_err(|error| {
                CliRunError::execution(format!(
                    "failed to render migration plan for `{}`: {error}",
                    config_path.display()
                ))
            })?;
            render_command_report(&report, output_mode)
        }
        CliInput::ReleaseDoctor {
            output_mode,
            config_path,
        } => {
            let context = load_customer_app_context(&config_path)?;
            let auth_package =
                configured_auth_model_package(context.config.auth.package.clone());
            let report = context
                .manifest
                .release_doctor_with_extensions(
                    &auth_package,
                    &context.module_manifests,
                    &[],
                    Some(&context.config),
                )
                .map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to build release doctor report for `{}`: {error}",
                        config_path.display()
                    ))
                })?
                .command_report()
                .map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to render release doctor report for `{}`: {error}",
                        config_path.display()
                    ))
                })?;
            render_command_report(&report, output_mode)
        }
        CliInput::ImportRun {
            output_mode,
            dry_run,
            invocation,
        } => {
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

            let report = if dry_run {
                plan.command_report().map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to render import plan `{}`: {error}",
                        invocation.manifest_path.display()
                    ))
                })?
            } else {
                let journal_path = import_journal_path(&invocation.manifest_path, &manifest.run_id);
                let execution = plan.execute(&journal_path).map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to execute import manifest `{}`: {error}",
                        invocation.manifest_path.display()
                    ))
                })?;
                execution.command_report().map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to render import execution `{}`: {error}",
                        invocation.manifest_path.display()
                    ))
                })?
            };
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
        "  platform module list [--config <path>] [--json]",
        "  platform migrate plan [--config <path>] [--json]",
        "  platform release doctor [--config <path>] [--json]",
        "  platform import run <manifest-path> [--dry-run] [--json]",
        "",
        "Examples:",
        "  platform dev server --config config/platform.toml",
        "  platform config validate --config config/platform.toml",
        "  platform auth explain --subject user:alice --capability cms.page.publish --resource page:homepage",
        "  platform module list --config config/platform.toml",
        "  platform migrate plan --config config/platform.toml",
        "  platform release doctor --config config/platform.toml",
        "  platform import run imports/wordpress-events.toml",
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
        let modules = load_official_modules(&config)?;
        let builder = RuntimeBuilder::new(
            config.clone(),
            configured_auth_model_package(auth_package_name),
        );
        let mut builder = builder;
        for module in modules {
            builder = builder.with_boxed_module(module);
        }
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

fn import_journal_path(manifest_path: &Path, run_id: &impl std::fmt::Display) -> PathBuf {
    let parent = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    parent
        .join(".davenda")
        .join("import-runs")
        .join(format!("{run_id}.json"))
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
    use std::time::{SystemTime, UNIX_EPOCH};

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

    const CUSTOMER_APP_MANIFEST: &str = r#"
[app]
name = "showcase-events"
display_name = "Showcase Events"

[domains]
canonical = "example.com"

[i18n]
default_locale = "en"
supported_locales = ["en"]

[theme]
active = "storefront"
template_namespaces = ["pages", "layouts"]

[auth]
package = "platform-default-auth"

[modules]
enabled = ["cms"]

[[customer_migrations]]
id = "site-navigation"
order = 10
description = "Migrate the customer navigation structure"
"#;

    fn customer_app_fixture() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("davenda-cli-workflow-{suffix}"));
        let config_dir = root.join("config");
        let app_root = root.join("apps").join("showcase-events");
        let templates_root = app_root.join("templates").join("pages");

        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&templates_root).unwrap();
        fs::write(config_dir.join("platform.toml"), DISABLED_EXPLAIN_CONFIG).unwrap();
        fs::write(app_root.join("app.toml"), CUSTOMER_APP_MANIFEST).unwrap();
        fs::write(
            templates_root.join("home.html"),
            "<html><body><main>Showcase Events</main></body></html>",
        )
        .unwrap();

        config_dir.join("platform.toml")
    }

    fn harbor_shop_platform_config() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/harbor-shop/platform.toml")
            .canonicalize()
            .expect("sample customer app config exists")
    }

    #[test]
    fn run_from_args_returns_usage_for_help() {
        let rendered = run_from_args(["--help".to_string()]).unwrap();
        assert!(rendered.contains("platform config validate [--config <path>]"));
        assert!(rendered.contains("platform auth explain [--config <path>]"));
        assert!(rendered.contains("platform module list [--config <path>]"));
        assert!(rendered.contains("platform migrate plan [--config <path>]"));
        assert!(rendered.contains("platform release doctor [--config <path>]"));
        assert!(rendered.contains("platform import run <manifest-path> [--dry-run]"));
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
    fn run_from_args_executes_and_resumes_import_runs_from_a_manifest() {
        let manifest_path = PathBuf::from("/tmp/davenda-cli-import-execute.toml");
        let journal_path = import_journal_path(
            &manifest_path,
            &davenda_import::ImportRunId::new("wordpress-execute").unwrap(),
        );
        if journal_path.exists() {
            fs::remove_file(&journal_path).unwrap();
        }
        if let Some(parent) = journal_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }

        fs::write(
            &manifest_path,
            r#"
run_id = "wordpress-execute"
source_system = "wordpress"
snapshot_at = "2026-03-19T00:00:00Z"
customer_app_id = "harbor-shop"

[[importers]]
id = "pages"
phase = 10
resource_kind = "page"
description = "Import pages"
"#,
        )
        .unwrap();

        let first = run_from_args([
            "import".to_string(),
            "run".to_string(),
            manifest_path.display().to_string(),
        ])
        .unwrap();
        assert!(first.contains("Executed import run `wordpress-execute`"));
        assert!(first.contains("executed"));
        assert!(journal_path.is_file());

        let second = run_from_args([
            "import".to_string(),
            "run".to_string(),
            manifest_path.display().to_string(),
        ])
        .unwrap();
        assert!(second.contains("Resumed import run `wordpress-execute`"));
        assert!(second.contains("skipped_completed"));
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

    #[test]
    fn run_from_args_renders_module_list_from_a_customer_app_runtime_plan() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "module".to_string(),
            "list".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("module list"));
        assert!(rendered.contains("cms"));
    }

    #[test]
    fn run_from_args_renders_migration_plan_from_a_customer_app_runtime_plan() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "migrate".to_string(),
            "plan".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("migrate plan"));
        assert!(rendered.contains("customer_app:showcase-events"));
    }

    #[test]
    fn run_from_args_renders_release_doctor_from_a_customer_app_runtime_plan() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "release".to_string(),
            "doctor".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("release doctor"));
        assert!(rendered.contains("showcase-events"));
    }

    #[test]
    fn run_from_args_renders_sample_customer_app_release_doctor_without_ops_blocker() {
        let rendered = run_from_args([
            "release".to_string(),
            "doctor".to_string(),
            "--config".to_string(),
            harbor_shop_platform_config().display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("release doctor"));
        assert!(!rendered.contains("module.ops.missing"));
    }

    #[test]
    fn run_from_args_renders_sample_customer_app_module_list_with_ops_installed() {
        let rendered = run_from_args([
            "module".to_string(),
            "list".to_string(),
            "--config".to_string(),
            harbor_shop_platform_config().display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("module list"));
        assert!(rendered.contains("ops"));
    }
}
