use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CliModelError {
    #[error("`{field}` cannot be empty")]
    EmptyField { field: &'static str },
    #[error("`{field}` contains an invalid token `{value}`")]
    InvalidToken { field: &'static str, value: String },
    #[error("command `{path}` is already registered")]
    DuplicateCommand { path: String },
    #[error("command `{path}` was not found")]
    UnknownCommand { path: String },
    #[error("command `{path}` does not support --dry-run")]
    DryRunUnsupported { path: String },
    #[error("command `{path}` must be confirmed explicitly")]
    ConfirmationRequired { path: String },
}

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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandRegistry {
    commands: BTreeMap<Vec<String>, CommandDescriptor>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, command: CommandDescriptor) -> Result<(), CliModelError> {
        if self
            .commands
            .insert(command.path.clone(), command.clone())
            .is_some()
        {
            return Err(CliModelError::DuplicateCommand {
                path: command.path.join(" "),
            });
        }

        Ok(())
    }

    pub fn find(&self, path: &[String]) -> Option<&CommandDescriptor> {
        self.commands.get(path)
    }

    pub fn commands(&self) -> impl Iterator<Item = &CommandDescriptor> {
        self.commands.values()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliRuntime {
    pub customer_app: String,
    pub registry: CommandRegistry,
}

impl CliRuntime {
    pub fn baseline(customer_app: impl Into<String>) -> Result<Self, CliModelError> {
        let customer_app = require_non_empty("customer_app", customer_app.into())?;
        let mut registry = CommandRegistry::new();

        for command in baseline_commands(&customer_app)? {
            registry.register(command)?;
        }

        Ok(Self {
            customer_app,
            registry,
        })
    }

    pub fn register_module_command(
        &mut self,
        path: impl IntoIterator<Item = impl Into<String>>,
        module: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<(), CliModelError> {
        self.registry.register(CommandDescriptor::new(
            path,
            CommandOwner::Module(validate_token("module_name", module.into())?),
            description,
        )?)
    }

    pub fn plan(
        &self,
        invocation: CommandInvocation,
    ) -> Result<CommandExecutionPlan, CliModelError> {
        let descriptor = self
            .registry
            .find(&invocation.path)
            .cloned()
            .ok_or_else(|| CliModelError::UnknownCommand {
                path: invocation.path.join(" "),
            })?;

        if invocation.dry_run && !descriptor.supports_dry_run {
            return Err(CliModelError::DryRunUnsupported {
                path: descriptor.path.join(" "),
            });
        }

        if descriptor.requires_confirmation && !invocation.confirmed && !invocation.dry_run {
            return Err(CliModelError::ConfirmationRequired {
                path: descriptor.path.join(" "),
            });
        }

        Ok(CommandExecutionPlan {
            descriptor,
            output_mode: invocation.output_mode,
            dry_run: invocation.dry_run,
        })
    }
}

fn baseline_commands(customer_app: &str) -> Result<Vec<CommandDescriptor>, CliModelError> {
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
            ["module", "list"],
            CommandOwner::Core,
            "List installed official modules for the current customer app",
        )?,
        CommandDescriptor::new(
            ["cache", "warm"],
            CommandOwner::Core,
            "Warm cache for a scoped route or customer app",
        )?
        .with_dry_run(),
        CommandDescriptor::new(
            ["storage", "verify"],
            CommandOwner::Core,
            "Verify storage policy and object-store state",
        )?
        .with_dry_run(),
        CommandDescriptor::new(
            ["assets", "publish"],
            CommandOwner::CustomerApp(customer_app.to_string()),
            "Publish static asset manifests for the current customer app",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["tls", "renew"],
            CommandOwner::Core,
            "Renew certificates and validate issuance state",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["jobs", "worker"],
            CommandOwner::Core,
            "Run background workers and inspect queue health",
        )?,
        CommandDescriptor::new(
            ["release", "doctor"],
            CommandOwner::Core,
            "Check upgrade compatibility across core, modules, auth, and extensions",
        )?,
    ])
}

fn validate_token(field: &'static str, value: String) -> Result<String, CliModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CliModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(CliModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, CliModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(CliModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_runtime_registers_core_command_families() {
        let runtime = CliRuntime::baseline("showcase-events").unwrap();
        let paths = runtime
            .registry
            .commands()
            .map(|command| command.path.join(" "))
            .collect::<Vec<_>>();

        assert!(paths.contains(&"config validate".to_string()));
        assert!(paths.contains(&"migrate plan".to_string()));
        assert!(paths.contains(&"assets publish".to_string()));
        assert!(paths.contains(&"tls renew".to_string()));
    }

    #[test]
    fn registry_rejects_duplicate_command_paths() {
        let mut registry = CommandRegistry::new();
        let command =
            CommandDescriptor::new(["cache", "warm"], CommandOwner::Core, "Warm cache").unwrap();
        registry.register(command.clone()).unwrap();

        let error = registry.register(command).unwrap_err();
        assert_eq!(
            error,
            CliModelError::DuplicateCommand {
                path: "cache warm".to_string(),
            }
        );
    }

    #[test]
    fn module_commands_register_under_the_shared_registry() {
        let mut runtime = CliRuntime::baseline("showcase-events").unwrap();
        runtime
            .register_module_command(
                ["events", "reindex"],
                "events",
                "Rebuild event search and reporting projections",
            )
            .unwrap();

        let descriptor = runtime
            .registry
            .find(&vec!["events".to_string(), "reindex".to_string()])
            .unwrap();
        assert_eq!(descriptor.owner, CommandOwner::Module("events".to_string()));
    }

    #[test]
    fn invocation_plans_enforce_dry_run_and_confirmation_rules() {
        let runtime = CliRuntime::baseline("showcase-events").unwrap();
        let dry_run = runtime
            .plan(
                CommandInvocation::new(["migrate", "plan"])
                    .unwrap()
                    .dry_run()
                    .with_output_mode(OutputMode::Json),
            )
            .unwrap();
        assert!(dry_run.dry_run);
        assert_eq!(dry_run.output_mode, OutputMode::Json);

        let blocked = runtime.plan(CommandInvocation::new(["tls", "renew"]).unwrap());
        assert_eq!(
            blocked.unwrap_err(),
            CliModelError::ConfirmationRequired {
                path: "tls renew".to_string(),
            }
        );

        let confirmed = runtime
            .plan(CommandInvocation::new(["tls", "renew"]).unwrap().confirm())
            .unwrap();
        assert_eq!(confirmed.descriptor.path.join(" "), "tls renew");
    }
}
