use super::*;
use davenda_auth::{DefaultSubject, DefaultTuple, DefaultTupleUpdate, Entity, Relation};
use davenda_config::{ObjectStoreKind, SingleNodeStorageMode, StorageClass, StorageDeployment};
use davenda_storage::{
    DeliveryMode, DurableStore, ObjectStoreTarget, Sensitivity, StorageBackendKind,
    StorageExecutor, StoragePlanner, StoragePolicyOverride, StoragePolicySet, StorageTopology,
    SyncMode,
};
use std::fs;

fn object_store_planner() -> StoragePlanner {
    StoragePlanner::new(
        StorageTopology {
            local_root: "/srv/davenda".to_string(),
            default_class: StorageClass::PublicUpload,
            deployment: StorageDeployment::Distributed,
            single_node_escape_hatch: SingleNodeStorageMode::Disabled,
            object_store: Some(ObjectStoreTarget {
                kind: ObjectStoreKind::S3,
            }),
        },
        StoragePolicySet::default(),
    )
}

fn local_only_planner() -> StoragePlanner {
    StoragePlanner::new(
        StorageTopology {
            local_root: "/srv/davenda".to_string(),
            default_class: StorageClass::LocalOnlySensitive,
            deployment: StorageDeployment::SingleNode,
            single_node_escape_hatch: SingleNodeStorageMode::ExplicitSingleNode,
            object_store: None,
        },
        StoragePolicySet::default(),
    )
}

fn fingerprint(seed: &str) -> ContentFingerprint {
    ContentFingerprint::new(FingerprintAlgorithm::Sha256, seed).unwrap()
}

#[test]
fn deployment_release_publishes_hashed_artifacts_to_a_manifest() {
    let planner = object_store_planner();
    let release = DeploymentRelease::new(
        ReleaseId::new("release-20260318").unwrap(),
        [DeploymentArtifact::new(
            "theme/app.css",
            "deploy/theme/app.abc123.css",
            fingerprint("abc123"),
            "text/css",
            4096,
        )
        .unwrap()],
    )
    .unwrap();

    let manifest = release
        .publish(&planner, "https://cdn.example.com/assets")
        .unwrap();
    let entry = manifest.resolve("theme/app.css").unwrap();

    assert_eq!(manifest.release_id().as_str(), "release-20260318");
    assert_eq!(entry.delivery().asset_kind(), AssetKind::DeploymentArtifact);
    assert_eq!(entry.delivery().audience(), DeliveryAudience::Public);
    assert_eq!(entry.delivery().delivery_mode(), DeliveryMode::PublicCdn);
    assert_eq!(entry.delivery().durable_store(), DurableStore::ObjectStore);
    assert!(entry.delivery().immutable());
    assert_eq!(
        entry
            .delivery()
            .storage_plan()
            .primary_write_target()
            .unwrap()
            .backend,
        StorageBackendKind::S3Compatible
    );
    assert_eq!(
        entry.delivery().target(),
        &AssetDeliveryTarget::Cdn {
            public_url: "https://cdn.example.com/assets/deploy/theme/app.abc123.css".to_string(),
            object_key: "deploy/theme/app.abc123.css".to_string(),
        }
    );
}

#[test]
fn deployment_release_rejects_duplicate_logical_paths() {
    let planner = object_store_planner();
    let release = DeploymentRelease::new(
        ReleaseId::new("release-dup").unwrap(),
        [
            DeploymentArtifact::new(
                "theme/app.css",
                "deploy/theme/app.abc123.css",
                fingerprint("abc123"),
                "text/css",
                4096,
            )
            .unwrap(),
            DeploymentArtifact::new(
                "theme/app.css",
                "deploy/theme/app.def456.css",
                fingerprint("def456"),
                "text/css",
                8192,
            )
            .unwrap(),
        ],
    )
    .unwrap();

    assert_eq!(
        release
            .publish(&planner, "https://cdn.example.com")
            .unwrap_err(),
        AssetModelError::DuplicateDeploymentArtifact {
            release_id: "release-dup".to_string(),
            logical_path: "theme/app.css".to_string(),
        }
    );
}

#[test]
fn theme_asset_publication_plan_publishes_and_syncs_source_roots() {
    let workspace = tempfile::tempdir().unwrap();
    let storage_root = tempfile::tempdir().unwrap();
    let theme_root = workspace.path().join("theme/assets");
    fs::create_dir_all(&theme_root).unwrap();
    fs::write(theme_root.join("site.css"), b"body { color: #111; }").unwrap();
    fs::write(theme_root.join("logo.svg"), b"<svg viewBox=\"0 0 1 1\" />").unwrap();

    let plan = ThemeAssetPublicationPlan::from_roots(
        ReleaseId::new("release-theme-assets").unwrap(),
        workspace.path(),
        ["theme/assets"],
    )
    .unwrap();

    let planner = StoragePlanner::new(
        StorageTopology {
            local_root: storage_root.path().display().to_string(),
            default_class: StorageClass::PublicUpload,
            deployment: StorageDeployment::Distributed,
            single_node_escape_hatch: SingleNodeStorageMode::Disabled,
            object_store: Some(ObjectStoreTarget {
                kind: ObjectStoreKind::S3,
            }),
        },
        StoragePolicySet::default(),
    );
    let executor = StorageExecutor::from_topology(planner.topology());
    let receipt = plan
        .publish_and_sync(&planner, "https://cdn.example.com/assets", &executor)
        .unwrap();

    assert_eq!(
        receipt.manifest().release_id().as_str(),
        "release-theme-assets"
    );
    assert_eq!(receipt.manifest().entries().count(), 2);
    assert_eq!(receipt.writes().len(), 2);

    let css = receipt.manifest().resolve("theme/assets/site.css").unwrap();
    assert!(matches!(
        css.delivery().target(),
        AssetDeliveryTarget::Cdn { public_url, object_key }
            if public_url.starts_with("https://cdn.example.com/assets/deploy/theme/assets/site.")
                && public_url.ends_with(".css")
                && object_key.starts_with("deploy/theme/assets/site.")
                && object_key.ends_with(".css")
    ));
    let read_back = executor
        .execute_read(css.delivery().storage_plan())
        .unwrap();
    assert_eq!(read_back.bytes, b"body { color: #111; }");
}

#[test]
fn managed_assets_require_publication_before_public_delivery() {
    let planner = object_store_planner();
    let revision = ManagedAssetRevision::plan(
        RevisionId::new("rev-1").unwrap(),
        &planner,
        "media/brochure.pdf",
        Some(StoragePolicyOverride {
            delivery_mode: Some(DeliveryMode::PublicCdn),
            sync_mode: Some(SyncMode::ObjectStore),
            sensitivity: Some(Sensitivity::Public),
        }),
        "application/pdf",
        1024,
        fingerprint("rev1"),
    )
    .unwrap();

    let asset = ManagedAsset::new(
        AssetId::new("asset-brochure").unwrap(),
        "Event brochure",
        revision,
    )
    .unwrap();

    assert_eq!(
        asset
            .plan_public_delivery(
                &DeliveryContext::default().with_cdn_base_url("https://cdn.example.com")
            )
            .unwrap_err(),
        AssetModelError::NotPublished {
            asset_id: "asset-brochure".to_string(),
        }
    );
}

#[test]
fn replacing_a_managed_asset_keeps_the_live_revision_until_republished() {
    let planner = object_store_planner();
    let mut asset = ManagedAsset::new(
        AssetId::new("asset-hero").unwrap(),
        "Homepage hero",
        ManagedAssetRevision::plan(
            RevisionId::new("rev-1").unwrap(),
            &planner,
            "media/hero-v1.jpg",
            Some(StoragePolicyOverride {
                delivery_mode: Some(DeliveryMode::PublicCdn),
                sync_mode: Some(SyncMode::ObjectStore),
                sensitivity: Some(Sensitivity::Public),
            }),
            "image/jpeg",
            2048,
            fingerprint("hero1"),
        )
        .unwrap(),
    )
    .unwrap();

    asset.publish_current();
    let first_live = asset
        .plan_public_delivery(
            &DeliveryContext::default().with_cdn_base_url("https://cdn.example.com"),
        )
        .unwrap();
    assert_eq!(
        first_live.revision_id().map(RevisionId::as_str),
        Some("rev-1")
    );

    asset.replace_current_revision(
        ManagedAssetRevision::plan(
            RevisionId::new("rev-2").unwrap(),
            &planner,
            "media/hero-v2.jpg",
            Some(StoragePolicyOverride {
                delivery_mode: Some(DeliveryMode::PublicCdn),
                sync_mode: Some(SyncMode::ObjectStore),
                sensitivity: Some(Sensitivity::Public),
            }),
            "image/jpeg",
            3072,
            fingerprint("hero2"),
        )
        .unwrap(),
    );

    assert!(asset.has_pending_changes());
    let still_live = asset
        .plan_public_delivery(
            &DeliveryContext::default().with_cdn_base_url("https://cdn.example.com"),
        )
        .unwrap();
    assert_eq!(
        still_live.revision_id().map(RevisionId::as_str),
        Some("rev-1")
    );

    asset.publish_current();
    let republished = asset
        .plan_public_delivery(
            &DeliveryContext::default().with_cdn_base_url("https://cdn.example.com"),
        )
        .unwrap();
    assert_eq!(
        republished.revision_id().map(RevisionId::as_str),
        Some("rev-2")
    );
}

#[test]
fn managed_asset_auth_updates_track_public_publication_state() {
    let planner = object_store_planner();
    let revision = ManagedAssetRevision::plan(
        RevisionId::new("rev-public").unwrap(),
        &planner,
        "media/public/logo.png",
        Some(StoragePolicyOverride {
            delivery_mode: Some(DeliveryMode::PublicCdn),
            sync_mode: Some(SyncMode::ObjectStore),
            sensitivity: Some(Sensitivity::Public),
        }),
        "image/png",
        512,
        fingerprint("public"),
    )
    .unwrap();
    let mut asset =
        ManagedAsset::new(AssetId::new("asset-logo").unwrap(), "Logo", revision).unwrap();

    let draft_updates = asset.auth_updates();
    assert!(
        draft_updates.contains(&DefaultTupleUpdate::Delete(DefaultTuple::new(
            Entity::asset("asset-logo"),
            Relation::ReadPublic,
            DefaultSubject::entity(Entity::any_user()),
        )))
    );

    asset.publish_current();
    let published_updates = asset.auth_updates();
    assert!(
        published_updates.contains(&DefaultTupleUpdate::Write(DefaultTuple::new(
            Entity::asset("asset-logo"),
            Relation::ReadPublic,
            DefaultSubject::entity(Entity::any_user()),
        )))
    );
}

#[test]
fn private_assets_plan_authorized_delivery_from_storage_policy() {
    let planner = local_only_planner();
    let revision = ManagedAssetRevision::plan_with_single_node_escape_hatch(
        RevisionId::new("rev-local").unwrap(),
        &planner.single_node_escape_hatch(),
        "staff/exports/orders.csv",
        Some(StoragePolicyOverride::force_single_node_escape_hatch()),
        "text/csv",
        512,
        fingerprint("orders1"),
    )
    .unwrap();

    let asset = ManagedAsset::new(
        AssetId::new("asset-orders-export").unwrap(),
        "Orders export",
        revision,
    )
    .unwrap();

    let plan = asset
        .plan_authorized_delivery(
            &DeliveryContext::default().with_app_proxy_base("/admin/downloads"),
        )
        .unwrap();

    assert_eq!(plan.asset_kind(), AssetKind::ManagedAsset);
    assert_eq!(plan.audience(), DeliveryAudience::Authorized);
    assert_eq!(plan.delivery_mode(), DeliveryMode::LocalOnly);
    assert_eq!(plan.durable_store(), DurableStore::LocalDisk);
    assert_eq!(plan.sensitivity(), Sensitivity::Secret);
    assert_eq!(
        plan.target(),
        &AssetDeliveryTarget::LocalPath {
            path: "/srv/davenda/staff/exports/orders.csv".to_string(),
        }
    );
}
