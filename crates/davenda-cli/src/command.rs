use crate::CliModelError;
use crate::validation::{require_non_empty, validate_token};

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

pub fn baseline_commands(_customer_app: &str) -> Result<Vec<CommandDescriptor>, CliModelError> {
    Ok(vec![
        CommandDescriptor::new(
            ["dev", "server"],
            CommandOwner::Core,
            "Run the local development server",
        )?,
        CommandDescriptor::new(
            ["config", "validate"],
            CommandOwner::Core,
            "Validate effective platform configuration",
        )?,
        CommandDescriptor::new(
            ["auth", "explain"],
            CommandOwner::Core,
            "Explain why a subject can or cannot exercise a capability",
        )?,
        CommandDescriptor::new(
            ["module", "list"],
            CommandOwner::Core,
            "List installed modules for the active customer app",
        )?,
        CommandDescriptor::new(
            ["migrate", "plan"],
            CommandOwner::Core,
            "Plan core, module, auth, and customer-app migrations",
        )?,
        CommandDescriptor::new(
            ["migrate", "apply"],
            CommandOwner::Core,
            "Apply executable core and module migrations for the active customer app",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["release", "doctor"],
            CommandOwner::Core,
            "Check release compatibility for the active customer app",
        )?,
        CommandDescriptor::new(
            ["assets", "publish"],
            CommandOwner::Core,
            "Publish theme asset artifacts for the active customer app",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["import", "run"],
            CommandOwner::Core,
            "Run a staged content or data import into the current customer app",
        )?
        .with_dry_run(),
    ])
}
