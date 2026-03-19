use std::collections::BTreeMap;

use crate::command::{
    baseline_commands, CommandDescriptor, CommandExecutionPlan, CommandInvocation, CommandOwner,
};
use crate::validation::{require_non_empty, validate_token};
use crate::CliModelError;

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
