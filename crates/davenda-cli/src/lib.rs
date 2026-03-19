mod cli;
mod command;
mod error;
mod registry;
#[cfg(test)]
mod tests;
mod validation;

pub use cli::{
    run_from_args, run_from_env, AuthExplainInvocation, AuthExplainResult, CliApplication,
    CliRunError, ImportRunInvocation,
};
pub use command::{
    baseline_commands, CommandDescriptor, CommandExecutionPlan, CommandInvocation, CommandOwner,
    OutputMode,
};
pub use davenda_report::{
    CommandReport, DiagnosticRecord, DiagnosticSeverity, ReportModelError, ReportRow, ReportStatus,
};
pub use error::CliModelError;
pub use registry::{CliRuntime, CommandRegistry};
