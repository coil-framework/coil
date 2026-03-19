use super::*;

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

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, EventModelError> {
                Ok(Self(validate_token($field, value.into())?))
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


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventEligibilityRule {
    Public,
    Authenticated,
    RequiresCapability(Capability),
    RequiresAnyCapability(Vec<Capability>),
    RequiresMembershipTierAny(Vec<MembershipTierId>),
    RequiresEntitlementAny(Vec<String>),
    AllOf(Vec<EventEligibilityRule>),
    AnyOf(Vec<EventEligibilityRule>),
}

impl EventEligibilityRule {
    pub fn allows(&self, context: &EventAccessContext) -> Result<(), EligibilityFailure> {
        match self {
            Self::Public => Ok(()),
            Self::Authenticated => {
                if context.authenticated {
                    Ok(())
                } else {
                    Err(EligibilityFailure::AuthenticationRequired)
                }
            }
            Self::RequiresCapability(capability) => {
                if context.granted_capabilities.contains(capability) {
                    Ok(())
                } else {
                    Err(EligibilityFailure::MissingCapability {
                        capability: *capability,
                    })
                }
            }
            Self::RequiresAnyCapability(capabilities) => {
                if capabilities
                    .iter()
                    .any(|capability| context.granted_capabilities.contains(capability))
                {
                    Ok(())
                } else {
                    Err(EligibilityFailure::MissingCapability {
                        capability: *capabilities
                            .first()
                            .unwrap_or(&Capability::EventsBookingCreate),
                    })
                }
            }
            Self::RequiresMembershipTierAny(allowed) => {
                if allowed
                    .iter()
                    .any(|tier| context.active_membership_tiers.contains(tier))
                {
                    Ok(())
                } else {
                    Err(EligibilityFailure::MissingMembershipTier {
                        allowed: allowed.clone(),
                    })
                }
            }
            Self::RequiresEntitlementAny(allowed) => {
                if allowed
                    .iter()
                    .any(|entitlement| context.entitlements.contains(entitlement))
                {
                    Ok(())
                } else {
                    Err(EligibilityFailure::MissingEntitlement {
                        entitlement_key: allowed.first().cloned().unwrap_or_default(),
                    })
                }
            }
            Self::AllOf(rules) => {
                for rule in rules {
                    rule.allows(context)?;
                }
                Ok(())
            }
            Self::AnyOf(rules) => {
                let mut failures = Vec::new();
                for rule in rules {
                    match rule.allows(context) {
                        Ok(()) => return Ok(()),
                        Err(failure) => failures.push(failure),
                    }
                }

                Err(EligibilityFailure::Composite { failures })
            }
        }
    }
}

impl fmt::Display for EventEligibilityRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public => f.write_str("public"),
            Self::Authenticated => f.write_str("authenticated"),
            Self::RequiresCapability(capability) => write!(f, "requires capability `{capability}`"),
            Self::RequiresAnyCapability(capabilities) => write!(
                f,
                "requires any of [{}]",
                capabilities
                    .iter()
                    .map(Capability::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::RequiresMembershipTierAny(tiers) => write!(
                f,
                "requires any membership tier [{}]",
                tiers
                    .iter()
                    .map(MembershipTierId::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::RequiresEntitlementAny(entitlements) => {
                write!(f, "requires any entitlement [{}]", entitlements.join(", "))
            }
            Self::AllOf(rules) => write!(
                f,
                "all of [{}]",
                rules
                    .iter()
                    .map(EventEligibilityRule::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::AnyOf(rules) => write!(
                f,
                "any of [{}]",
                rules
                    .iter()
                    .map(EventEligibilityRule::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EligibilityFailure {
    AuthenticationRequired,
    MissingCapability { capability: Capability },
    MissingMembershipTier { allowed: Vec<MembershipTierId> },
    MissingEntitlement { entitlement_key: String },
    Composite { failures: Vec<EligibilityFailure> },
}

impl fmt::Display for EligibilityFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthenticationRequired => f.write_str("authentication is required"),
            Self::MissingCapability { capability } => {
                write!(f, "missing capability `{capability}`")
            }
            Self::MissingMembershipTier { allowed } => write!(
                f,
                "missing required membership tier among [{}]",
                allowed
                    .iter()
                    .map(MembershipTierId::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::MissingEntitlement { entitlement_key } => {
                write!(f, "missing entitlement `{entitlement_key}`")
            }
            Self::Composite { failures } => write!(
                f,
                "none of the alternatives matched: {}",
                failures
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventAccessContext {
    pub authenticated: bool,
    pub granted_capabilities: HashSet<Capability>,
    pub active_membership_tiers: BTreeSet<MembershipTierId>,
    pub entitlements: BTreeSet<String>,
}

impl EventAccessContext {
    pub fn public() -> Self {
        Self::default()
    }

    pub fn with_authenticated(mut self, authenticated: bool) -> Self {
        self.authenticated = authenticated;
        self
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.granted_capabilities.insert(capability);
        self
    }

    pub fn with_membership_tier(mut self, tier_id: MembershipTierId) -> Self {
        self.active_membership_tiers.insert(tier_id);
        self
    }

    pub fn with_entitlement(mut self, entitlement: impl Into<String>) -> Self {
        self.entitlements.insert(entitlement.into());
        self
    }
}

impl Default for EventAccessContext {
    fn default() -> Self {
        Self {
            authenticated: false,
            granted_capabilities: HashSet::new(),
            active_membership_tiers: BTreeSet::new(),
            entitlements: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorAccessContext {
    pub granted_capabilities: HashSet<Capability>,
}

impl OperatorAccessContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.granted_capabilities.insert(capability);
        self
    }

    pub fn allows(&self, capability: Capability) -> bool {
        self.granted_capabilities.contains(&capability)
    }
}

impl Default for OperatorAccessContext {
    fn default() -> Self {
        Self {
            granted_capabilities: HashSet::new(),
        }
    }
}

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
        let title = require_non_empty("slot_title", title.into())?;
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
            site_id: validate_token("site_id", site_id.into())?,
            handle,
            title: require_non_empty("event_title", title.into())?,
            summary: require_non_empty("event_summary", summary.into())?,
            body: require_non_empty("event_body", body.into())?,
            route: validate_route("event_route", route.into())?,
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
        self.seo_title = Some(require_non_empty("seo_title", seo_title.into())?);
        self.seo_description = Some(require_non_empty(
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

#[derive(Debug, Clone, PartialEq, Eq)]

fn validate_token(field: &'static str, value: String) -> Result<String, EventModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(EventModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

fn validate_route(field: &'static str, value: String) -> Result<String, EventModelError> {
    let route = require_non_empty(field, value)?;
    if route.starts_with('/') {
        Ok(route)
    } else {
        Err(EventModelError::InvalidRoute {
            field,
            value: route,
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, EventModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(EventModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}
