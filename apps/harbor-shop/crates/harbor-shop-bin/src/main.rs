use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use harbor_shop_app::HarborShopWorkspace;

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
    Serve {
        #[arg(long)]
        bind: Option<String>,
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
        Command::Serve { bind } => serve(&workspace, &cli.config, bind),
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
    println!("server bind: {}", summary.config.server.bind);
    Ok(())
}

fn serve(workspace: &HarborShopWorkspace, config_path: &PathBuf, bind: Option<String>) -> Result<()> {
    davenda_all::builder()
        .with_customer_plugin(harbor_shop_backend::plugin())
        .run_from_paths(workspace.app_root(), workspace.resolve_path(config_path), bind)
        .context("Harbor Shop server exited with an error")
}
