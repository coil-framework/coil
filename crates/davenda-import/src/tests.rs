use super::*;
use davenda_report::ReportStatus;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("davenda-import-{label}-{unique}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_json(path: impl AsRef<Path>, value: serde_json::Value) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
}

fn write_text(path: impl AsRef<Path>, text: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap();
}

fn journal_path(root: &Path, run_id: &str) -> PathBuf {
    root.join(".davenda")
        .join("import-runs")
        .join(format!("{run_id}.json"))
}

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
    .with_module("media")
    .unwrap()
    .with_importer(
        ImporterSpec::new(
            ImporterId::new("media").unwrap(),
            10,
            "asset",
            "Import media and attachments",
        )
        .unwrap()
        .with_source_path("fixtures/media.json")
        .unwrap(),
    )
    .with_importer(
        ImporterSpec::new(
            ImporterId::new("pages").unwrap(),
            20,
            "page",
            "Import pages after media",
        )
        .unwrap()
        .with_source_path("fixtures/pages.json")
        .unwrap()
        .depending_on(ImporterId::new("media").unwrap()),
    );

    let plan = manifest.plan().unwrap();
    assert_eq!(
        plan.ordered_importers[0].id,
        ImporterId::new("media").unwrap()
    );
    assert_eq!(
        plan.ordered_importers[1].id,
        ImporterId::new("pages").unwrap()
    );

    let report = plan.command_report().unwrap();
    assert_eq!(
        report.command,
        vec!["import".to_string(), "run".to_string()]
    );
    assert_eq!(report.rows.len(), 2);
    assert_eq!(report.rows[0].cells["source"], "fixtures/media.json");
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
        .with_source_path("pages.json")
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
        .with_source_path("pages.json")
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
        .with_source_path("media.json")
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
fn import_execution_stages_media_then_pages_with_resolved_asset_references() {
    let root = unique_dir("execute-stage");
    write_json(
        root.join("media.json"),
        json!([
            {
                "source_key": "wp:media:hero",
                "checksum": "hero-v1",
                "title": "Harbor Hero",
                "slug": "harbor-hero",
                "content_type": "image/jpeg",
                "source_url": "https://legacy.example.com/uploads/hero.jpg",
                "alt_text": "Harbor hero"
            }
        ]),
    );
    write_json(
        root.join("pages.json"),
        json!([
            {
                "source_key": "wp:post:home",
                "checksum": "page-v1",
                "title": "Home",
                "slug": "home",
                "body_html": "<p>Welcome aboard</p>",
                "canonical_path": "/en-GB/home",
                "media_references": ["wp:media:hero"]
            }
        ]),
    );

    let manifest = ImportManifest::new(
        ImportRunId::new("wordpress-cutover").unwrap(),
        SourceSystemId::new("wordpress").unwrap(),
        "2026-03-19T00:00:00Z",
        "harbor-shop",
    )
    .unwrap()
    .with_locale("en-GB")
    .unwrap()
    .with_site("main")
    .unwrap()
    .with_importer(
        ImporterSpec::new(
            ImporterId::new("media").unwrap(),
            10,
            "asset",
            "Import media",
        )
        .unwrap()
        .with_source_path("media.json")
        .unwrap(),
    )
    .with_importer(
        ImporterSpec::new(
            ImporterId::new("pages").unwrap(),
            20,
            "page",
            "Import pages",
        )
        .unwrap()
        .with_source_path("pages.json")
        .unwrap()
        .with_mapping("template", "pages/home")
        .unwrap()
        .with_mapping("page_type", "home")
        .unwrap()
        .depending_on(ImporterId::new("media").unwrap()),
    );
    let plan = manifest.plan().unwrap();

    let execution = plan
        .execute(&root, journal_path(&root, "wordpress-cutover"))
        .unwrap();

    assert_eq!(execution.importer_records.len(), 2);
    assert_eq!(execution.importer_records[0].staged_records, 1);
    assert_eq!(execution.importer_records[1].staged_records, 1);
    assert_eq!(
        execution.summary.status_counts()[&ImportRecordStatus::StagedForReview],
        2
    );

    let pages_path = PathBuf::from(
        execution.importer_records[1]
            .staged_path
            .clone()
            .unwrap(),
    );
    let pages: serde_json::Value = serde_json::from_str(&fs::read_to_string(pages_path).unwrap()).unwrap();
    assert_eq!(
        pages[0]["normalized"]["media_references"][0].as_str(),
        Some("asset:harbor-hero")
    );

    let report = execution.command_report().unwrap();
    assert_eq!(report.status, ReportStatus::Warning);
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "import.pages.staged"));
}

#[test]
fn import_execution_skips_unchanged_records_and_updates_changed_ones() {
    let root = unique_dir("execute-rerun");
    let source_path = root.join("pages.json");
    write_json(
        &source_path,
        json!([
            {
                "source_key": "wp:post:landing",
                "checksum": "page-v1",
                "title": "Landing",
                "slug": "landing",
                "body_html": "<p>v1</p>"
            }
        ]),
    );

    let mut manifest = ImportManifest::new(
        ImportRunId::new("wordpress-pages").unwrap(),
        SourceSystemId::new("wordpress").unwrap(),
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
        .with_source_path("pages.json")
        .unwrap(),
    );
    manifest.publication_mode = PublicationMode::PublishValidated;
    let plan = manifest.plan().unwrap();
    let journal = journal_path(&root, "wordpress-pages");

    let first = plan.execute(&root, &journal).unwrap();
    assert_eq!(first.importer_records[0].imported_records, 1);

    let second = plan.execute(&root, &journal).unwrap();
    assert_eq!(second.importer_records[0].skipped_records, 1);
    assert_eq!(
        second.importer_records[0].status,
        ImporterExecutionStatus::SkippedCompleted
    );

    write_json(
        &source_path,
        json!([
            {
                "source_key": "wp:post:landing",
                "checksum": "page-v2",
                "title": "Landing",
                "slug": "landing",
                "body_html": "<p>v2</p>"
            }
        ]),
    );
    let third = plan.execute(&root, &journal).unwrap();
    assert_eq!(third.importer_records[0].updated_records, 1);
    assert_eq!(
        third.summary.status_counts()[&ImportRecordStatus::Updated],
        1
    );
}

#[test]
fn strict_validation_stops_invalid_records_and_permissive_mode_stages_exceptions() {
    let root = unique_dir("execute-invalid");
    write_json(
        root.join("pages.json"),
        json!([
            {
                "source_key": "wp:post:broken",
                "checksum": "broken-v1",
                "slug": "broken",
                "body_html": "<p>Broken</p>",
                "canonical_path": "not-a-path"
            }
        ]),
    );

    let importer = ImporterSpec::new(
        ImporterId::new("pages").unwrap(),
        10,
        "page",
        "Import pages",
    )
    .unwrap()
    .with_source_path("pages.json")
    .unwrap();

    let strict_manifest = ImportManifest::new(
        ImportRunId::new("strict-pages").unwrap(),
        SourceSystemId::new("wordpress").unwrap(),
        "2026-03-19T00:00:00Z",
        "harbor-shop",
    )
    .unwrap()
    .with_importer(importer.clone());
    let strict_error = strict_manifest
        .plan()
        .unwrap()
        .execute(&root, journal_path(&root, "strict-pages"))
        .unwrap_err();
    assert!(matches!(
        strict_error,
        ImportModelError::InvalidSourceRecord { .. }
    ));

    let mut permissive_manifest = ImportManifest::new(
        ImportRunId::new("permissive-pages").unwrap(),
        SourceSystemId::new("wordpress").unwrap(),
        "2026-03-19T00:00:00Z",
        "harbor-shop",
    )
    .unwrap()
    .with_importer(importer);
    permissive_manifest.validation_mode = ValidationMode::Permissive;
    let execution = permissive_manifest
        .plan()
        .unwrap()
        .execute(&root, journal_path(&root, "permissive-pages"))
        .unwrap();

    assert_eq!(execution.importer_records[0].failed_records, 1);
    let report = execution.command_report().unwrap();
    assert_eq!(report.status, ReportStatus::Unsafe);
    assert!(execution.importer_records[0].exception_path.is_some());
}

#[test]
fn import_execution_requires_source_paths_and_well_formed_source_batches() {
    let root = unique_dir("execute-source-shape");

    let manifest = ImportManifest::new(
        ImportRunId::new("missing-source").unwrap(),
        SourceSystemId::new("wordpress").unwrap(),
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
        .unwrap(),
    );
    let error = manifest
        .plan()
        .unwrap()
        .execute(&root, journal_path(&root, "missing-source"))
        .unwrap_err();
    assert!(matches!(
        error,
        ImportModelError::MissingImporterSourcePath { .. }
    ));

    write_text(root.join("invalid.json"), r#"{"not_records": []}"#);
    let malformed = ImportManifest::new(
        ImportRunId::new("bad-source").unwrap(),
        SourceSystemId::new("wordpress").unwrap(),
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
        .with_source_path("invalid.json")
        .unwrap(),
    );
    let error = malformed
        .plan()
        .unwrap()
        .execute(&root, journal_path(&root, "bad-source"))
        .unwrap_err();
    assert!(matches!(error, ImportModelError::SourceShape { .. }));
}

#[test]
fn strict_validation_rejects_pages_with_unresolved_media_references() {
    let root = unique_dir("execute-missing-media");
    write_json(
        root.join("pages.json"),
        json!([
            {
                "source_key": "wp:post:home",
                "checksum": "page-home-v1",
                "title": "Home",
                "slug": "home",
                "body_html": "<p>Home</p>",
                "media_references": ["wp:media:missing"]
            }
        ]),
    );

    let manifest = ImportManifest::new(
        ImportRunId::new("missing-media").unwrap(),
        SourceSystemId::new("wordpress").unwrap(),
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
        .with_source_path("pages.json")
        .unwrap(),
    );

    let error = manifest
        .plan()
        .unwrap()
        .execute(&root, journal_path(&root, "missing-media"))
        .unwrap_err();
    assert!(matches!(
        error,
        ImportModelError::InvalidSourceRecord { .. }
    ));
}

#[test]
fn checked_in_wordpress_fixture_manifest_executes_end_to_end() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let manifest_path = repo_root.join("imports").join("wordpress-events.toml");
    let manifest = ImportManifest::from_file(&manifest_path).unwrap();
    let plan = manifest.plan().unwrap();
    let run_root = unique_dir("checked-in-wordpress");
    let journal = journal_path(&run_root, "wordpress-events");
    let execution = plan
        .execute(manifest_path.parent().unwrap(), &journal)
        .unwrap();

    assert_eq!(execution.importer_records.len(), 4);
    assert!(execution
        .importer_records
        .iter()
        .all(|record| record.staged_records == 1));
    assert_eq!(
        execution.summary.status_counts()[&ImportRecordStatus::StagedForReview],
        4
    );
}

#[test]
fn manifest_document_loads_toml_into_a_typed_manifest_with_sources_and_mapping() {
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
id = "pages"
phase = 10
resource_kind = "page"
description = "Import pages"
source_path = "fixtures/pages.json"
mapping = { template = "pages/home", page_type = "home" }

[[importers]]
id = "events"
phase = 20
resource_kind = "event"
description = "Import events and timeslots"
source_path = "fixtures/events.json"
dependencies = ["pages"]
"#,
    )
    .unwrap();

    assert_eq!(
        manifest.run_id,
        ImportRunId::new("wordpress-events").unwrap()
    );
    assert_eq!(
        manifest.modules,
        vec!["cms".to_string(), "events".to_string()]
    );
    assert_eq!(manifest.locale.as_deref(), Some("en"));
    assert_eq!(manifest.site.as_deref(), Some("main"));
    assert_eq!(manifest.importers.len(), 2);
    assert_eq!(
        manifest.importers[0].source_path.as_deref(),
        Some("fixtures/pages.json")
    );
    assert_eq!(
        manifest.importers[0].mapping["template"],
        "pages/home".to_string()
    );
    assert_eq!(
        manifest.importers[1].dependencies,
        vec![ImporterId::new("pages").unwrap()]
    );
}

#[test]
fn manifest_document_reads_from_disk() {
    let root = unique_dir("manifest-disk");
    let path = root.join("manifest.toml");
    write_text(
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
source_path = "pages.json"
"#,
    );

    let manifest = ImportManifest::from_file(&path).unwrap();
    assert_eq!(manifest.importers.len(), 1);
    assert_eq!(manifest.importers[0].id, ImporterId::new("pages").unwrap());
    assert_eq!(manifest.importers[0].source_path.as_deref(), Some("pages.json"));
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
