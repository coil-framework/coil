use crate::cli::args::{parse, CliInput};
use crate::cli::auth::AuthExplainResult;
use crate::cli::backend::{AuthExplainBackend, LiveAuthExplainBackend};
use crate::cli::error::CliRunError;
use crate::cli::render::{render_auth_explain, render_command_report};
use crate::registry::CliRuntime;
use crate::CliModelError;
use davenda_import::ImportManifest;
use davenda_config::PlatformConfig;

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
            confirmed,
            invocation,
        } => {
            if !dry_run && !confirmed {
                return Err(CliRunError::usage(
                    "`import run` requires `--dry-run` while planning or `--yes` before execution",
                ));
            }

            let manifest = ImportManifest::from_file(&invocation.manifest_path).map_err(|error| {
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

            if !dry_run {
                return Err(CliRunError::execution(
                    "live import execution requires customer-app importer bindings; `davenda-cli` currently supports planned validation via `import run --dry-run`",
                ));
            }

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
        "  platform auth explain [--config <path>] --subject <subject> --capability <capability> --resource <namespace:id> [--json]",
        "  platform import run <manifest-path> --dry-run [--json]",
        "",
        "Examples:",
        "  platform auth explain --subject user:alice --capability cms.page.publish --resource page:homepage",
        "  platform import run imports/wordpress-events.toml --dry-run",
    ]
    .join("\n")
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
enabled = []

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
            )
            .replace("[modules]\nenabled = []", "[modules]\nenabled = [\"cms\"]")
            .replace(
                "[cache]\nl1 = \"moka\"",
                "[cache]\nl1 = \"moka\"\nl2 = \"redis\"",
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
        assert!(error
            .to_string()
            .contains("failed to build the auth explanation"));
        assert!(!error.to_string().contains("not registered"));
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
}
