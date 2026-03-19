mod command;
mod error;
mod registry;
mod report;
#[cfg(test)]
mod tests;
mod validation;

pub use command::{
    CommandDescriptor, CommandExecutionPlan, CommandInvocation, CommandOwner, OutputMode,
    baseline_commands,
};
pub use error::CliModelError;
pub use registry::{CliRuntime, CommandRegistry};
pub use report::{CommandReport, DiagnosticRecord, DiagnosticSeverity, ReportRow, ReportStatus};
