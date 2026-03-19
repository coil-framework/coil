use crate::validation::{require_non_empty, validate_token};
use crate::CliModelError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOwner {
    Core,
    Module(String),
    CustomerApp(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDescriptor {
    pub path: Vec<String>,
    pub owner: CommandOwner,
    pub description: String,
    pub supports_json: bool,
    pub supports_dry_run: bool,
    pub requires_confirmation: bool,
}

impl CommandDescriptor {
    pub fn new(
        path: impl IntoIterator<Item = impl Into<String>>,
        owner: CommandOwner,
        description: impl Into<String>,
    ) -> Result<Self, CliModelError> {
        let path = path
            .into_iter()
            .map(|segment| validate_token("command_segment", segment.into()))
            .collect::<Result<Vec<_>, _>>()?;

        if path.is_empty() {
            return Err(CliModelError::EmptyField {
                field: "command_path",
            });
        }

        Ok(Self {
            path,
            owner,
            description: require_non_empty("command_description", description.into())?,
            supports_json: true,
            supports_dry_run: false,
            requires_confirmation: false,
        })
    }

    pub fn with_dry_run(mut self) -> Self {
        self.supports_dry_run = true;
        self
    }

    pub fn requiring_confirmation(mut self) -> Self {
        self.requires_confirmation = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    pub path: Vec<String>,
    pub output_mode: OutputMode,
    pub dry_run: bool,
    pub confirmed: bool,
}

impl CommandInvocation {
    pub fn new(path: impl IntoIterator<Item = impl Into<String>>) -> Result<Self, CliModelError> {
        let path = path
            .into_iter()
            .map(|segment| validate_token("command_segment", segment.into()))
            .collect::<Result<Vec<_>, _>>()?;

        if path.is_empty() {
            return Err(CliModelError::EmptyField {
                field: "command_path",
            });
        }

        Ok(Self {
            path,
            output_mode: OutputMode::Human,
            dry_run: false,
            confirmed: false,
        })
    }

    pub fn with_output_mode(mut self, output_mode: OutputMode) -> Self {
        self.output_mode = output_mode;
        self
    }

    pub fn dry_run(mut self) -> Self {
        self.dry_run = true;
        self
    }

    pub fn confirm(mut self) -> Self {
        self.confirmed = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandExecutionPlan {
    pub descriptor: CommandDescriptor,
    pub output_mode: OutputMode,
    pub dry_run: bool,
}

pub fn baseline_commands(customer_app: &str) -> Result<Vec<CommandDescriptor>, CliModelError> {
    Ok(vec![
        CommandDescriptor::new(
            ["dev", "server"],
            CommandOwner::Core,
            "Run the local development server",
        )?,
        CommandDescriptor::new(
            ["dev", "stack"],
            CommandOwner::Core,
            "Boot the full local platform stack with workers, scheduler, storage, and TLS",
        )?,
        CommandDescriptor::new(
            ["config", "validate"],
            CommandOwner::Core,
            "Validate effective platform configuration",
        )?,
        CommandDescriptor::new(
            ["config", "render"],
            CommandOwner::Core,
            "Render effective platform configuration after overlays",
        )?,
        CommandDescriptor::new(
            ["config", "diff"],
            CommandOwner::Core,
            "Diff effective platform configuration between environments or revisions",
        )?,
        CommandDescriptor::new(
            ["migrate", "plan"],
            CommandOwner::Core,
            "Plan core, module, auth, and customer-app migrations",
        )?
        .with_dry_run(),
        CommandDescriptor::new(
            ["migrate", "apply"],
            CommandOwner::Core,
            "Apply pending core, module, auth, and customer-app migrations",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["auth", "explain"],
            CommandOwner::Core,
            "Explain why a subject can or cannot exercise a capability",
        )?,
        CommandDescriptor::new(
            ["auth", "package", "validate"],
            CommandOwner::Core,
            "Validate an auth package before deployment",
        )?,
        CommandDescriptor::new(
            ["module", "list"],
            CommandOwner::Core,
            "List installed official modules for the current customer app",
        )?,
        CommandDescriptor::new(
            ["module", "install"],
            CommandOwner::Core,
            "Install or enable an official module for the current customer app",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["cache", "warm"],
            CommandOwner::Core,
            "Warm cache for a scoped route or customer app",
        )?
        .with_dry_run(),
        CommandDescriptor::new(
            ["cache", "invalidate"],
            CommandOwner::Core,
            "Invalidate cache by scope, route, or tag",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["storage", "verify"],
            CommandOwner::Core,
            "Verify storage policy and object-store state",
        )?
        .with_dry_run(),
        CommandDescriptor::new(
            ["storage", "sync"],
            CommandOwner::Core,
            "Reconcile managed assets and uploads with object storage",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["assets", "publish"],
            CommandOwner::CustomerApp(customer_app.to_string()),
            "Publish static asset manifests for the current customer app",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["assets", "verify"],
            CommandOwner::CustomerApp(customer_app.to_string()),
            "Verify published asset manifests and CDN-targeted outputs",
        )?,
        CommandDescriptor::new(
            ["tls", "renew"],
            CommandOwner::Core,
            "Renew certificates and validate issuance state",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["tls", "status"],
            CommandOwner::Core,
            "Inspect certificate health, challenge status, and active provider state",
        )?,
        CommandDescriptor::new(
            ["jobs", "worker"],
            CommandOwner::Core,
            "Run background workers and inspect queue health",
        )?,
        CommandDescriptor::new(
            ["jobs", "retry"],
            CommandOwner::Core,
            "Retry failed or dead-lettered jobs after inspection",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["import", "run"],
            CommandOwner::Core,
            "Run a staged content or data import into the current customer app",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["release", "doctor"],
            CommandOwner::Core,
            "Check upgrade compatibility across core, modules, auth, and extensions",
        )?,
        CommandDescriptor::new(
            ["release", "plan"],
            CommandOwner::Core,
            "Produce an upgrade and rollout plan for the current customer app",
        )?
        .with_dry_run(),
    ])
}
