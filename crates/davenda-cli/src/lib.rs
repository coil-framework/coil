mod command;
mod error;
mod registry;
mod report;
#[cfg(test)]
mod tests;
mod validation;

pub use command::{
    baseline_commands, CommandDescriptor, CommandExecutionPlan, CommandInvocation, CommandOwner,
    OutputMode,
};
pub use error::CliModelError;
pub use registry::{CliRuntime, CommandRegistry};
pub use report::{CommandReport, DiagnosticRecord, DiagnosticSeverity, ReportRow, ReportStatus};
