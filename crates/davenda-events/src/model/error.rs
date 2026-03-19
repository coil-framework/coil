use super::*;
use std::error::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    InvalidRoute {
        field: &'static str,
        value: String,
    },
    DuplicateEvent {
        event_id: String,
    },
    DuplicateSlot {
        event_id: String,
        slot_id: String,
    },
    MissingEvent {
        event_id: String,
    },
    MissingSlot {
        event_id: String,
        slot_id: String,
    },
    MissingReservation {
        reservation_id: String,
    },
    MissingBooking {
        booking_id: String,
    },
    MissingWaitlistEntry {
        waitlist_entry_id: String,
    },
    EventNotLive {
        event_id: String,
        status: EventState,
    },
    SlotNotOpen {
        event_id: String,
        slot_id: String,
        status: SlotState,
    },
    CapacityExceeded {
        event_id: String,
        slot_id: String,
        capacity: u32,
        booked: u32,
        reserved: u32,
    },
    WaitlistFull {
        event_id: String,
        slot_id: String,
        capacity: u32,
    },
    ReservationExpired {
        reservation_id: String,
        expires_at: EventInstant,
        now: EventInstant,
    },
    InvalidReservationTransition {
        reservation_id: String,
        from: ReservationStatus,
        to: ReservationStatus,
    },
    InvalidBookingTransition {
        booking_id: String,
        from: BookingStatus,
        to: BookingStatus,
    },
    Unauthorized {
        capability: Capability,
    },
    EligibilityDenied {
        event_id: String,
        slot_id: Option<String>,
        reason: EligibilityFailure,
    },
    TimestampOverflow {
        field: &'static str,
        base: u64,
        offset_seconds: u64,
    },
    DataPlan {
        error: DataModelError,
    },
}

impl fmt::Display for EventModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidRoute { field, value } => {
                write!(f, "`{field}` must start with `/`, got `{value}`")
            }
            Self::DuplicateEvent { event_id } => write!(f, "event `{event_id}` is duplicated"),
            Self::DuplicateSlot { event_id, slot_id } => {
                write!(f, "slot `{slot_id}` is duplicated on event `{event_id}`")
            }
            Self::MissingEvent { event_id } => write!(f, "event `{event_id}` was not found"),
            Self::MissingSlot { event_id, slot_id } => {
                write!(f, "slot `{slot_id}` was not found on event `{event_id}`")
            }
            Self::MissingReservation { reservation_id } => {
                write!(f, "reservation `{reservation_id}` was not found")
            }
            Self::MissingBooking { booking_id } => {
                write!(f, "booking `{booking_id}` was not found")
            }
            Self::MissingWaitlistEntry { waitlist_entry_id } => {
                write!(f, "waitlist entry `{waitlist_entry_id}` was not found")
            }
            Self::EventNotLive { event_id, status } => {
                write!(f, "event `{event_id}` is not live while `{status}`")
            }
            Self::SlotNotOpen {
                event_id,
                slot_id,
                status,
            } => write!(
                f,
                "slot `{slot_id}` on event `{event_id}` is not open while `{status}`"
            ),
            Self::CapacityExceeded {
                event_id,
                slot_id,
                capacity,
                booked,
                reserved,
            } => write!(
                f,
                "slot `{slot_id}` on event `{event_id}` is full: capacity={capacity} booked={booked} reserved={reserved}"
            ),
            Self::WaitlistFull {
                event_id,
                slot_id,
                capacity,
            } => write!(
                f,
                "waitlist for slot `{slot_id}` on event `{event_id}` is full at `{capacity}`"
            ),
            Self::ReservationExpired {
                reservation_id,
                expires_at,
                now,
            } => write!(
                f,
                "reservation `{reservation_id}` expired at `{expires_at}`, current time is `{now}`"
            ),
            Self::InvalidReservationTransition {
                reservation_id,
                from,
                to,
            } => write!(
                f,
                "cannot transition reservation `{reservation_id}` from `{from}` to `{to}`"
            ),
            Self::InvalidBookingTransition {
                booking_id,
                from,
                to,
            } => write!(
                f,
                "cannot transition booking `{booking_id}` from `{from}` to `{to}`"
            ),
            Self::Unauthorized { capability } => {
                write!(f, "operator is missing capability `{capability}`")
            }
            Self::EligibilityDenied {
                event_id,
                slot_id,
                reason,
            } => match slot_id {
                Some(slot_id) => write!(
                    f,
                    "eligibility check failed for event `{event_id}` slot `{slot_id}`: {reason}"
                ),
                None => write!(
                    f,
                    "eligibility check failed for event `{event_id}`: {reason}"
                ),
            },
            Self::TimestampOverflow {
                field,
                base,
                offset_seconds,
            } => write!(
                f,
                "timestamp overflow while calculating `{field}` from `{base}` plus `{offset_seconds}` seconds"
            ),
            Self::DataPlan { error } => write!(f, "{error}"),
        }
    }
}

impl Error for EventModelError {}

impl From<DataModelError> for EventModelError {
    fn from(error: DataModelError) -> Self {
        Self::DataPlan { error }
    }
}
