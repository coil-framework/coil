use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventState {
    Draft,
    Scheduled,
    Published,
    Cancelled,
    Archived,
}

impl EventState {
    pub const fn is_live(self) -> bool {
        matches!(self, Self::Published)
    }
}

impl fmt::Display for EventState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => f.write_str("draft"),
            Self::Scheduled => f.write_str("scheduled"),
            Self::Published => f.write_str("published"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Archived => f.write_str("archived"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventVisibility {
    Public,
    MembersOnly,
    InviteOnly,
}

impl fmt::Display for EventVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public => f.write_str("public"),
            Self::MembersOnly => f.write_str("members_only"),
            Self::InviteOnly => f.write_str("invite_only"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    Draft,
    Open,
    Closed,
    Cancelled,
    Completed,
}

impl SlotState {
    pub const fn is_bookable(self) -> bool {
        matches!(self, Self::Open)
    }
}

impl fmt::Display for SlotState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => f.write_str("draft"),
            Self::Open => f.write_str("open"),
            Self::Closed => f.write_str("closed"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Completed => f.write_str("completed"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservationStatus {
    Held,
    Confirmed,
    Cancelled,
    Expired,
}

impl fmt::Display for ReservationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Held => f.write_str("held"),
            Self::Confirmed => f.write_str("confirmed"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Expired => f.write_str("expired"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookingStatus {
    Confirmed,
    Cancelled,
    CheckedIn,
    NoShow,
}

impl fmt::Display for BookingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Confirmed => f.write_str("confirmed"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::CheckedIn => f.write_str("checked_in"),
            Self::NoShow => f.write_str("no_show"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitlistStatus {
    Waiting,
    Promoted,
    Cancelled,
    Expired,
}

impl fmt::Display for WaitlistStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Waiting => f.write_str("waiting"),
            Self::Promoted => f.write_str("promoted"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Expired => f.write_str("expired"),
        }
    }
}
