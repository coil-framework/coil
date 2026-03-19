use super::*;
use davenda_report::ReportStatus;

#[test]
fn manifest_plans_importers_in_dependency_order() {
    let manifest = ImportManifest::new(
        ImportRunId::new("wordpress-cutover").unwrap(),
        SourceSystemId::new("wordpress").unwrap(),
        "2026-03-19T00:00:00Z",
        "harbor-shop",
    )
    .unwrap()
    .with_module("cms")
    .unwrap()
    .with_module("events")
    .unwrap()
    .with_importer(
        ImporterSpec::new(
            ImporterId::new("users").unwrap(),
            10,
            "user",
            "Import users and groups",
        )
        .unwrap(),
    )
    .with_importer(
        ImporterSpec::new(
            ImporterId::new("events").unwrap(),
            20,
            "event",
            "Import events and timeslots",
        )
        .unwrap()
        .depending_on(ImporterId::new("users").unwrap()),
    )
    .with_importer(
        ImporterSpec::new(
            ImporterId::new("bookings").unwrap(),
            30,
            "booking",
            "Import bookings after events and identities",
        )
        .unwrap()
        .depending_on(ImporterId::new("users").unwrap())
        .depending_on(ImporterId::new("events").unwrap()),
    );

    let plan = manifest.plan().unwrap();
    assert_eq!(
        plan.ordered_importers[0].id,
        ImporterId::new("users").unwrap()
    );
    assert_eq!(
        plan.ordered_importers[1].id,
        ImporterId::new("events").unwrap()
    );
    assert_eq!(
        plan.ordered_importers[2].id,
        ImporterId::new("bookings").unwrap()
    );

    let report = plan.command_report().unwrap();
    assert_eq!(
        report.command,
        vec!["import".to_string(), "run".to_string()]
    );
    assert_eq!(report.rows.len(), 3);
}

#[test]
fn manifest_rejects_cycles_and_unknown_dependencies() {
    let unknown = ImportManifest::new(
        ImportRunId::new("bad-import").unwrap(),
        SourceSystemId::new("legacy").unwrap(),
        "2026-03-19T00:00:00Z",
        "harbor-shop",
    )
    .unwrap()
    .with_importer(
        ImporterSpec::new(
            ImporterId::new("pages").unwrap(),
            10,
            "page",
            "Import pages",
        )
        .unwrap()
        .depending_on(ImporterId::new("media").unwrap()),
    );

    assert_eq!(
        unknown.plan().unwrap_err(),
        ImportModelError::UnknownImporterDependency {
            importer_id: "pages".to_string(),
            dependency: "media".to_string(),
        }
    );

    let cyclic = ImportManifest::new(
        ImportRunId::new("cyclic-import").unwrap(),
        SourceSystemId::new("legacy").unwrap(),
        "2026-03-19T00:00:00Z",
        "harbor-shop",
    )
    .unwrap()
    .with_importer(
        ImporterSpec::new(
            ImporterId::new("pages").unwrap(),
            10,
            "page",
            "Import pages",
        )
        .unwrap()
        .depending_on(ImporterId::new("media").unwrap()),
    )
    .with_importer(
        ImporterSpec::new(
            ImporterId::new("media").unwrap(),
            10,
            "asset",
            "Import media",
        )
        .unwrap()
        .depending_on(ImporterId::new("pages").unwrap()),
    );

    assert_eq!(
        cyclic.plan().unwrap_err(),
        ImportModelError::CyclicImporterDependencies
    );
}

#[test]
fn import_run_summary_tracks_idempotent_receipts() {
    let mut summary = ImportRunSummary::new();
    summary
        .record(
            ImportRecordReceipt::new(
                SourceRecordKey::new("wp:post:42").unwrap(),
                "batch-1",
                ImportRecordStatus::Imported,
            )
            .unwrap()
            .targeting(TargetRecordId::new("page:home").unwrap()),
        )
        .unwrap();
    summary
        .record(
            ImportRecordReceipt::new(
                SourceRecordKey::new("wp:post:43").unwrap(),
                "batch-1",
                ImportRecordStatus::SkippedUnchanged,
            )
            .unwrap(),
        )
        .unwrap();

    let counts = summary.status_counts();
    assert_eq!(counts[&ImportRecordStatus::Imported], 1);
    assert_eq!(counts[&ImportRecordStatus::SkippedUnchanged], 1);
}

#[test]
fn cutover_plan_surfaces_readiness_and_rollback_triggers() {
    let ready = CutoverPlan::new()
        .with_check(
            CutoverCheck::new("tls", "TLS issuance and validation are green", true, true).unwrap(),
        )
        .with_check(
            CutoverCheck::new("final-import", "Final delta import completed", true, true).unwrap(),
        );
    assert!(ready.is_ready());
    assert_eq!(ready.command_report().unwrap().status, ReportStatus::Ok);

    let blocked = ready.with_trigger(
        RollbackTrigger::new(
            RollbackTriggerId::new("auth-failure").unwrap(),
            "Systemic admin auth failures after cutover",
        )
        .unwrap()
        .fired(),
    );
    assert!(!blocked.is_ready());
    let report = blocked.command_report().unwrap();
    assert_eq!(report.status, ReportStatus::Unsafe);
    assert_eq!(report.diagnostics.len(), 1);
}
