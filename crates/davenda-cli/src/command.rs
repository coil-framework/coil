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
            ["auth", "check"],
            CommandOwner::Core,
            "Check whether a subject can currently exercise a capability against a live resource",
        )?,
        CommandDescriptor::new(
            ["auth", "bindings", "inspect"],
            CommandOwner::Core,
            "Inspect the active auth package capability bindings and their resolved relations",
        )?,
        CommandDescriptor::new(
            ["auth", "test-model"],
            CommandOwner::Core,
            "Run a batch of auth-model capability checks from a checked-in spec file",
        )?,
        CommandDescriptor::new(
            ["auth", "list"],
            CommandOwner::Core,
            "List live objects reachable for a subject, relation, and resource namespace",
        )?,
        CommandDescriptor::new(
            ["auth", "lookup"],
            CommandOwner::Core,
            "Lookup live subject ids for a resource, relation, and subject namespace",
        )?,
        CommandDescriptor::new(
            ["auth", "explain"],
            CommandOwner::Core,
            "Explain why a subject can or cannot exercise a capability",
        )?,
        CommandDescriptor::new(
            ["auth", "package", "validate"],
            CommandOwner::Core,
            "Validate the configured auth package against the installed module capability contracts",
        )?,
        CommandDescriptor::new(
            ["module", "list"],
            CommandOwner::Core,
            "List installed modules for the active customer app",
        )?,
        CommandDescriptor::new(
            ["module", "inspect"],
            CommandOwner::Core,
            "Inspect the installed module manifest, dependency contracts, and operator-relevant contributions",
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
            ["release", "plan"],
            CommandOwner::Core,
            "Compose the current upgrade plan from migrations, auth validation, and compatibility findings",
        )?,
        CommandDescriptor::new(
            ["cache", "warm"],
            CommandOwner::Core,
            "Warm public application cache entries for explicit customer-app routes",
        )?
        .with_dry_run(),
        CommandDescriptor::new(
            ["jobs", "status"],
            CommandOwner::Core,
            "Inspect registered runtime jobs, queue topology, and current queue health",
        )?,
        CommandDescriptor::new(
            ["jobs", "dead-letters"],
            CommandOwner::Core,
            "Inspect dead-lettered jobs, failure reasons, and retry exhaustion outcomes",
        )?,
        CommandDescriptor::new(
            ["tls", "status"],
            CommandOwner::Core,
            "Inspect TLS mode, provider state, and managed certificate inventory",
        )?,
        CommandDescriptor::new(
            ["tls", "renew"],
            CommandOwner::Core,
            "Renew a managed TLS certificate by issuing and activating a replacement",
        )?
        .with_dry_run()
        .requiring_confirmation(),
        CommandDescriptor::new(
            ["storage", "inspect"],
            CommandOwner::Core,
            "Inspect storage topology, delivery posture, and object-store readiness for the active customer app",
        )?,
        CommandDescriptor::new(
            ["storage", "verify"],
            CommandOwner::Core,
            "Verify storage policy planning and backend availability for the active customer app",
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
        CommandDescriptor::new(
            ["import", "cutover"],
            CommandOwner::Core,
            "Evaluate or execute cutover preparation for an import package and its target customer app",
        )?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_commands_include_jobs_dead_letters() {
        let commands = baseline_commands("harbor-shop").unwrap();
        assert!(commands.iter().any(|descriptor| {
            descriptor.path == vec!["jobs".to_string(), "dead-letters".to_string()]
        }));
    }
}
