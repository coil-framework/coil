use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventTransitionKind {
    Scheduled,
    Published,
    Cancelled,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotTransitionKind {
    Opened,
    Closed,
    Cancelled,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookingSourceKind {
    Manual,
    CommerceOrder,
    MembershipEntitlement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookingSource {
    Manual,
    CommerceOrder { order_id: OrderId },
    MembershipEntitlement { tier_id: MembershipTierId },
}

impl BookingSource {
    pub const fn kind(&self) -> BookingSourceKind {
        match self {
            Self::Manual => BookingSourceKind::Manual,
            Self::CommerceOrder { .. } => BookingSourceKind::CommerceOrder,
            Self::MembershipEntitlement { .. } => BookingSourceKind::MembershipEntitlement,
        }
    }
}


pub struct Reservation {
    pub id: ReservationId,
    pub event_id: EventId,
    pub slot_id: EventSlotId,
    pub attendee_id: Option<AttendeeId>,
    pub source: BookingSource,
    pub status: ReservationStatus,
    pub created_at: EventInstant,
    pub expires_at: EventInstant,
    pub booking_id: Option<BookingId>,
    pub waitlist_entry_id: Option<WaitlistEntryId>,
}

impl Reservation {
    fn held(
        id: ReservationId,
        event_id: EventId,
        slot_id: EventSlotId,
        attendee_id: Option<AttendeeId>,
        source: BookingSource,
        created_at: EventInstant,
        expires_at: EventInstant,
    ) -> Self {
        Self {
            id,
            event_id,
            slot_id,
            attendee_id,
            source,
            status: ReservationStatus::Held,
            created_at,
            expires_at,
            booking_id: None,
            waitlist_entry_id: None,
        }
    }

    fn from_waitlist(
        id: ReservationId,
        waitlist_entry_id: WaitlistEntryId,
        entry: &WaitlistEntry,
        source: BookingSource,
        created_at: EventInstant,
        expires_at: EventInstant,
    ) -> Self {
        Self {
            id,
            event_id: entry.event_id.clone(),
            slot_id: entry.slot_id.clone(),
            attendee_id: entry.attendee_id.clone(),
            source,
            status: ReservationStatus::Held,
            created_at,
            expires_at,
            booking_id: None,
            waitlist_entry_id: Some(waitlist_entry_id),
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            ReservationStatus::Held | ReservationStatus::Confirmed
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Booking {
    pub id: BookingId,
    pub reservation_id: ReservationId,
    pub event_id: EventId,
    pub slot_id: EventSlotId,
    pub attendee_id: Option<AttendeeId>,
    pub source: BookingSource,
    pub status: BookingStatus,
    pub booked_at: EventInstant,
    pub checked_in_at: Option<EventInstant>,
    pub cancelled_at: Option<EventInstant>,
}

impl Booking {
    fn from_reservation(id: BookingId, reservation: &Reservation, booked_at: EventInstant) -> Self {
        Self {
            id,
            reservation_id: reservation.id.clone(),
            event_id: reservation.event_id.clone(),
            slot_id: reservation.slot_id.clone(),
            attendee_id: reservation.attendee_id.clone(),
            source: reservation.source.clone(),
            status: BookingStatus::Confirmed,
            booked_at,
            checked_in_at: None,
            cancelled_at: None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            BookingStatus::Confirmed | BookingStatus::CheckedIn
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitlistEntry {
    pub id: WaitlistEntryId,
    pub event_id: EventId,
    pub slot_id: EventSlotId,
    pub attendee_id: Option<AttendeeId>,
    pub source: BookingSource,
    pub status: WaitlistStatus,
    pub created_at: EventInstant,
    pub position: u32,
}

impl WaitlistEntry {
    fn new(
        id: WaitlistEntryId,
        event_id: EventId,
        slot_id: EventSlotId,
        attendee_id: Option<AttendeeId>,
        source: BookingSource,
        created_at: EventInstant,
        position: u32,
    ) -> Self {
        Self {
            id,
            event_id,
            slot_id,
            attendee_id,
            source,
            status: WaitlistStatus::Waiting,
            created_at,
            position,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReservationOutcome {
    Held(Reservation),
    Waitlisted(WaitlistEntry),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookingCancellationOutcome {
    pub booking: Booking,
    pub promoted_reservations: Vec<Reservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservationExpirationOutcome {
    pub reservation: Reservation,
    pub promoted_reservations: Vec<Reservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
