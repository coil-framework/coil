use super::*;

pub struct EventListingQuery {
    pub query: QuerySpec,
    pub include_slots: bool,
    pub include_membership_pricing: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventCatalog {
    pub(super) events: BTreeMap<EventId, EventRecord>,
    pub(super) reservations: BTreeMap<ReservationId, Reservation>,
    pub(super) bookings: BTreeMap<BookingId, Booking>,
    pub(super) waitlist_entries: BTreeMap<WaitlistEntryId, WaitlistEntry>,
    pub(super) waitlists: BTreeMap<EventSlotId, VecDeque<WaitlistEntryId>>,
    pub(super) next_reservation_seq: u64,
    pub(super) next_booking_seq: u64,
    pub(super) next_waitlist_seq: u64,
}

impl EventCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_event(&mut self, event: EventRecord) -> Result<(), EventModelError> {
        if self.events.contains_key(&event.id) {
            return Err(EventModelError::DuplicateEvent {
                event_id: event.id.to_string(),
            });
        }

        if self
            .events
            .values()
            .any(|existing| existing.handle == event.handle)
        {
            return Err(EventModelError::DuplicateEvent {
                event_id: event.handle.to_string(),
            });
        }

        self.events.insert(event.id.clone(), event);
        Ok(())
    }

    pub fn event(&self, event_id: &EventId) -> Option<&EventRecord> {
        self.events.get(event_id)
    }

    pub fn event_mut(&mut self, event_id: &EventId) -> Option<&mut EventRecord> {
        self.events.get_mut(event_id)
    }

    pub fn event_required(&self, event_id: &EventId) -> Result<&EventRecord, EventModelError> {
        self.event(event_id)
            .ok_or_else(|| EventModelError::MissingEvent {
                event_id: event_id.to_string(),
            })
    }

    pub fn event_required_mut(
        &mut self,
        event_id: &EventId,
    ) -> Result<&mut EventRecord, EventModelError> {
        self.event_mut(event_id)
            .ok_or_else(|| EventModelError::MissingEvent {
                event_id: event_id.to_string(),
            })
    }

    pub fn public_listing_query(
        &self,
        locale: Option<&str>,
    ) -> Result<EventListingQuery, EventModelError> {
        let query = QuerySpec::new(
            PageRequest::new(0, 24)?,
            QueryContext {
                locale: locale.map(ToString::to_string),
                principal_id: None,
                publication_visibility: PublicationVisibility::PublishedOnly,
                cache_scope: QueryCacheScope::Public,
            },
        )
        .with_filter(QueryFilter::new(
            "event_state",
            FilterOperator::Eq,
            vec!["live".to_string()],
        )?)
        .with_sort(QuerySort::ascending("starts_at")?);

        Ok(EventListingQuery {
            query,
            include_slots: true,
            include_membership_pricing: true,
        })
    }

    pub fn add_slot(&mut self, event_id: &EventId, slot: EventSlot) -> Result<(), EventModelError> {
        let event = self.event_required_mut(event_id)?;
        event.add_slot(slot)
    }

    pub fn slot_required(
        &self,
        event_id: &EventId,
        slot_id: &EventSlotId,
    ) -> Result<&EventSlot, EventModelError> {
        self.event_required(event_id)?
            .slot(slot_id)
            .ok_or_else(|| EventModelError::MissingSlot {
                event_id: event_id.to_string(),
                slot_id: slot_id.to_string(),
            })
    }

    pub fn slot_required_mut(
        &mut self,
        event_id: &EventId,
        slot_id: &EventSlotId,
    ) -> Result<&mut EventSlot, EventModelError> {
        self.event_required_mut(event_id)?
            .slot_mut(slot_id)
            .ok_or_else(|| EventModelError::MissingSlot {
                event_id: event_id.to_string(),
                slot_id: slot_id.to_string(),
            })
    }

    pub fn reservation_transaction_plan(
        &self,
        reservation: &Reservation,
    ) -> Result<TransactionPlan, EventModelError> {
        TransactionPlan::new(
            "events.reservation.hold",
            TransactionIsolation::Serializable,
        )?
        .with_write(DomainWrite::new("events.reservations", "insert")?)
        .with_write(DomainWrite::new("events.slots", "hold_capacity")?)
        .with_after_commit_job("events.reservations.expiry")?
        .with_after_commit_event(format!("events.reservation.{}", reservation.status))
        .map_err(EventModelError::from)
    }

    pub fn booking_confirmation_transaction_plan(
        &self,
        booking: &Booking,
    ) -> Result<TransactionPlan, EventModelError> {
        TransactionPlan::new("events.booking.confirm", TransactionIsolation::Serializable)?
            .with_write(DomainWrite::new("events.bookings", "insert")?)
            .with_write(DomainWrite::new("events.reservations", "confirm")?)
            .with_write(DomainWrite::new("events.slots", "book_capacity")?)
            .with_after_commit_job("events.bookings.confirmation_mail")?
            .with_after_commit_event(format!("events.booking.{}", booking.status))
            .map_err(EventModelError::from)
    }

    pub fn booking_cancellation_transaction_plan(
        &self,
        outcome: &BookingCancellationOutcome,
    ) -> Result<TransactionPlan, EventModelError> {
        let mut transaction =
            TransactionPlan::new("events.booking.cancel", TransactionIsolation::Serializable)?
                .with_write(DomainWrite::new("events.bookings", "cancel")?)
                .with_write(DomainWrite::new("events.slots", "release_capacity")?)
                .with_after_commit_event("events.booking.cancelled")?;

        if !outcome.promoted_reservations.is_empty() {
            transaction = transaction.with_after_commit_job("events.waitlist.promotions")?;
        }

        Ok(transaction)
    }

    pub fn check_in_transaction_plan(
        &self,
        booking: &Booking,
    ) -> Result<TransactionPlan, EventModelError> {
        TransactionPlan::new(
            "events.booking.check_in",
            TransactionIsolation::Serializable,
        )?
        .with_write(DomainWrite::new("events.bookings", "check_in")?)
        .with_write(DomainWrite::new("events.slots", "check_in")?)
        .with_after_commit_event(format!("events.booking.{}", booking.status))
        .map_err(EventModelError::from)
    }

    pub fn reservations(&self) -> impl Iterator<Item = &Reservation> {
        self.reservations.values()
    }

    pub fn bookings(&self) -> impl Iterator<Item = &Booking> {
        self.bookings.values()
    }

    pub fn waitlist_entries(&self) -> impl Iterator<Item = &WaitlistEntry> {
        self.waitlist_entries.values()
    }

    pub fn auth_projection(&self) -> Vec<DefaultTupleUpdate> {
        let mut updates = Vec::new();
        for event in self.events.values() {
            updates.extend(event.auth_projection());
            for booking in self
                .bookings
                .values()
                .filter(|booking| booking.event_id == event.id)
            {
                updates.push(DefaultTupleUpdate::Write(DefaultTuple::new(
                    Entity::booking(booking.id.to_string()),
                    Relation::Slot,
                    DefaultSubject::entity(Entity::event_slot(booking.slot_id.to_string())),
                )));
            }
        }
        updates
    }

    pub(super) fn next_reservation_id(
        &mut self,
        event_id: &EventId,
        slot_id: &EventSlotId,
    ) -> ReservationId {
        self.next_reservation_seq += 1;
        ReservationId::new(format!(
            "res-{}-{}-{}",
            event_id.as_str(),
            slot_id.as_str(),
            self.next_reservation_seq
        ))
        .expect("generated reservation id is valid")
    }

    pub(super) fn next_booking_id(
        &mut self,
        event_id: &EventId,
        slot_id: &EventSlotId,
    ) -> BookingId {
        self.next_booking_seq += 1;
        BookingId::new(format!(
            "book-{}-{}-{}",
            event_id.as_str(),
            slot_id.as_str(),
            self.next_booking_seq
        ))
        .expect("generated booking id is valid")
    }

    pub(super) fn next_waitlist_id(
        &mut self,
        event_id: &EventId,
        slot_id: &EventSlotId,
    ) -> WaitlistEntryId {
        self.next_waitlist_seq += 1;
        WaitlistEntryId::new(format!(
            "wait-{}-{}-{}",
            event_id.as_str(),
            slot_id.as_str(),
            self.next_waitlist_seq
        ))
        .expect("generated waitlist id is valid")
    }
}
