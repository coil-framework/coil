use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSlot {
    pub id: EventSlotId,
    pub title: String,
    pub starts_at: EventInstant,
    pub ends_at: EventInstant,
    pub capacity: u32,
    pub waitlist_capacity: u32,
    pub hold_duration: Duration,
    pub requires_check_in: bool,
    pub state: SlotState,
    reserved_count: u32,
    booked_count: u32,
    checked_in_count: u32,
    waitlisted_count: u32,
}

impl EventSlot {
    pub fn new(
        id: EventSlotId,
        title: impl Into<String>,
        starts_at: EventInstant,
        ends_at: EventInstant,
        capacity: u32,
        waitlist_capacity: u32,
        hold_duration: Duration,
        requires_check_in: bool,
    ) -> Result<Self, EventModelError> {
        let title = validate::require_non_empty("slot_title", title.into())?;
        if capacity == 0 {
            return Err(EventModelError::EmptyField {
                field: "slot_capacity",
            });
        }
        if ends_at <= starts_at {
            return Err(EventModelError::InvalidToken {
                field: "slot_ends_at",
                value: ends_at.to_string(),
            });
        }
        if hold_duration.is_zero() {
            return Err(EventModelError::EmptyField {
                field: "slot_hold_duration",
            });
        }

        Ok(Self {
            id,
            title,
            starts_at,
            ends_at,
            capacity,
            waitlist_capacity,
            hold_duration,
            requires_check_in,
            state: SlotState::Open,
            reserved_count: 0,
            booked_count: 0,
            checked_in_count: 0,
            waitlisted_count: 0,
        })
    }

    pub fn reserved_count(&self) -> u32 {
        self.reserved_count
    }

    pub fn booked_count(&self) -> u32 {
        self.booked_count
    }

    pub fn checked_in_count(&self) -> u32 {
        self.checked_in_count
    }

    pub fn waitlisted_count(&self) -> u32 {
        self.waitlisted_count
    }

    pub(crate) fn increment_waitlisted_count(&mut self) {
        self.waitlisted_count = self
            .waitlisted_count
            .checked_add(1)
            .expect("waitlist count overflow is not practical");
    }

    pub fn available_capacity(&self) -> u32 {
        self.capacity
            .saturating_sub(self.reserved_count + self.booked_count)
    }

    pub fn has_waitlist_space(&self) -> bool {
        self.waitlisted_count < self.waitlist_capacity
    }

    pub fn can_accept_hold(&self) -> bool {
        self.state.is_bookable() && self.available_capacity() > 0
    }

    pub fn can_waitlist(&self) -> bool {
        matches!(self.state, SlotState::Open) && self.has_waitlist_space()
    }

    pub fn reserve_hold(&mut self) -> Result<(), EventModelError> {
        if !self.can_accept_hold() {
            return Err(EventModelError::CapacityExceeded {
                event_id: String::new(),
                slot_id: self.id.to_string(),
                capacity: self.capacity,
                booked: self.booked_count,
                reserved: self.reserved_count,
            });
        }

        self.reserved_count = self
            .reserved_count
            .checked_add(1)
            .expect("slot hold count overflow is not practical");
        Ok(())
    }

    pub fn release_hold(&mut self) {
        if self.reserved_count > 0 {
            self.reserved_count -= 1;
        }
    }

    pub fn confirm_hold(&mut self) -> Result<(), EventModelError> {
        if self.reserved_count == 0 {
            return Err(EventModelError::InvalidReservationTransition {
                reservation_id: String::new(),
                from: ReservationStatus::Cancelled,
                to: ReservationStatus::Confirmed,
            });
        }

        self.reserved_count -= 1;
        self.booked_count = self
            .booked_count
            .checked_add(1)
            .expect("slot booking count overflow is not practical");
        Ok(())
    }

    pub fn cancel_booking(&mut self) {
        if self.booked_count > 0 {
            self.booked_count -= 1;
        }
    }

    pub fn check_in(&mut self) -> Result<(), EventModelError> {
        if self.booked_count == 0 {
            return Err(EventModelError::InvalidBookingTransition {
                booking_id: String::new(),
                from: BookingStatus::Cancelled,
                to: BookingStatus::CheckedIn,
            });
        }

        self.checked_in_count = self
            .checked_in_count
            .checked_add(1)
            .expect("slot check-in count overflow is not practical");
        Ok(())
    }

    pub fn open(&mut self) {
        self.state = SlotState::Open;
    }

    pub fn close(&mut self) {
        self.state = SlotState::Closed;
    }

    pub fn cancel(&mut self) {
        self.state = SlotState::Cancelled;
    }

    pub fn complete(&mut self) {
        self.state = SlotState::Completed;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub id: EventId,
    pub site_id: String,
    pub handle: EventHandle,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub route: String,
    pub visibility: EventVisibility,
    pub state: EventState,
    pub eligibility: EventEligibilityRule,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    slots: BTreeMap<EventSlotId, EventSlot>,
}

impl EventRecord {
    pub fn new(
        id: EventId,
        site_id: impl Into<String>,
        handle: EventHandle,
        title: impl Into<String>,
        summary: impl Into<String>,
        body: impl Into<String>,
        route: impl Into<String>,
        visibility: EventVisibility,
        eligibility: EventEligibilityRule,
    ) -> Result<Self, EventModelError> {
        Ok(Self {
            id,
            site_id: validate::validate_token("site_id", site_id.into())?,
            handle,
            title: validate::require_non_empty("event_title", title.into())?,
            summary: validate::require_non_empty("event_summary", summary.into())?,
            body: validate::require_non_empty("event_body", body.into())?,
            route: validate::validate_route("event_route", route.into())?,
            visibility,
            state: EventState::Draft,
            eligibility,
            seo_title: None,
            seo_description: None,
            slots: BTreeMap::new(),
        })
    }

    pub fn with_seo(
        mut self,
        seo_title: impl Into<String>,
        seo_description: impl Into<String>,
    ) -> Result<Self, EventModelError> {
        self.seo_title = Some(validate::require_non_empty("seo_title", seo_title.into())?);
        self.seo_description = Some(validate::require_non_empty(
            "seo_description",
            seo_description.into(),
        )?);
        Ok(self)
    }

    pub fn slots(&self) -> impl Iterator<Item = &EventSlot> {
        self.slots.values()
    }

    pub fn slot(&self, slot_id: &EventSlotId) -> Option<&EventSlot> {
        self.slots.get(slot_id)
    }

    pub fn slot_mut(&mut self, slot_id: &EventSlotId) -> Option<&mut EventSlot> {
        self.slots.get_mut(slot_id)
    }

    pub fn add_slot(&mut self, slot: EventSlot) -> Result<(), EventModelError> {
        if self.slots.contains_key(&slot.id) {
            return Err(EventModelError::DuplicateSlot {
                event_id: self.id.to_string(),
                slot_id: slot.id.to_string(),
            });
        }

        self.slots.insert(slot.id.clone(), slot);
        Ok(())
    }

    pub fn schedule(&mut self) {
        self.state = EventState::Scheduled;
    }

    pub fn publish(&mut self) {
        self.state = EventState::Published;
    }

    pub fn cancel(&mut self) {
        self.state = EventState::Cancelled;
    }

    pub fn archive(&mut self) {
        self.state = EventState::Archived;
    }

    pub fn can_view(&self, context: &EventAccessContext) -> Result<(), EventModelError> {
        if !self.state.is_live() {
            return Err(EventModelError::EventNotLive {
                event_id: self.id.to_string(),
                status: self.state,
            });
        }

        match self.visibility {
            EventVisibility::Public => {}
            EventVisibility::MembersOnly | EventVisibility::InviteOnly
                if !context.authenticated =>
            {
                return Err(EventModelError::EligibilityDenied {
                    event_id: self.id.to_string(),
                    slot_id: None,
                    reason: EligibilityFailure::AuthenticationRequired,
                });
            }
            _ => {}
        }

        self.eligibility
            .allows(context)
            .map_err(|reason| EventModelError::EligibilityDenied {
                event_id: self.id.to_string(),
                slot_id: None,
                reason,
            })
    }

    pub fn can_book(
        &self,
        slot_id: &EventSlotId,
        context: &EventAccessContext,
    ) -> Result<(), EventModelError> {
        self.can_view(context)?;
        let slot = self
            .slot(slot_id)
            .ok_or_else(|| EventModelError::MissingSlot {
                event_id: self.id.to_string(),
                slot_id: slot_id.to_string(),
            })?;

        if !slot.state.is_bookable() {
            return Err(EventModelError::SlotNotOpen {
                event_id: self.id.to_string(),
                slot_id: slot_id.to_string(),
                status: slot.state,
            });
        }

        Ok(())
    }

    pub fn auth_projection(&self) -> Vec<DefaultTupleUpdate> {
        let mut updates = vec![DefaultTupleUpdate::Write(DefaultTuple::new(
            Entity::event(self.id.to_string()),
            Relation::Site,
            DefaultSubject::entity(Entity::site(self.site_id.clone())),
        ))];

        for slot in self.slots.values() {
            updates.push(DefaultTupleUpdate::Write(DefaultTuple::new(
                Entity::event_slot(slot.id.to_string()),
                Relation::Event,
                DefaultSubject::entity(Entity::event(self.id.to_string())),
            )));
        }

        updates
    }
}
