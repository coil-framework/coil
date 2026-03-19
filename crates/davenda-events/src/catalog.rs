use super::*;

pub struct EventListingQuery {
    pub query: QuerySpec,
    pub include_slots: bool,
    pub include_membership_pricing: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventCatalog {
    events: BTreeMap<EventId, EventRecord>,
    reservations: BTreeMap<ReservationId, Reservation>,
    bookings: BTreeMap<BookingId, Booking>,
    waitlist_entries: BTreeMap<WaitlistEntryId, WaitlistEntry>,
    waitlists: BTreeMap<EventSlotId, VecDeque<WaitlistEntryId>>,
    next_reservation_seq: u64,
    next_booking_seq: u64,
    next_waitlist_seq: u64,
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

    pub fn reserve_slot(
        &mut self,
        event_id: &EventId,
        slot_id: &EventSlotId,
        attendee_id: Option<AttendeeId>,
        source: BookingSource,
        now: EventInstant,
        context: &EventAccessContext,
    ) -> Result<ReservationOutcome, EventModelError> {
        let event = self.event_required(event_id)?;
        event.can_book(slot_id, context)?;

        let (available_capacity, waitlist_space, hold_duration, slot_title, slot_status) = {
            let slot = event
                .slot(slot_id)
                .ok_or_else(|| EventModelError::MissingSlot {
                    event_id: event_id.to_string(),
                    slot_id: slot_id.to_string(),
                })?;
            (
                slot.available_capacity(),
                slot.has_waitlist_space(),
                slot.hold_duration,
                slot.title.clone(),
                slot.state,
            )
        };

        if !slot_status.is_bookable() {
            return Err(EventModelError::SlotNotOpen {
                event_id: event_id.to_string(),
                slot_id: slot_id.to_string(),
                status: slot_status,
            });
        }

        if available_capacity > 0 {
            let reservation_id = self.next_reservation_id(event_id, slot_id);
            let expires_at = now.checked_add("reservation_expires_at", hold_duration)?;
            {
                let slot = self.slot_required_mut(event_id, slot_id)?;
                slot.reserve_hold()?;
            }

            let reservation = Reservation::held(
                reservation_id.clone(),
                event_id.clone(),
                slot_id.clone(),
                attendee_id,
                source,
                now,
                expires_at,
            );
            self.reservations
                .insert(reservation_id, reservation.clone());
            let _ = slot_title;
            Ok(ReservationOutcome::Held(reservation))
        } else if waitlist_space {
            let waitlist_id = self.next_waitlist_id(event_id, slot_id);
            let position = {
                let slot = self.slot_required(event_id, slot_id)?;
                slot.waitlisted_count() + 1
            };

            {
                let slot = self.slot_required_mut(event_id, slot_id)?;
                if !slot.can_waitlist() {
                    return Err(EventModelError::WaitlistFull {
                        event_id: event_id.to_string(),
                        slot_id: slot_id.to_string(),
                        capacity: slot.waitlist_capacity,
                    });
                }
                slot.waitlisted_count = slot
                    .waitlisted_count
                    .checked_add(1)
                    .expect("waitlist count overflow is not practical");
            }

            let entry = WaitlistEntry::new(
                waitlist_id.clone(),
                event_id.clone(),
                slot_id.clone(),
                attendee_id,
                source,
                now,
                position,
            );
            self.waitlists
                .entry(slot_id.clone())
                .or_default()
                .push_back(waitlist_id.clone());
            self.waitlist_entries.insert(waitlist_id, entry.clone());
            Ok(ReservationOutcome::Waitlisted(entry))
        } else {
            let slot = self.slot_required(event_id, slot_id)?;
            Err(EventModelError::CapacityExceeded {
                event_id: event_id.to_string(),
                slot_id: slot_id.to_string(),
                capacity: slot.capacity,
                booked: slot.booked_count,
                reserved: slot.reserved_count,
            })
        }
    }

    pub fn confirm_reservation(
        &mut self,
        reservation_id: &ReservationId,
        booking_id: Option<BookingId>,
        now: EventInstant,
    ) -> Result<Booking, EventModelError> {
        let reservation = self
            .reservations
            .get(reservation_id)
            .cloned()
            .ok_or_else(|| EventModelError::MissingReservation {
                reservation_id: reservation_id.to_string(),
            })?;

        if reservation.status != ReservationStatus::Held {
            return Err(EventModelError::InvalidReservationTransition {
                reservation_id: reservation.id.to_string(),
                from: reservation.status,
                to: ReservationStatus::Confirmed,
            });
        }

        if now > reservation.expires_at {
            return Err(EventModelError::ReservationExpired {
                reservation_id: reservation.id.to_string(),
                expires_at: reservation.expires_at,
                now,
            });
        }

        {
            let slot = self.slot_required_mut(&reservation.event_id, &reservation.slot_id)?;
            slot.confirm_hold()?;
        }

        let booking_id = booking_id
            .unwrap_or_else(|| self.next_booking_id(&reservation.event_id, &reservation.slot_id));
        let booking = Booking::from_reservation(booking_id.clone(), &reservation, now);
        self.bookings.insert(booking_id.clone(), booking.clone());

        let mut reservation = reservation;
        reservation.status = ReservationStatus::Confirmed;
        reservation.booking_id = Some(booking_id);
        self.reservations
            .insert(reservation.id.clone(), reservation);
        Ok(booking)
    }

    pub fn expire_reservation(
        &mut self,
        reservation_id: &ReservationId,
        now: EventInstant,
    ) -> Result<ReservationExpirationOutcome, EventModelError> {
        let reservation = self
            .reservations
            .get(reservation_id)
            .cloned()
            .ok_or_else(|| EventModelError::MissingReservation {
                reservation_id: reservation_id.to_string(),
            })?;

        if reservation.status != ReservationStatus::Held {
            return Err(EventModelError::InvalidReservationTransition {
                reservation_id: reservation.id.to_string(),
                from: reservation.status,
                to: ReservationStatus::Expired,
            });
        }

        if now < reservation.expires_at {
            return Err(EventModelError::ReservationExpired {
                reservation_id: reservation.id.to_string(),
                expires_at: reservation.expires_at,
                now,
            });
        }

        {
            let slot = self.slot_required_mut(&reservation.event_id, &reservation.slot_id)?;
            slot.release_hold();
        }

        let mut reservation = reservation;
        reservation.status = ReservationStatus::Expired;
        self.reservations
            .insert(reservation.id.clone(), reservation.clone());
        let promoted_reservations =
            self.promote_waitlist(&reservation.event_id, &reservation.slot_id, now)?;

        Ok(ReservationExpirationOutcome {
            reservation,
            promoted_reservations,
        })
    }

    pub fn cancel_booking(
        &mut self,
        booking_id: &BookingId,
        now: EventInstant,
    ) -> Result<BookingCancellationOutcome, EventModelError> {
        let booking = self.bookings.get(booking_id).cloned().ok_or_else(|| {
            EventModelError::MissingBooking {
                booking_id: booking_id.to_string(),
            }
        })?;

        if booking.status == BookingStatus::CheckedIn {
            return Err(EventModelError::InvalidBookingTransition {
                booking_id: booking.id.to_string(),
                from: booking.status,
                to: BookingStatus::Cancelled,
            });
        }

        if booking.status == BookingStatus::Cancelled {
            return Err(EventModelError::InvalidBookingTransition {
                booking_id: booking.id.to_string(),
                from: booking.status,
                to: BookingStatus::Cancelled,
            });
        }

        {
            let slot = self.slot_required_mut(&booking.event_id, &booking.slot_id)?;
            slot.cancel_booking();
        }

        let mut booking = booking;
        booking.status = BookingStatus::Cancelled;
        booking.cancelled_at = Some(now);
        self.bookings.insert(booking.id.clone(), booking.clone());

        let promoted_reservations =
            self.promote_waitlist(&booking.event_id, &booking.slot_id, now)?;
        Ok(BookingCancellationOutcome {
            booking,
            promoted_reservations,
        })
    }

    pub fn check_in_booking(
        &mut self,
        booking_id: &BookingId,
        checked_in_at: EventInstant,
        operator: &OperatorAccessContext,
    ) -> Result<(), EventModelError> {
        if !operator.allows(Capability::EventsBookingCheckIn) {
            return Err(EventModelError::Unauthorized {
                capability: Capability::EventsBookingCheckIn,
            });
        }

        let booking = self.bookings.get(booking_id).cloned().ok_or_else(|| {
            EventModelError::MissingBooking {
                booking_id: booking_id.to_string(),
            }
        })?;

        if booking.status != BookingStatus::Confirmed {
            return Err(EventModelError::InvalidBookingTransition {
                booking_id: booking.id.to_string(),
                from: booking.status,
                to: BookingStatus::CheckedIn,
            });
        }

        {
            let slot = self.slot_required_mut(&booking.event_id, &booking.slot_id)?;
            slot.check_in()?;
        }

        let mut booking = booking;
        booking.status = BookingStatus::CheckedIn;
        booking.checked_in_at = Some(checked_in_at);
        self.bookings.insert(booking.id.clone(), booking);
        Ok(())
    }

    pub fn promote_waitlist(
        &mut self,
        event_id: &EventId,
        slot_id: &EventSlotId,
        now: EventInstant,
    ) -> Result<Vec<Reservation>, EventModelError> {
        let mut promoted = Vec::new();

        loop {
            let (state, available_capacity, hold_duration) = {
                let slot = self.slot_required(event_id, slot_id)?;
                (slot.state, slot.available_capacity(), slot.hold_duration)
            };

            if !state.is_bookable() || available_capacity == 0 {
                break;
            }

            let next_waitlist_id = {
                let queue = match self.waitlists.get_mut(slot_id) {
                    Some(queue) => queue,
                    None => break,
                };
                queue.pop_front()
            };

            let Some(waitlist_id) = next_waitlist_id else {
                break;
            };

            let entry_snapshot = self
                .waitlist_entries
                .get(&waitlist_id)
                .cloned()
                .ok_or_else(|| EventModelError::MissingWaitlistEntry {
                    waitlist_entry_id: waitlist_id.to_string(),
                })?;

            if entry_snapshot.status != WaitlistStatus::Waiting {
                continue;
            }

            {
                let slot = self.slot_required_mut(event_id, slot_id)?;
                slot.reserve_hold()?;
            }

            let reservation_id = self.next_reservation_id(event_id, slot_id);
            let expires_at = now.checked_add("reservation_expires_at", hold_duration)?;
            let mut entry = entry_snapshot;
            entry.status = WaitlistStatus::Promoted;
            self.waitlist_entries
                .insert(waitlist_id.clone(), entry.clone());

            let reservation = Reservation::from_waitlist(
                reservation_id.clone(),
                waitlist_id,
                &entry,
                entry.source.clone(),
                now,
                expires_at,
            );
            self.reservations
                .insert(reservation_id, reservation.clone());
            promoted.push(reservation);
        }

        Ok(promoted)
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

    fn next_reservation_id(&mut self, event_id: &EventId, slot_id: &EventSlotId) -> ReservationId {
        self.next_reservation_seq += 1;
        ReservationId::new(format!(
            "res-{}-{}-{}",
            event_id.as_str(),
            slot_id.as_str(),
            self.next_reservation_seq
        ))
        .expect("generated reservation id is valid")
    }

    fn next_booking_id(&mut self, event_id: &EventId, slot_id: &EventSlotId) -> BookingId {
        self.next_booking_seq += 1;
        BookingId::new(format!(
            "book-{}-{}-{}",
            event_id.as_str(),
            slot_id.as_str(),
            self.next_booking_seq
        ))
        .expect("generated booking id is valid")
    }

    fn next_waitlist_id(&mut self, event_id: &EventId, slot_id: &EventSlotId) -> WaitlistEntryId {
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
