use super::*;

fn fingerprint(value: &str) -> ContentFingerprint {
    ContentFingerprint::new(davenda_assets::FingerprintAlgorithm::Sha256, value).unwrap()
}

fn storage_public() -> StoragePolicy {
    StoragePolicy::public_asset()
}

fn storage_private() -> StoragePolicy {
    StoragePolicy::private_shared()
}

#[test]
fn metadata_and_revision_capture_the_core_media_fields() {
    let metadata = MediaMetadata::new("Spring Campaign")
        .unwrap()
        .with_alt_text("A bright hero image")
        .unwrap()
        .with_caption("Spring campaign hero")
        .unwrap()
        .with_description("Hero image used for the spring launch")
        .unwrap()
        .with_credit("OpenAI Studio")
        .unwrap()
        .with_dimensions(1200, 800)
        .with_tag("hero")
        .unwrap()
        .with_tag("campaign")
        .unwrap();

    let technical =
        MediaTechnicalMetadata::new("image/jpeg", 42_000, fingerprint("abc123")).unwrap();
    let revision = MediaAssetRevision::new(
        MediaRevisionId::new("rev-1").unwrap(),
        AssetId::new("asset-1").unwrap(),
        "media/spring/rev-1.jpg",
        storage_public(),
        technical,
        metadata,
    )
    .unwrap()
    .with_derivative(
        MediaDerivative::new(
            MediaDerivativeId::new("thumb").unwrap(),
            MediaDerivativeKind::Thumbnail,
            "Thumbnail",
            "image/webp",
            storage_public(),
        )
        .unwrap()
        .with_dimensions(400, 267),
    );

    assert_eq!(revision.metadata.title, "Spring Campaign");
    assert_eq!(revision.metadata.image_dimensions(), Some((1200, 800)));
    assert_eq!(revision.derivatives.len(), 1);
    assert!(revision.is_publicly_deliverable());
}

#[test]
fn folder_and_asset_policies_compose_in_order() {
    let library = MediaLibrary::new(
        MediaLibraryId::new("library").unwrap(),
        "Site media",
        storage_private(),
    )
    .unwrap();
    let folder = MediaFolder::new(
        MediaFolderId::new("folder").unwrap(),
        library.id.clone(),
        "Campaigns",
        MediaSlug::new("campaigns").unwrap(),
    )
    .unwrap()
    .with_storage_override(StoragePolicyOverride {
        delivery_mode: Some(DeliveryMode::SignedUrl),
        sync_mode: Some(davenda_storage::SyncMode::ObjectStore),
        sensitivity: Some(Sensitivity::Restricted),
    });

    let technical = MediaTechnicalMetadata::new("image/png", 1_024, fingerprint("def456")).unwrap();
    let revision = MediaAssetRevision::new(
        MediaRevisionId::new("rev-2").unwrap(),
        AssetId::new("asset-2").unwrap(),
        "media/campaigns/rev-2.png",
        storage_private(),
        technical,
        MediaMetadata::new("Campaign image").unwrap(),
    )
    .unwrap();
    let asset = MediaAsset::new(
        MediaAssetId::new("asset-media-1").unwrap(),
        library.id.clone(),
        "Campaign image",
        MediaSlug::new("campaign-image").unwrap(),
        revision,
    )
    .unwrap()
    .with_folder(folder.id.clone())
    .with_storage_override(StoragePolicyOverride::force_single_node_escape_hatch());

    let mut library = library;
    library.insert_folder(folder).unwrap();
    library.insert_asset(asset).unwrap();

    let policy = library
        .effective_storage_policy_for_asset(&MediaAssetId::new("asset-media-1").unwrap())
        .unwrap();

    assert_eq!(policy.delivery_mode, DeliveryMode::LocalOnly);
    assert_eq!(policy.sync_mode, davenda_storage::SyncMode::LocalOnly);
    assert_eq!(policy.sensitivity, Sensitivity::Secret);
}

#[test]
fn publication_state_emits_public_read_auth_tuples_for_public_assets() {
    let library = MediaLibrary::new(
        MediaLibraryId::new("library").unwrap(),
        "Site media",
        storage_public(),
    )
    .unwrap();
    let technical =
        MediaTechnicalMetadata::new("image/jpeg", 10_000, fingerprint("ghi789")).unwrap();
    let revision = MediaAssetRevision::new(
        MediaRevisionId::new("rev-3").unwrap(),
        AssetId::new("asset-3").unwrap(),
        "media/public/rev-3.jpg",
        storage_public(),
        technical,
        MediaMetadata::new("Public image").unwrap(),
    )
    .unwrap();
    let mut asset = MediaAsset::new(
        MediaAssetId::new("asset-media-2").unwrap(),
        library.id.clone(),
        "Public image",
        MediaSlug::new("public-image").unwrap(),
        revision,
    )
    .unwrap()
    .with_folder(MediaFolderId::new("folder-2").unwrap());
    asset.publish_current();

    let updates = asset.auth_updates();
    let expected = DefaultTupleUpdate::Write(DefaultTuple::new(
        Entity::asset("asset-3"),
        Relation::ReadPublic,
        DefaultSubject::entity(Entity::any_user()),
    ));

    assert!(updates.contains(&expected));
}

#[test]
fn replacement_workflow_tracks_staged_revisions() {
    let technical =
        MediaTechnicalMetadata::new("image/jpeg", 10_000, fingerprint("jkl012")).unwrap();
    let current = MediaAssetRevision::new(
        MediaRevisionId::new("rev-4").unwrap(),
        AssetId::new("asset-4").unwrap(),
        "media/current.jpg",
        storage_private(),
        technical.clone(),
        MediaMetadata::new("Current image").unwrap(),
    )
    .unwrap();
    let staged = MediaAssetRevision::new(
        MediaRevisionId::new("rev-5").unwrap(),
        AssetId::new("asset-5").unwrap(),
        "media/staged.jpg",
        storage_private(),
        technical,
        MediaMetadata::new("Replacement image").unwrap(),
    )
    .unwrap();
    let mut asset = MediaAsset::new(
        MediaAssetId::new("asset-media-3").unwrap(),
        MediaLibraryId::new("library").unwrap(),
        "Current image",
        MediaSlug::new("current-image").unwrap(),
        current,
    )
    .unwrap();

    asset.stage_replacement(staged);
    assert!(asset.staged_replacement.is_some());
    asset.apply_staged_replacement().unwrap();
    assert_eq!(asset.current_revision.metadata.title, "Replacement image");
    assert!(asset.staged_replacement.is_none());
}

#[test]
fn module_manifest_and_registration_match_first_party_patterns() {
    let module = MediaModule::default();
    let manifest = module.manifest();
    assert_eq!(manifest.name, "media");
    assert!(
        manifest
            .required_capabilities
            .contains(&Capability::AssetManageStorage)
    );
    assert!(
        manifest
            .optional_capabilities
            .contains(&Capability::AdminShellAccess)
    );
    assert_eq!(manifest.migrations.len(), 3);
    assert_eq!(manifest.route_surfaces.len(), 3);
    assert_eq!(manifest.http_surfaces.len(), 3);
    assert_eq!(manifest.jobs.len(), 2);
    assert_eq!(manifest.event_subscriptions.len(), 2);
    assert_eq!(manifest.admin_resources.len(), 2);
    assert_eq!(manifest.search_contributions.len(), 1);
    assert!(
        manifest
            .behaviors
            .contains(&ModuleBehavior::AuthGovernedPublication)
    );
    assert!(
        manifest
            .extension_slots
            .iter()
            .any(|slot| slot.kind == ExtensionSlotKind::AdminWidget)
    );
    assert_eq!(
        module
            .install_migration_plan()
            .expect("media migration plan")
            .ordered_steps()
            .len(),
        3
    );

    let mut registry = ServiceRegistry::new();
    module.register(&mut registry).unwrap();
    let service_ids = registry
        .services()
        .map(|service| service.id.clone())
        .collect::<Vec<_>>();
    assert!(service_ids.contains(&"module.media.assets".to_string()));
    assert!(service_ids.contains(&"module.media.storage".to_string()));
}
