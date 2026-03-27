use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use harbor_shop_app::{
    HarborShopWorkspace, default_cookie_secret, default_csrf_secret, environment_secret_resolver,
};

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
    let bootstrap = workspace.build_bootstrap(&cli.config)?;

    match cli.command {
        Command::Describe => describe(&bootstrap),
        Command::Serve { bind } => {
            let cookie_secret = required_secret_bytes(
                "DAVENDA_COOKIE_SECRET",
                default_cookie_secret(bootstrap.runtime_plan.runtime.config.app.environment),
            )?;
            let csrf_secret = required_secret_bytes(
                "DAVENDA_CSRF_SECRET",
                default_csrf_secret(bootstrap.runtime_plan.runtime.config.app.environment),
            )?;
            let bind =
                bind.unwrap_or_else(|| bootstrap.runtime_plan.runtime.config.server.bind.clone());
            let server = bootstrap.server_host(
                &environment_secret_resolver(),
                &cookie_secret,
                &csrf_secret,
            )?;
            let listener = tokio::net::TcpListener::bind(&bind)
                .await
                .with_context(|| format!("failed to bind Harbor Shop server to {bind}"))?;
            println!("Harbor Shop linked workspace server listening on {bind}");
            server
                .serve(listener)
                .await
                .context("Harbor Shop server exited with an error")
        }
    }
}

fn describe(bootstrap: &harbor_shop_app::HarborShopBootstrap) -> Result<()> {
    println!("Harbor Shop customer workspace");
    println!("app root: {}", bootstrap.app_root.display());
    println!("config: {}", bootstrap.config_path.display());
    println!("app id: {}", bootstrap.manifest.id);
    println!("auth package: {}", bootstrap.manifest.auth.package_name);
    println!(
        "modules: {}",
        bootstrap
            .module_ids()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "linked plugins: {}",
        bootstrap.linked_plugin_ids().join(", ")
    );
    println!(
        "server bind: {}",
        bootstrap.runtime_plan.runtime.config.server.bind
    );
    Ok(())
}

fn required_secret_bytes(name: &str, fallback: Option<&str>) -> Result<Vec<u8>> {
    match env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value.into_bytes()),
        Ok(_) => bail!("environment variable `{name}` is present but empty"),
        Err(_) => fallback
            .map(|value| value.as_bytes().to_vec())
            .ok_or_else(|| anyhow::anyhow!("environment variable `{name}` is required")),
    }
}
