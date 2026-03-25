use super::*;

#[test]
fn entity_converts_to_object() {
    let site = Entity::site("london");
    let object = site.to_object();

    assert_eq!(object.namespace, "site");
    assert_eq!(object.id, "london");
    assert_eq!(site.to_string(), "site:london");
}

#[test]
fn typed_subject_and_tuple_convert_to_zanzibar_types() {
    let team = Entity::team("ops");
    let asset = Entity::asset("hero-image");
    let subject = DefaultSubject::userset(team.clone(), Relation::Member);
    let tuple = DefaultTuple::new(asset.clone(), Relation::Viewer, subject.clone());

    assert_eq!(
        subject.to_subject(),
        Subject::Userset {
            object: team.to_object(),
            relation: "member".into(),
        }
    );
    assert_eq!(
        tuple.to_tuple(),
        Tuple {
            object: asset.to_object(),
            relation: "viewer".into(),
            subject: Subject::Userset {
                object: Entity::team("ops").to_object(),
                relation: "member".into(),
            },
        }
    );
}

#[test]
fn default_manifest_tracks_independent_versions() {
    let manifest = default_manifest();

    assert_eq!(manifest.name, "platform-default-auth");
    assert_eq!(manifest.version.to_string(), "1.0.0");
    assert_eq!(manifest.mode, PackageMode::Replace);
    assert_eq!(manifest.storage_schema_version, 1);
    assert_eq!(manifest.model_version, 1);
    assert_eq!(manifest.capability_binding_version, 1);
}

#[test]
fn default_schema_contains_expected_default_namespaces() {
    let schema = default_schema();

    for namespace in [
        Namespace::Tenant,
        Namespace::Site,
        Namespace::Brand,
        Namespace::Storefront,
        Namespace::Page,
        Namespace::Navigation,
        Namespace::Product,
        Namespace::Collection,
        Namespace::Order,
        Namespace::Subscription,
        Namespace::MembershipTier,
        Namespace::Event,
        Namespace::EventSlot,
        Namespace::Booking,
        Namespace::Media,
        Namespace::MediaLibrary,
        Namespace::Asset,
        Namespace::AssetFolder,
        Namespace::ThemeAssetBundle,
        Namespace::AdminModule,
    ] {
        assert!(schema.namespaces.contains_key(namespace.as_str()));
    }
}

#[test]
fn page_namespace_inherits_roles_from_site() {
    let schema = default_schema();
    let page_namespace = schema.namespaces.get("page").unwrap();

    assert_eq!(
        page_namespace.rules.get("viewer"),
        Some(&vec![RelationRule::Computed {
            tuple_relation: "site".into(),
            target_relation: "viewer".into(),
        }])
    );
    assert_eq!(
        page_namespace.rules.get("publish"),
        Some(&vec![
            RelationRule::Inherit("manage".into()),
            RelationRule::Inherit("editor".into()),
        ])
    );
}

#[test]
fn asset_namespace_contains_storage_and_publication_rules() {
    let schema = default_schema();
    let asset_namespace = schema.namespaces.get("asset").unwrap();

    assert_eq!(
        asset_namespace.rules.get("replace"),
        Some(&vec![RelationRule::Inherit("edit".into())])
    );
    assert_eq!(
        asset_namespace.rules.get("manage_storage"),
        Some(&vec![RelationRule::Inherit("manage".into())])
    );
    assert!(
        !asset_namespace.rules.contains_key("read_public"),
        "read_public is intentionally left as a direct relation so publication state can be written explicitly"
    );
}

#[test]
fn storefront_and_order_namespaces_include_commerce_permissions() {
    let schema = default_schema();
    let storefront = schema.namespaces.get("storefront").unwrap();
    let order = schema.namespaces.get("order").unwrap();

    assert_eq!(
        storefront.rules.get("checkout"),
        Some(&vec![
            RelationRule::Inherit("view".into()),
            RelationRule::Inherit("member".into()),
        ])
    );
    assert_eq!(
        order.rules.get("refund"),
        Some(&vec![
            RelationRule::Inherit("manage".into()),
            RelationRule::Inherit("support".into()),
        ])
    );
}

#[test]
fn capability_registry_contains_expected_bindings() {
    let package = DefaultAuthModelPackage::default();

    let page_publish = package.binding_for(Capability::CmsPagePublish).unwrap();
    assert_eq!(page_publish.resource_namespaces, vec![Namespace::Page]);
    assert_eq!(page_publish.relation, Relation::Publish);

    let booking_create = package
        .binding_for(Capability::EventsBookingCreate)
        .unwrap();
    assert_eq!(
        booking_create.resource_namespaces,
        vec![Namespace::EventSlot]
    );
    assert_eq!(booking_create.relation, Relation::Book);

    let booking_manage = package
        .binding_for(Capability::EventsBookingManage)
        .unwrap();
    assert_eq!(booking_manage.resource_namespaces, vec![Namespace::Booking]);
    assert_eq!(booking_manage.relation, Relation::Manage);
}

#[test]
fn checked_in_customer_auth_package_loads_real_manifest_bindings_and_schema_extensions() {
    let app_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("apps")
        .join("harbor-shop");
    let package = load_auth_model_package_at("harbor-auth", &app_root).unwrap();

    assert_eq!(package.manifest().name, "harbor-auth");
    assert_eq!(package.manifest().mode, PackageMode::Extend);
    assert_eq!(
        package.manifest().imports,
        vec!["platform-default-auth".to_string()]
    );
    assert_eq!(
        package
            .binding_for(Capability::CatalogFeaturedEdit)
            .unwrap(),
        &CapabilityBinding {
            capability: Capability::CatalogFeaturedEdit,
            resource_namespaces: vec![Namespace::Product],
            relation: Relation::FeaturedEdit,
        }
    );
    assert_eq!(
        package
            .schema()
            .namespaces
            .get(Namespace::Product.as_str())
            .unwrap()
            .rules
            .get(Relation::FeaturedEdit.as_str()),
        Some(&vec![RelationRule::Inherit(
            Relation::Merchandiser.as_str().into()
        )])
    );
    assert_eq!(
        package.binding_for(Capability::CmsPageRead).unwrap(),
        DefaultAuthModelPackage::default()
            .binding_for(Capability::CmsPageRead)
            .unwrap()
    );
}

#[test]
fn capability_parser_accepts_checked_in_import_mapping_names() {
    assert_eq!(
        Capability::from_str("events.booking.manage"),
        Some(Capability::EventsBookingManage)
    );
    assert_eq!(
        Capability::from_str("cms.page.publish"),
        Some(Capability::CmsPagePublish)
    );
}

#[test]
fn capability_resolution_rejects_wrong_namespace() {
    let package = DefaultAuthModelPackage::default();
    let result = package.resolve_binding(Capability::CmsPagePublish, &Entity::product("sku-1"));

    match result {
        Err(DavendaAuthError::ResourceNamespaceMismatch {
            capability,
            actual,
            expected,
        }) => {
            assert_eq!(capability, Capability::CmsPagePublish);
            assert_eq!(actual, Namespace::Product);
            assert_eq!(expected, vec![Namespace::Page]);
        }
        other => panic!("expected namespace mismatch, got {other:?}"),
    }
}

#[test]
fn explain_capability_returns_allow_path_for_inherited_access() {
    let package = DefaultAuthModelPackage::default();
    let page = Entity::page("homepage");
    let site = Entity::site("main");
    let subject = DefaultSubject::entity(Entity::user("alice"));
    let tuples = vec![
        DefaultTuple::new(page.clone(), Relation::Site, site.clone().as_subject()).to_tuple(),
        DefaultTuple::new(site.clone(), Relation::Viewer, subject.clone()).to_tuple(),
    ];

    let explanation = build_capability_explanation(
        &package,
        &tuples,
        &subject,
        Capability::CmsPageRead,
        &page,
        ExplainOptions::default(),
    )
    .unwrap();

    assert_eq!(explanation.decision, ExplainDecision::Allow);
    assert_eq!(explanation.binding.relation, Relation::View);

    let ExplainTrace::Allowed(trace) = explanation.trace else {
        panic!("expected allow trace");
    };
    assert_eq!(
        trace.steps.first(),
        Some(&ExplainStep::Start {
            node: ExplainedNode {
                object: page.clone(),
                relation: Some(Relation::View),
            },
        })
    );
    assert!(trace.steps.iter().any(|step| matches!(
        step,
        ExplainStep::Computed {
            from,
            via_tuple,
            to,
        } if *from == ExplainedNode {
            object: page.clone(),
            relation: Some(Relation::Viewer),
        } && *via_tuple == DefaultTuple::new(
            page.clone(),
            Relation::Site,
            site.clone().as_subject(),
        ) && *to == ExplainedNode {
            object: site.clone(),
            relation: Some(Relation::Viewer),
        }
    )));
    assert!(trace.steps.iter().any(|step| matches!(
        step,
        ExplainStep::TupleSubjectMatch { from, tuple }
            if *from == ExplainedNode {
                object: site.clone(),
                relation: Some(Relation::Viewer),
            } && *tuple == DefaultTuple::new(site.clone(), Relation::Viewer, subject.clone())
    )));
}

#[test]
fn explain_capability_returns_deny_tree_when_required_publish_path_is_missing() {
    let package = DefaultAuthModelPackage::default();
    let page = Entity::page("homepage");
    let site = Entity::site("main");
    let subject = DefaultSubject::entity(Entity::user("alice"));
    let tuples = vec![
        DefaultTuple::new(page.clone(), Relation::Site, site.clone().as_subject()).to_tuple(),
        DefaultTuple::new(site.clone(), Relation::Viewer, subject.clone()).to_tuple(),
    ];

    let explanation = build_capability_explanation(
        &package,
        &tuples,
        &subject,
        Capability::CmsPagePublish,
        &page,
        ExplainOptions::default(),
    )
    .unwrap();

    assert_eq!(explanation.decision, ExplainDecision::Deny);

    let ExplainTrace::Denied(trace) = explanation.trace else {
        panic!("expected deny trace");
    };
    assert_eq!(
        trace.node,
        ExplainedNode {
            object: page.clone(),
            relation: Some(Relation::Publish),
        }
    );
    assert_eq!(trace.reason, DeniedReason::NoMatchingPath);
    assert_eq!(trace.attempts.len(), 2);
    assert!(
        trace
            .attempts
            .iter()
            .all(|attempt| matches!(attempt, DeniedAttempt::Inherit { .. }))
    );
}

#[test]
fn explain_capability_honors_public_wildcard_subjects() {
    let package = DefaultAuthModelPackage::default();
    let asset = Entity::asset("hero");
    let subject = DefaultSubject::entity(Entity::user("alice"));
    let tuples = vec![
        DefaultTuple::new(
            asset.clone(),
            Relation::ReadPublic,
            DefaultSubject::entity(Entity::any_user()),
        )
        .to_tuple(),
    ];

    let explanation = build_capability_explanation(
        &package,
        &tuples,
        &subject,
        Capability::AssetReadPublic,
        &asset,
        ExplainOptions::default(),
    )
    .unwrap();

    assert_eq!(explanation.decision, ExplainDecision::Allow);

    let ExplainTrace::Allowed(trace) = explanation.trace else {
        panic!("expected allow trace");
    };
    assert!(trace.steps.iter().any(|step| matches!(
        step,
        ExplainStep::TupleSubjectMatch { tuple, .. }
            if *tuple == DefaultTuple::new(
                asset.clone(),
                Relation::ReadPublic,
                DefaultSubject::entity(Entity::any_user()),
            )
    )));
}

#[test]
fn explain_capability_reports_recursion_limit_when_depth_budget_is_exhausted() {
    let package = DefaultAuthModelPackage::default();
    let page = Entity::page("homepage");
    let site = Entity::site("main");
    let subject = DefaultSubject::entity(Entity::user("alice"));
    let tuples = vec![
        DefaultTuple::new(page.clone(), Relation::Site, site.clone().as_subject()).to_tuple(),
        DefaultTuple::new(site.clone(), Relation::Viewer, subject.clone()).to_tuple(),
    ];

    let explanation = build_capability_explanation(
        &package,
        &tuples,
        &subject,
        Capability::CmsPageRead,
        &page,
        ExplainOptions::new(1),
    )
    .unwrap();

    assert_eq!(explanation.decision, ExplainDecision::Deny);

    let ExplainTrace::Denied(trace) = explanation.trace else {
        panic!("expected deny trace");
    };
    assert_eq!(
        trace.reason,
        DeniedReason::RecursionLimitReached { max_depth: 1 }
    );
}
