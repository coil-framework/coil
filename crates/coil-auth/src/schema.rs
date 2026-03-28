use super::*;

pub fn default_schema() -> Schema {
    SchemaBuilder::new()
        .namespace(Namespace::Tenant.as_str(), top_level_namespace())
        .namespace(
            Namespace::Site.as_str(),
            inherited_namespace(Relation::Tenant),
        )
        .namespace(
            Namespace::Brand.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::Storefront.as_str(),
            storefront_namespace(Relation::Brand),
        )
        .namespace(Namespace::Group.as_str(), principal_set_namespace())
        .namespace(Namespace::Team.as_str(), principal_set_namespace())
        .namespace(
            Namespace::Page.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::Navigation.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::Product.as_str(),
            inherited_namespace(Relation::Storefront),
        )
        .namespace(
            Namespace::Collection.as_str(),
            inherited_namespace(Relation::Storefront),
        )
        .namespace(
            Namespace::Order.as_str(),
            order_namespace(Relation::Storefront),
        )
        .namespace(
            Namespace::Subscription.as_str(),
            inherited_namespace(Relation::Storefront),
        )
        .namespace(
            Namespace::MembershipTier.as_str(),
            inherited_namespace(Relation::Storefront),
        )
        .namespace(
            Namespace::Event.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::EventSlot.as_str(),
            event_slot_namespace(Relation::Event),
        )
        .namespace(
            Namespace::Booking.as_str(),
            booking_namespace(Relation::Slot),
        )
        .namespace(
            Namespace::MediaLibrary.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::Media.as_str(),
            media_namespace(Relation::Library),
        )
        .namespace(
            Namespace::AssetFolder.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(Namespace::Asset.as_str(), asset_namespace(Relation::Folder))
        .namespace(
            Namespace::ThemeAssetBundle.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::AdminModule.as_str(),
            admin_module_namespace(Relation::Site),
        )
        .build()
}

fn top_level_namespace() -> NamespaceConfig {
    NamespaceConfig {
        rules: permission_ladder(),
    }
}

fn principal_set_namespace() -> NamespaceConfig {
    let mut rules = permission_ladder();
    rules.insert(
        Relation::View.to_string(),
        vec![
            inherit(Relation::Member),
            inherit(Relation::Viewer),
            inherit(Relation::Support),
            inherit(Relation::Edit),
        ],
    );
    NamespaceConfig { rules }
}

fn inherited_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    NamespaceConfig { rules }
}

fn storefront_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(
        Relation::Checkout.to_string(),
        vec![inherit(Relation::View), inherit(Relation::Member)],
    );
    NamespaceConfig { rules }
}

fn order_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(
        Relation::Refund.to_string(),
        vec![inherit(Relation::Manage), inherit(Relation::Support)],
    );
    NamespaceConfig { rules }
}

fn event_slot_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(
        Relation::Book.to_string(),
        vec![inherit(Relation::View), inherit(Relation::Member)],
    );
    NamespaceConfig { rules }
}

fn booking_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(
        Relation::CheckIn.to_string(),
        vec![inherit(Relation::Manage), inherit(Relation::Support)],
    );
    NamespaceConfig { rules }
}

fn media_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(Relation::Read.to_string(), vec![inherit(Relation::View)]);
    NamespaceConfig { rules }
}

fn admin_module_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(Relation::Read.to_string(), vec![inherit(Relation::View)]);
    NamespaceConfig { rules }
}

fn asset_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(Relation::Read.to_string(), vec![inherit(Relation::View)]);
    rules.insert(Relation::Replace.to_string(), vec![inherit(Relation::Edit)]);
    rules.insert(
        Relation::Delete.to_string(),
        vec![inherit(Relation::Manage)],
    );
    rules.insert(
        Relation::Unpublish.to_string(),
        vec![inherit(Relation::Manage)],
    );
    rules.insert(
        Relation::ManageStorage.to_string(),
        vec![inherit(Relation::Manage)],
    );
    NamespaceConfig { rules }
}

fn permission_ladder() -> HashMap<String, Vec<RelationRule>> {
    HashMap::from([
        (
            Relation::Manage.to_string(),
            vec![inherit(Relation::Owner), inherit(Relation::Admin)],
        ),
        (
            Relation::Publish.to_string(),
            vec![inherit(Relation::Manage), inherit(Relation::Editor)],
        ),
        (
            Relation::Edit.to_string(),
            vec![inherit(Relation::Publish), inherit(Relation::Editor)],
        ),
        (
            Relation::View.to_string(),
            vec![
                inherit(Relation::Edit),
                inherit(Relation::Viewer),
                inherit(Relation::Support),
            ],
        ),
    ])
}

fn add_inherited_roles(rules: &mut HashMap<String, Vec<RelationRule>>, link_relation: Relation) {
    for relation in [
        Relation::Member,
        Relation::Owner,
        Relation::Admin,
        Relation::Editor,
        Relation::Viewer,
        Relation::Support,
    ] {
        rules.insert(
            relation.to_string(),
            vec![computed(link_relation, relation)],
        );
    }
}

fn inherit(relation: Relation) -> RelationRule {
    RelationRule::Inherit(relation.to_string())
}

fn computed(tuple_relation: Relation, target_relation: Relation) -> RelationRule {
    RelationRule::Computed {
        tuple_relation: tuple_relation.to_string(),
        target_relation: target_relation.to_string(),
    }
}
