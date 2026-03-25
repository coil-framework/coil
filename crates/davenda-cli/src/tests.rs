use crate::{
    CliModelError, CliRuntime, CommandDescriptor, CommandInvocation, CommandOwner, CommandRegistry,
    CommandReport, DiagnosticRecord, DiagnosticSeverity, OutputMode, ReportRow, ReportStatus,
};

#[test]
fn baseline_runtime_registers_core_command_families() {
    let runtime = CliRuntime::baseline("showcase-events").unwrap();
    let paths = runtime
        .registry
        .commands()
        .map(|command| command.path.join(" "))
        .collect::<Vec<_>>();

    assert!(paths.contains(&"dev server".to_string()));
    assert!(paths.contains(&"config validate".to_string()));
    assert!(paths.contains(&"auth explain".to_string()));
    assert!(paths.contains(&"module list".to_string()));
    assert!(paths.contains(&"migrate plan".to_string()));
    assert!(paths.contains(&"migrate apply".to_string()));
    assert!(paths.contains(&"release doctor".to_string()));
    assert!(paths.contains(&"cache warm".to_string()));
    assert!(paths.contains(&"storage verify".to_string()));
    assert!(paths.contains(&"assets publish".to_string()));
    assert!(paths.contains(&"import run".to_string()));
    assert!(paths.contains(&"import cutover".to_string()));
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
            CommandInvocation::new(["import", "run"])
                .unwrap()
                .dry_run()
                .with_output_mode(OutputMode::Json),
        )
        .unwrap();
    assert!(dry_run.dry_run);
    assert_eq!(dry_run.output_mode, OutputMode::Json);

    let blocked = runtime.plan(
        CommandInvocation::new(["config", "validate"])
            .unwrap()
            .dry_run(),
    );
    assert_eq!(
        blocked.unwrap_err(),
        CliModelError::DryRunUnsupported {
            path: "config validate".to_string(),
        }
    );

    let descriptor = runtime
        .registry
        .find(&["import".to_string(), "run".to_string()]);
    assert_eq!(
        descriptor.unwrap().description,
        "Run a staged content or data import into the current customer app"
    );

    let confirmation_required = runtime.plan(CommandInvocation::new(["migrate", "apply"]).unwrap());
    assert_eq!(
        confirmation_required.unwrap_err(),
        CliModelError::ConfirmationRequired {
            path: "migrate apply".to_string(),
        }
    );

    let confirmed = runtime
        .plan(
            CommandInvocation::new(["assets", "publish"])
                .unwrap()
                .confirm(),
        )
        .unwrap();
    assert_eq!(confirmed.descriptor.path, vec!["assets", "publish"]);
}

#[test]
fn command_reports_capture_rows_and_diagnostics() {
    let mut report = CommandReport::new(
        ["release", "doctor"],
        "Checked upgrade compatibility for the current customer app",
    )
    .unwrap()
    .with_status(ReportStatus::Warning)
    .with_columns(["severity", "code", "message"])
    .unwrap();
    report.push_row(
        ReportRow::new()
            .with_cell("severity", "warning")
            .unwrap()
            .with_cell("code", "module.version.unpinned")
            .unwrap()
            .with_cell("message", "cms is not version pinned")
            .unwrap(),
    );
    report.push_diagnostic(
        DiagnosticRecord::new(
            DiagnosticSeverity::Warning,
            "module.version.unpinned",
            "cms is not version pinned",
        )
        .unwrap(),
    );

    assert_eq!(
        report.command,
        vec!["release".to_string(), "doctor".to_string()]
    );
    assert_eq!(report.status, ReportStatus::Warning);
    assert_eq!(report.rows.len(), 1);
    assert_eq!(report.diagnostics.len(), 1);
}
