use std::path::PathBuf;

use anyhow::{Context, Result};
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
    Serve {
        #[arg(long)]
        bind: Option<String>,
    },
    LinkedBackend {
        #[command(subcommand)]
        command: LinkedBackendCommand,
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
        Command::Serve { bind } => serve(&workspace, &cli.config, bind),
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

fn serve(
    workspace: &HarborShopWorkspace,
    config_path: &PathBuf,
    bind: Option<String>,
) -> Result<()> {
    davenda_all::builder()
        .with_customer_plugin(harbor_shop_backend::plugin())
        .run_from_paths(
            workspace.app_root(),
            workspace.resolve_path(config_path),
            bind,
        )
        .context("Harbor Shop server exited with an error")
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
        summary.documentation_url.unwrap_or_else(|| "none".to_string()),
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
        assert!(output.contains("registered hooks: checkout, verified_webhook"), "{output}");
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

        assert!(output.contains("plugin id: harbor-shop-backend"), "{output}");
        assert!(output.contains("display name: Harbor Shop Linked Backend"), "{output}");
        assert!(output.contains("registered hooks: checkout, verified_webhook"), "{output}");
        assert!(
            output.contains("documentation: apps/harbor-shop/backend/README.md"),
            "{output}"
        );
    }
}
