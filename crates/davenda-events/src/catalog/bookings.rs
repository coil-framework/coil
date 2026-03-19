use super::*;

impl EventCatalog {
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
                slot.increment_waitlisted_count();
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
                booked: slot.booked_count(),
                reserved: slot.reserved_count(),
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
}
