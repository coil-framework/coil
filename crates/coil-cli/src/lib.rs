mod cli;
mod command;
mod error;
mod registry;
#[cfg(test)]
mod tests;
mod validation;

pub use cli::{
    AuthExplainInvocation, AuthExplainResult, CliApplication, CliRunError,
    ConfigValidateInvocation, ImportRunInvocation, run_from_args, run_from_env,
};
pub use command::{
    CommandDescriptor, CommandExecutionPlan, CommandInvocation, CommandOwner, OutputMode,
    baseline_commands,
};
pub use coil_report::{
    CommandReport, DiagnosticRecord, DiagnosticSeverity, ReportModelError, ReportRow, ReportStatus,
};
pub use error::CliModelError;
pub use registry::{CliRuntime, CommandRegistry};
