mod cli;
mod command;
mod error;
mod registry;
mod report;
#[cfg(test)]
mod tests;
mod validation;

pub use cli::{
    AuthExplainInvocation, AuthExplainResult, CliApplication, CliRunError, run_from_args,
    run_from_env,
};
pub use command::{
    CommandDescriptor, CommandExecutionPlan, CommandInvocation, CommandOwner, OutputMode,
    baseline_commands,
};
pub use error::CliModelError;
pub use registry::{CliRuntime, CommandRegistry};
pub use report::{CommandReport, DiagnosticRecord, DiagnosticSeverity, ReportRow, ReportStatus};
