use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use harbor_shop_app::HarborShopWorkspace;
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "harbor-shop")]
#[command(about = "Harbor Shop customer workspace binary")]
struct Cli {
    #[arg(long)]
    app_root: Option<PathBuf>,

    #[arg(long, default_value = "platform.dev.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Describe,
    Validate,
    Assets {
        #[command(subcommand)]
        command: AssetsCommand,
    },
    Migrate {
        #[command(subcommand)]
        command: MigrateCommand,
    },
    Serve {
        #[arg(long)]
        bind: Option<String>,
    },
    Up {
        #[arg(long)]
        bind: Option<String>,
    },
    LinkedBackend {
        #[command(subcommand)]
        command: LinkedBackendCommand,
    },
}

#[derive(Debug, Subcommand)]
enum AssetsCommand {
    Publish,
}

#[derive(Debug, Subcommand)]
enum MigrateCommand {
    Apply {
        #[arg(long)]
        dry_run: bool,

        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
enum LinkedBackendCommand {
    Describe,
    Demo,
    LoyaltyPreview {
        #[arg(long)]
        request: Option<PathBuf>,
    },
    OrderReview {
        #[arg(long)]
        request: Option<PathBuf>,
    },
    CrmContact {
        #[arg(long)]
        request: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let workspace = match cli.app_root {
        Some(path) => HarborShopWorkspace::at(path)?,
        None => HarborShopWorkspace::default()?,
    };
    match cli.command {
        Command::Describe => describe(&workspace, &cli.config),
        Command::Validate => validate(&workspace, &cli.config),
        Command::Assets { command } => assets(&workspace, &cli.config, command),
        Command::Migrate { command } => migrate(&workspace, &cli.config, command),
        Command::Serve { bind } => serve(&workspace, &cli.config, bind),
        Command::Up { bind } => up(&workspace, &cli.config, bind),
        Command::LinkedBackend { command } => linked_backend(&workspace, command),
    }
}

fn describe(workspace: &HarborShopWorkspace, config_path: &PathBuf) -> Result<()> {
    let summary = workspace.describe(config_path)?;
    println!("Harbor Shop customer workspace");
    println!("app root: {}", summary.app_root.display());
    println!("config: {}", summary.config_path.display());
    println!("app id: {}", summary.manifest.id);
    println!("auth package: {}", summary.manifest.auth.package_name);
    println!(
        "modules: {}",
        summary
            .manifest
            .modules
            .iter()
            .map(|module| module.id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("linked plugins: {}", summary.linked_plugin_ids.join(", "));
    for plugin in &summary.linked_plugins {
        let hooks = plugin
            .hook_kinds
            .iter()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "  - {} ({}) [{}]",
            plugin.display_name, plugin.id, plugin.version
        );
        println!("    hooks: {hooks}");
        if let Some(documentation_url) = plugin.documentation_url.as_deref() {
            println!("    docs: {documentation_url}");
        }
    }
    println!("server bind: {}", summary.config.server.bind);
    Ok(())
}

fn validate(workspace: &HarborShopWorkspace, config_path: &PathBuf) -> Result<()> {
    let validation = workspace.validate(config_path)?;
    println!("Harbor Shop validation passed");
    println!("app root: {}", validation.app_root.display());
    println!("config: {}", validation.config_path.display());
    println!("app id: {}", validation.app_id);
    println!("modules: {}", validation.module_ids.join(", "));
    println!(
        "linked plugins: {}",
        validation.linked_plugin_ids.join(", ")
    );
    println!("route surfaces: {}", validation.route_surface_count);
    println!("jobs: {}", validation.job_count);
    println!(
        "migration contracts: {}",
        validation.migration_contract_count
    );
    if validation.manual_customer_migration_entries.is_empty() {
        println!("manual customer migrations: none");
    } else {
        println!(
            "manual customer migrations: {}",
            validation.manual_customer_migration_entries.len()
        );
    }
    Ok(())
}

fn assets(
    workspace: &HarborShopWorkspace,
    config_path: &PathBuf,
    command: AssetsCommand,
) -> Result<()> {
    match command {
        AssetsCommand::Publish => {
            let publication = workspace.publish_assets(config_path)?;
            println!("Harbor Shop asset publication");
            println!("config: {}", publication.config_path.display());
            println!("app id: {}", publication.app_id);
            println!("asset roots: {}", publication.asset_roots.join(", "));
            if publication.published {
                println!(
                    "published {} asset entries with {} storage writes",
                    publication.asset_entries, publication.writes
                );
                if let Some(release_id) = publication.release_id {
                    println!("release: {release_id}");
                }
            } else {
                println!("published 0 asset entries with 0 storage writes");
                println!("reason: asset publication is disabled or no asset roots are configured");
            }
            Ok(())
        }
    }
}

fn migrate(
    workspace: &HarborShopWorkspace,
    config_path: &PathBuf,
    command: MigrateCommand,
) -> Result<()> {
    match command {
        MigrateCommand::Apply { dry_run, yes } => {
            if !dry_run && !yes {
                bail!("`harbor-shop migrate apply` requires `--yes` unless `--dry-run` is used");
            }
            let report = workspace.migrate_apply(config_path, dry_run)?;
            println!("Harbor Shop migration apply");
            println!("config: {}", report.config_path.display());
            println!("app id: {}", report.app_id);
            if report.dry_run {
                println!(
                    "planned {} pending executable migration steps",
                    report.pending_steps
                );
            } else {
                println!(
                    "applied {} pending executable migration steps with {} SQL statements",
                    report.pending_steps, report.executed_statements
                );
            }
            println!("total executable steps: {}", report.executable_steps);
            println!("already applied: {}", report.already_applied_steps);
            if report.manual_customer_migration_entries.is_empty() {
                println!("manual customer migrations: none");
            } else {
                println!(
                    "manual customer migrations: {}",
                    report.manual_customer_migration_entries.len()
                );
            }
            Ok(())
        }
    }
}

fn serve(
    workspace: &HarborShopWorkspace,
    config_path: &PathBuf,
    bind: Option<String>,
) -> Result<()> {
    workspace
        .build_bootstrap(config_path)?
        .serve_from_env(bind)
        .context("Harbor Shop server exited with an error")
}

fn up(workspace: &HarborShopWorkspace, config_path: &PathBuf, bind: Option<String>) -> Result<()> {
    let bootstrap = workspace.build_bootstrap(config_path)?;
    println!("Harbor Shop lifecycle bootstrap");
    println!("app root: {}", bootstrap.app_root.display());
    println!("config: {}", bootstrap.config_path.display());
    println!("app id: {}", bootstrap.manifest.id);
    println!("modules: {}", bootstrap.module_ids().join(", "));
    println!(
        "linked plugins: {}",
        bootstrap.linked_plugin_ids().join(", ")
    );

    let assets = bootstrap.asset_publication_report();
    if assets.published {
        println!(
            "published {} asset entries with {} storage writes",
            assets.asset_entries, assets.writes
        );
    } else {
        println!("asset publication skipped");
    }

    let migrations = bootstrap.apply_migrations(false)?;
    if migrations.pending_steps == 0 {
        println!("migrations: no pending executable steps");
    } else {
        println!(
            "migrations: applied {} pending executable steps with {} SQL statements",
            migrations.pending_steps, migrations.executed_statements
        );
    }

    bootstrap
        .serve_from_env(bind)
        .context("Harbor Shop lifecycle bootstrap failed while serving")
}

fn linked_backend(workspace: &HarborShopWorkspace, command: LinkedBackendCommand) -> Result<()> {
    let output = match command {
        LinkedBackendCommand::Describe => linked_backend_describe_output(),
        LinkedBackendCommand::Demo => linked_backend_demo_output(workspace)?,
        LinkedBackendCommand::LoyaltyPreview { request } => {
            let request_path =
                linked_backend_request_path(workspace, request, "loyalty-preview.json");
            let request: harbor_shop_backend::LoyaltyPreviewRequest =
                read_json_file(&request_path)?;
            render_json(&harbor_shop_backend::plugin().preview_loyalty(&request))?
        }
        LinkedBackendCommand::OrderReview { request } => {
            let request_path = linked_backend_request_path(workspace, request, "order-review.json");
            let request: harbor_shop_backend::OrderReviewRequest = read_json_file(&request_path)?;
            render_json(&harbor_shop_backend::plugin().review_checkout_order(&request))?
        }
        LinkedBackendCommand::CrmContact { request } => {
            let request_path =
                linked_backend_request_path(workspace, request, "contact-updated.json");
            let request: harbor_shop_backend::CrmContactUpdate = read_json_file(&request_path)?;
            render_json(&harbor_shop_backend::plugin().route_crm_contact_update(&request))?
        }
    };
    println!("{output}");
    Ok(())
}

fn linked_backend_describe_output() -> String {
    let summary = harbor_shop_backend::linked_plugin_summary();
    let hooks = summary
        .hook_kinds
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Harbor Shop linked backend\nplugin id: {}\ndisplay name: {}\nversion: {}\ndocumentation: {}\nregistered hooks: {}\nreal demo surfaces:\n- checkout review hooks in the customer runtime\n- verified webhook hooks in the customer runtime\n- direct customer-workspace demo commands via `harbor-shop linked-backend ...`",
        summary.id,
        summary.display_name,
        summary.version,
        summary
            .documentation_url
            .unwrap_or_else(|| "none".to_string()),
        hooks,
    )
}

fn linked_backend_demo_output(workspace: &HarborShopWorkspace) -> Result<String> {
    let loyalty_request: harbor_shop_backend::LoyaltyPreviewRequest = read_json_file(
        &linked_backend_request_path(workspace, None, "loyalty-preview.json"),
    )?;
    let order_request: harbor_shop_backend::OrderReviewRequest = read_json_file(
        &linked_backend_request_path(workspace, None, "order-review.json"),
    )?;
    let crm_request: harbor_shop_backend::CrmContactUpdate = read_json_file(
        &linked_backend_request_path(workspace, None, "contact-updated.json"),
    )?;
    let backend = harbor_shop_backend::plugin();

    Ok(format!(
        "{}\n\nloyalty preview sample:\n{}\n\norder review sample:\n{}\n\ncrm contact sample:\n{}",
        linked_backend_describe_output(),
        render_json(&backend.preview_loyalty(&loyalty_request))?,
        render_json(&backend.review_checkout_order(&order_request))?,
        render_json(&backend.route_crm_contact_update(&crm_request))?,
    ))
}

fn linked_backend_request_path(
    workspace: &HarborShopWorkspace,
    override_path: Option<PathBuf>,
    default_file_name: &str,
) -> PathBuf {
    match override_path {
        Some(path) => workspace.resolve_path(path),
        None => workspace
            .app_root()
            .join("backend/harbor-loyalty-backend/requests")
            .join(default_file_name),
    }
}

fn read_json_file<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Result<T> {
    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read linked backend request `{}`", path.display()))?;
    serde_json::from_str(&input).with_context(|| {
        format!(
            "failed to parse linked backend request `{}`",
            path.display()
        )
    })
}

fn render_json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string_pretty(value).context("failed to render linked backend demo output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_backend_demo_uses_checked_in_customer_workspace_requests() {
        let workspace = HarborShopWorkspace::default().unwrap();
        let output = linked_backend_demo_output(&workspace).unwrap();

        assert!(output.contains("Harbor Shop linked backend"), "{output}");
        assert!(
            output.contains("registered hooks: checkout, verified_webhook"),
            "{output}"
        );
        assert!(output.contains("\"segment\": \"harbor-vip\""), "{output}");
        assert!(
            output.contains("\"assigned_queue\": \"ops-manual-review\""),
            "{output}"
        );
        assert!(
            output.contains(
                "Gold high-value order: route to concierge packing and same-day follow-up."
            ),
            "{output}"
        );
    }

    #[test]
    fn linked_backend_request_path_defaults_to_checked_in_request_files() {
        let workspace = HarborShopWorkspace::default().unwrap();
        let path = linked_backend_request_path(&workspace, None, "order-review.json");

        assert!(
            path.ends_with(
                "apps/harbor-shop/backend/harbor-loyalty-backend/requests/order-review.json"
            ),
            "{}",
            path.display()
        );
    }

    #[test]
    fn linked_backend_describe_output_reports_registered_hook_summary() {
        let output = linked_backend_describe_output();

        assert!(
            output.contains("plugin id: harbor-shop-backend"),
            "{output}"
        );
        assert!(
            output.contains("display name: Harbor Shop Linked Backend"),
            "{output}"
        );
        assert!(
            output.contains("registered hooks: checkout, verified_webhook"),
            "{output}"
        );
        assert!(
            output.contains("documentation: apps/harbor-shop/backend/README.md"),
            "{output}"
        );
    }

    #[test]
    fn cli_accepts_customer_owned_lifecycle_commands() {
        let cli = Cli::try_parse_from([
            "harbor-shop",
            "--config",
            "platform.dev.toml",
            "migrate",
            "apply",
            "--dry-run",
        ])
        .expect("lifecycle migrate command should parse");

        assert!(matches!(
            cli.command,
            Command::Migrate {
                command: MigrateCommand::Apply {
                    dry_run: true,
                    yes: false
                }
            }
        ));

        let cli = Cli::try_parse_from(["harbor-shop", "assets", "publish"])
            .expect("asset publication command should parse");
        assert!(matches!(
            cli.command,
            Command::Assets {
                command: AssetsCommand::Publish
            }
        ));

        let cli = Cli::try_parse_from(["harbor-shop", "validate"])
            .expect("validate command should parse");
        assert!(matches!(cli.command, Command::Validate));

        let cli = Cli::try_parse_from(["harbor-shop", "up"])
            .expect("lifecycle bootstrap command should parse");
        assert!(matches!(cli.command, Command::Up { .. }));
    }
}
