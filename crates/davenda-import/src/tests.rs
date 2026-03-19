use super::*;
use davenda_report::ReportStatus;
use std::fs;
use std::path::PathBuf;

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

#[test]
fn manifest_document_loads_toml_into_a_typed_manifest() {
    let manifest = ImportManifest::from_toml_str(
        r#"
run_id = "wordpress-events"
source_system = "wordpress"
snapshot_at = "2026-03-19T00:00:00Z"
customer_app_id = "harbor-shop"
modules = ["cms", "events"]
locale = "en"
site = "main"
validation_mode = "strict"
publication_mode = "stage_validated"
asset_storage_default = "public_upload"

[[importers]]
id = "users"
phase = 10
resource_kind = "user"
description = "Import users and groups"

[[importers]]
id = "events"
phase = 20
resource_kind = "event"
description = "Import events and timeslots"
dependencies = ["users"]
"#,
    )
    .unwrap();

    assert_eq!(manifest.run_id, ImportRunId::new("wordpress-events").unwrap());
    assert_eq!(manifest.modules, vec!["cms".to_string(), "events".to_string()]);
    assert_eq!(manifest.locale.as_deref(), Some("en"));
    assert_eq!(manifest.site.as_deref(), Some("main"));
    assert_eq!(manifest.importers.len(), 2);
    assert_eq!(
        manifest.importers[1].dependencies,
        vec![ImporterId::new("users").unwrap()]
    );
}

#[test]
fn manifest_document_reads_from_disk() {
    let path = PathBuf::from("/tmp/davenda-import-manifest.toml");
    fs::write(
        &path,
        r#"
run_id = "wordpress-pages"
source_system = "wordpress"
snapshot_at = "2026-03-19T00:00:00Z"
customer_app_id = "harbor-shop"

[[importers]]
id = "pages"
phase = 10
resource_kind = "page"
description = "Import pages"
"#,
    )
    .unwrap();

    let manifest = ImportManifest::from_file(&path).unwrap();
    assert_eq!(manifest.importers.len(), 1);
    assert_eq!(manifest.importers[0].id, ImporterId::new("pages").unwrap());
}
