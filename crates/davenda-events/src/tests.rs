use super::*;

fn event_context() -> EventAccessContext {
    EventAccessContext::public().with_authenticated(true)
}

fn published_event() -> Result<EventRecord, EventModelError> {
    let mut event = EventRecord::new(
        EventId::new("event-spring-tasting").unwrap(),
        "site-main",
        EventHandle::new("spring-tasting").unwrap(),
        "Spring Tasting",
        "A seasonal tasting event",
        "<p>Seasonal tasting</p>",
        "/events/spring-tasting",
        EventVisibility::MembersOnly,
        EventEligibilityRule::AnyOf(vec![
            EventEligibilityRule::RequiresMembershipTierAny(vec![
                MembershipTierId::new("tier-premium").unwrap(),
            ]),
            EventEligibilityRule::RequiresCapability(Capability::EventsBookingCreate),
        ]),
    )?;
    event = event.with_seo("Spring Tasting", "Seasonal tasting event")?;
    event.publish();
    Ok(event)
}

fn sample_slot() -> Result<EventSlot, EventModelError> {
    EventSlot::new(
        EventSlotId::new("slot-evening").unwrap(),
        "Evening session",
        EventInstant::from_unix_seconds(1_000),
        EventInstant::from_unix_seconds(1_600),
        1,
        3,
        Duration::from_secs(600),
        true,
    )
}

#[test]
fn event_catalog_handles_reservations_waitlist_and_check_in() {
    let mut catalog = EventCatalog::new();
    let event = published_event().unwrap();
    let event_id = event.id.clone();
    let slot_id = EventSlotId::new("slot-evening").unwrap();
    catalog.insert_event(event).unwrap();
    catalog.add_slot(&event_id, sample_slot().unwrap()).unwrap();

    let held = catalog
        .reserve_slot(
            &event_id,
            &slot_id,
            Some(AttendeeId::new("attendee-1").unwrap()),
            BookingSource::Manual,
            EventInstant::from_unix_seconds(1_010),
            &event_context().with_capability(Capability::EventsBookingCreate),
        )
        .unwrap();

    let reservation_id = match held {
        ReservationOutcome::Held(reservation) => {
            assert_eq!(reservation.status, ReservationStatus::Held);
            reservation.id
        }
        other => panic!("expected held reservation, got {other:?}"),
    };

    let booking = catalog
        .confirm_reservation(
            &reservation_id,
            None,
            EventInstant::from_unix_seconds(1_020),
        )
        .unwrap();
    assert_eq!(booking.status, BookingStatus::Confirmed);

    let waitlisted = catalog
        .reserve_slot(
            &event_id,
            &slot_id,
            Some(AttendeeId::new("attendee-2").unwrap()),
            BookingSource::CommerceOrder {
                order_id: OrderId::new("order-1").unwrap(),
            },
            EventInstant::from_unix_seconds(1_030),
            &event_context().with_capability(Capability::EventsBookingCreate),
        )
        .unwrap();
    let waitlist_id = match waitlisted {
        ReservationOutcome::Waitlisted(entry) => {
            assert_eq!(entry.status, WaitlistStatus::Waiting);
            entry.id
        }
        other => panic!("expected waitlist entry, got {other:?}"),
    };

    let outcome = catalog
        .cancel_booking(&booking.id, EventInstant::from_unix_seconds(1_040))
        .unwrap();
    assert_eq!(outcome.booking.status, BookingStatus::Cancelled);
    assert_eq!(outcome.promoted_reservations.len(), 1);
    assert_eq!(
        outcome.promoted_reservations[0].waitlist_entry_id,
        Some(waitlist_id)
    );

    let promoted_reservation_id = outcome.promoted_reservations[0].id.clone();
    let promoted_booking = catalog
        .confirm_reservation(
            &promoted_reservation_id,
            None,
            EventInstant::from_unix_seconds(1_050),
        )
        .unwrap();
    assert_eq!(
        promoted_booking.source.kind(),
        BookingSourceKind::CommerceOrder
    );

    catalog
        .check_in_booking(
            &promoted_booking.id,
            EventInstant::from_unix_seconds(1_060),
            &OperatorAccessContext::new().with_capability(Capability::EventsBookingCheckIn),
        )
        .unwrap();
}

#[test]
fn events_module_exposes_query_and_transaction_plans_for_bookings() {
    let mut catalog = EventCatalog::new();
    let event = published_event().unwrap();
    let event_id = event.id.clone();
    let slot_id = EventSlotId::new("slot-evening").unwrap();
    catalog.insert_event(event).unwrap();
    catalog.add_slot(&event_id, sample_slot().unwrap()).unwrap();

    let listing = catalog.public_listing_query(Some("fr-FR")).unwrap();
    assert_eq!(listing.query.context.locale.as_deref(), Some("fr-FR"));
    assert_eq!(listing.query.context.cache_scope, QueryCacheScope::Public);
    assert_eq!(listing.query.filters.len(), 1);

    let reservation = match catalog
        .reserve_slot(
            &event_id,
            &slot_id,
            Some(AttendeeId::new("attendee-1").unwrap()),
            BookingSource::Manual,
            EventInstant::from_unix_seconds(1_010),
            &event_context().with_capability(Capability::EventsBookingCreate),
        )
        .unwrap()
    {
        ReservationOutcome::Held(reservation) => reservation,
        other => panic!("expected held reservation, got {other:?}"),
    };
    let reservation_tx = catalog.reservation_transaction_plan(&reservation).unwrap();
    assert_eq!(reservation_tx.isolation, TransactionIsolation::Serializable);
    assert_eq!(
        reservation_tx.after_commit_jobs,
        vec!["events.reservations.expiry".to_string()]
    );

    let booking = catalog
        .confirm_reservation(
            &reservation.id,
            None,
            EventInstant::from_unix_seconds(1_020),
        )
        .unwrap();
    let confirmation_tx = catalog
        .booking_confirmation_transaction_plan(&booking)
        .unwrap();
    assert_eq!(confirmation_tx.writes.len(), 3);
    assert_eq!(
        confirmation_tx.after_commit_events,
        vec!["events.booking.confirmed".to_string()]
    );

    let cancellation = catalog
        .cancel_booking(&booking.id, EventInstant::from_unix_seconds(1_030))
        .unwrap();
    let cancellation_tx = catalog
        .booking_cancellation_transaction_plan(&cancellation)
        .unwrap();
    assert_eq!(
        cancellation_tx.after_commit_events,
        vec!["events.booking.cancelled".to_string()]
    );
}

#[test]
fn membership_tier_rules_gate_booking_access() {
    let event = published_event().unwrap();
    let slot_id = EventSlotId::new("slot-evening").unwrap();
    let mut catalog = EventCatalog::new();
    catalog.insert_event(event.clone()).unwrap();
    catalog.add_slot(&event.id, sample_slot().unwrap()).unwrap();

    let denied = catalog.reserve_slot(
        &event.id,
        &slot_id,
        None,
        BookingSource::MembershipEntitlement {
            tier_id: MembershipTierId::new("tier-basic").unwrap(),
        },
        EventInstant::from_unix_seconds(1_010),
        &EventAccessContext::default()
            .with_authenticated(true)
            .with_membership_tier(MembershipTierId::new("tier-basic").unwrap()),
    );
    assert!(matches!(
        denied,
        Err(EventModelError::EligibilityDenied { .. })
    ));

    let allowed = catalog.reserve_slot(
        &event.id,
        &slot_id,
        None,
        BookingSource::MembershipEntitlement {
            tier_id: MembershipTierId::new("tier-premium").unwrap(),
        },
        EventInstant::from_unix_seconds(1_010),
        &EventAccessContext::default()
            .with_authenticated(true)
            .with_membership_tier(MembershipTierId::new("tier-premium").unwrap()),
    );
    assert!(matches!(allowed, Ok(ReservationOutcome::Held(_))));
}

#[test]
fn module_manifest_and_admin_resources_match_events_workloads() {
    let module = EventsModule::new();
    let manifest = module.manifest();
    let mut registry = ServiceRegistry::new();

    module.register(&mut registry).unwrap();

    assert_eq!(manifest.name, "events");
    assert_eq!(manifest.config_namespace.as_deref(), Some("events"));
    assert!(
        manifest
            .required_capabilities
            .contains(&Capability::EventsBookingCheckIn)
    );
    assert!(
        manifest
            .optional_capabilities
            .contains(&Capability::MembershipSubscriptionManage)
    );
    assert_eq!(manifest.migrations.len(), 3);
    assert_eq!(manifest.route_surfaces.len(), 6);
    assert_eq!(manifest.http_surfaces.len(), 6);
    assert_eq!(manifest.jobs.len(), 3);
    assert_eq!(manifest.event_subscriptions.len(), 2);
    assert_eq!(manifest.admin_resources.len(), 4);
    assert_eq!(manifest.search_contributions.len(), 2);
    assert_eq!(manifest.report_definitions.len(), 1);
    assert_eq!(manifest.bulk_operations.len(), 1);
    assert!(
        manifest
            .module_dependencies
            .iter()
            .any(|dependency| dependency.module == "commerce")
    );
    assert!(
        manifest
            .core_service_dependencies
            .contains(&CoreServiceDependency::Jobs)
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
            .expect("events migration plan")
            .ordered_steps()
            .len(),
        3
    );
    assert!(
        module
            .install_migration_plan()
            .expect("events migration plan")
            .ordered_steps()[0]
            .statements
            .iter()
            .any(|statement| statement.contains("CREATE TABLE IF NOT EXISTS events_catalog"))
    );
    assert!(
        registry
            .services()
            .any(|service| service.id == "module.events.waitlists")
    );
    assert_eq!(module.admin_resources().len(), 4);
}

#[test]
fn auth_projection_links_events_slots_and_bookings() {
    let mut catalog = EventCatalog::new();
    let mut event = published_event().unwrap();
    let event_id = event.id.clone();
    let slot = sample_slot().unwrap();
    let slot_id = slot.id.clone();
    event.add_slot(slot).unwrap();
    catalog.insert_event(event).unwrap();

    let reservation = match catalog
        .reserve_slot(
            &event_id,
            &slot_id,
            None,
            BookingSource::Manual,
            EventInstant::from_unix_seconds(1_010),
            &event_context().with_capability(Capability::EventsBookingCreate),
        )
        .unwrap()
    {
        ReservationOutcome::Held(reservation) => reservation,
        other => panic!("expected reservation, got {other:?}"),
    };

    let booking = catalog
        .confirm_reservation(
            &reservation.id,
            None,
            EventInstant::from_unix_seconds(1_020),
        )
        .unwrap();

    let projection = catalog.auth_projection();
    assert!(projection.iter().any(|update| matches!(
        update,
        DefaultTupleUpdate::Write(DefaultTuple {
            object: Entity::Event(_),
            relation: Relation::Site,
            subject: DefaultSubject::Entity(Entity::Site(_)),
        })
    )));
    assert!(projection.iter().any(|update| matches!(
        update,
        DefaultTupleUpdate::Write(DefaultTuple {
            object: Entity::EventSlot(_),
            relation: Relation::Event,
            subject: DefaultSubject::Entity(Entity::Event(_)),
        })
    )));
    assert!(projection.iter().any(|update| matches!(
        update,
        DefaultTupleUpdate::Write(DefaultTuple {
            object: Entity::Booking(_),
            relation: Relation::Slot,
            subject: DefaultSubject::Entity(Entity::EventSlot(_)),
        })
    )));
    assert_eq!(booking.status, BookingStatus::Confirmed);
}
