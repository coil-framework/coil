use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use octohub_app::OctohubWorkspace;

#[derive(Debug, Parser)]
#[command(name = "octohub")]
#[command(about = "OctoHub customer workspace binary")]
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
    ExtensionChecksums,
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
    Repository,
    Pulls,
    Workflows,
    Organization,
    User,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let workspace = match cli.app_root {
        Some(path) => OctohubWorkspace::at(path)?,
        None => OctohubWorkspace::default()?,
    };

    match cli.command {
        Command::Describe => describe(&workspace, &cli.config),
        Command::Validate => validate(&workspace, &cli.config),
        Command::Assets { command } => assets(&workspace, &cli.config, command),
        Command::Migrate { command } => migrate(&workspace, &cli.config, command),
        Command::Serve { bind } => serve(&workspace, &cli.config, bind),
        Command::Up { bind } => up(&workspace, &cli.config, bind),
        Command::ExtensionChecksums => extension_checksums(&workspace),
        Command::LinkedBackend { command } => linked_backend(command),
    }
}

fn describe(workspace: &OctohubWorkspace, config_path: &PathBuf) -> Result<()> {
    let summary = workspace.describe(config_path)?;
    println!("OctoHub customer workspace");
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

fn validate(workspace: &OctohubWorkspace, config_path: &PathBuf) -> Result<()> {
    let validation = workspace.validate(config_path)?;
    println!("OctoHub validation passed");
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

fn assets(workspace: &OctohubWorkspace, config_path: &PathBuf, command: AssetsCommand) -> Result<()> {
    match command {
        AssetsCommand::Publish => {
            let publication = workspace.publish_assets(config_path)?;
            println!("OctoHub asset publication");
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

fn migrate(workspace: &OctohubWorkspace, config_path: &PathBuf, command: MigrateCommand) -> Result<()> {
    match command {
        MigrateCommand::Apply { dry_run, yes } => {
            if !dry_run && !yes {
                bail!("`octohub migrate apply` requires `--yes` unless `--dry-run` is used");
            }
            let report = workspace.migrate_apply(config_path, dry_run)?;
            println!("OctoHub migration apply");
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
            Ok(())
        }
    }
}

fn serve(workspace: &OctohubWorkspace, config_path: &PathBuf, bind: Option<String>) -> Result<()> {
    let bootstrap = workspace.build_bootstrap(config_path)?;
    bootstrap.serve_from_env(bind)
}

fn up(workspace: &OctohubWorkspace, config_path: &PathBuf, bind: Option<String>) -> Result<()> {
    let validation = workspace.validate(config_path)?;
    println!(
        "validated {} routes and {} jobs before serving",
        validation.route_surface_count, validation.job_count
    );
    serve(workspace, config_path, bind)
}

fn extension_checksums(workspace: &OctohubWorkspace) -> Result<()> {
    println!(
        "octohub-community-pulse {}",
        octohub_app::octohub_community_pulse_demo_sha256(workspace.app_root())?
    );
    println!(
        "octohub-actions-scheduler {}",
        octohub_app::octohub_actions_scheduler_demo_sha256(workspace.app_root())?
    );
    Ok(())
}

fn linked_backend(command: LinkedBackendCommand) -> Result<()> {
    match command {
        LinkedBackendCommand::Describe => {
            let summary = octohub_backend::linked_plugin_summary();
            println!("OctoHub linked backend");
            println!("id: {}", summary.id);
            println!("display name: {}", summary.display_name);
            println!("version: {}", summary.version);
            println!(
                "hooks: {}",
                summary
                    .hook_kinds
                    .iter()
                    .map(|kind| kind.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        LinkedBackendCommand::Repository => {
            println!("{:#?}", octohub_backend::repository());
        }
        LinkedBackendCommand::Pulls => {
            println!("{:#?}", octohub_backend::pull_requests());
        }
        LinkedBackendCommand::Workflows => {
            println!("{:#?}", octohub_backend::workflow_runs());
        }
        LinkedBackendCommand::Organization => {
            println!("{:#?}", octohub_backend::organization());
        }
        LinkedBackendCommand::User => {
            println!("{:#?}", octohub_backend::user());
        }
    }
    Ok(())
}
