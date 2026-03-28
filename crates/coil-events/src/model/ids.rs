use super::*;

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, EventModelError> {
                Ok(Self(validate::validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(EventId, "event_id");
token_type!(EventHandle, "event_handle");
token_type!(EventSlotId, "event_slot_id");
token_type!(ReservationId, "reservation_id");
token_type!(BookingId, "booking_id");
token_type!(WaitlistEntryId, "waitlist_entry_id");
token_type!(AttendeeId, "attendee_id");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventInstant(u64);

impl EventInstant {
    pub const fn from_unix_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub fn checked_add(
        self,
        field: &'static str,
        duration: Duration,
    ) -> Result<Self, EventModelError> {
        let offset_seconds = duration.as_secs();
        let next =
            self.0
                .checked_add(offset_seconds)
                .ok_or(EventModelError::TimestampOverflow {
                    field,
                    base: self.0,
                    offset_seconds,
                })?;
        Ok(Self(next))
    }
}

impl fmt::Display for EventInstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
