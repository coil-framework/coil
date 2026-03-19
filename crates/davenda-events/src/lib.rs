use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::error::Error;
use std::fmt;
use std::time::Duration;

use davenda_auth::{
    Capability, DefaultSubject, DefaultTuple, DefaultTupleUpdate, Entity, Relation,
};
use davenda_commerce::OrderId;
use davenda_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution,
    CapabilityContract, CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor,
    ExtensionSlotKind, IntegrationKind, IntegrationPoint, JobContract, JobTriggerKind,
    MigrationContract, ModuleBehavior, ModuleDependency, ModuleManifest, PlatformModule,
    RegistrationError, RouteSurface, RouteSurfaceKind, ServiceRegistry,
};
use davenda_data::{
    DataModelError, DomainWrite, FilterOperator, MigrationId, MigrationOwner, MigrationPlan,
    MigrationStep, PageRequest, PublicationVisibility, QueryCacheScope, QueryContext,
    QueryFilter, QuerySort, QuerySpec, TransactionIsolation, TransactionPlan,
};
use davenda_memberships::MembershipTierId;

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
        TransactionPlan::new("events.reservation.hold", TransactionIsolation::Serializable)?
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

    pub fn check_in_transaction_plan(&self, booking: &Booking) -> Result<TransactionPlan, EventModelError> {
        TransactionPlan::new("events.booking.check_in", TransactionIsolation::Serializable)?
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventsModule {
    name: String,
    config_namespace: String,
    admin_resources: Vec<AdminResourceContribution>,
}

impl EventsModule {
    pub fn new() -> Self {
        Self {
            name: "events".to_string(),
            config_namespace: "events".to_string(),
            admin_resources: vec![
                AdminResourceContribution::new(
                    "events.events",
                    "/admin/events/events",
                    "Events",
                    "Events",
                    AdminNavigationSection::Events,
                    AdminContributionKind::ResourceIndex,
                    Capability::EventsEventPublish,
                ),
                AdminResourceContribution::new(
                    "events.slots",
                    "/admin/events/slots",
                    "Slots",
                    "Slots",
                    AdminNavigationSection::Events,
                    AdminContributionKind::ResourceIndex,
                    Capability::EventsSlotManage,
                ),
                AdminResourceContribution::new(
                    "events.bookings",
                    "/admin/events/bookings",
                    "Bookings",
                    "Bookings",
                    AdminNavigationSection::Events,
                    AdminContributionKind::ResourceIndex,
                    Capability::EventsBookingCreate,
                ),
                AdminResourceContribution::new(
                    "events.check-in",
                    "/admin/events/check-in",
                    "Check-in",
                    "Check-in",
                    AdminNavigationSection::Events,
                    AdminContributionKind::Workflow,
                    Capability::EventsBookingCheckIn,
                ),
            ],
        }
    }

    pub fn admin_resources(&self) -> &[AdminResourceContribution] {
        &self.admin_resources
    }
}

impl Default for EventsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformModule for EventsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(self.name.clone())
            .with_required_capabilities(vec![
                Capability::EventsEventPublish,
                Capability::EventsSlotManage,
                Capability::EventsBookingCreate,
                Capability::EventsBookingCheckIn,
            ])
            .with_optional_capabilities(vec![
                Capability::AdminShellAccess,
                Capability::CmsPageRead,
                Capability::SeoMetadataEdit,
                Capability::I18nTranslationEdit,
                Capability::MembershipSubscriptionManage,
                Capability::AssetRead,
                Capability::CheckoutSessionCreate,
                Capability::OrderRead,
            ])
            .with_config_namespace(self.config_namespace.clone())
            .with_capability_contracts(vec![
                CapabilityContract::required(
                    Capability::EventsEventPublish,
                    ["event"],
                ),
                CapabilityContract::required(
                    Capability::EventsSlotManage,
                    ["event_slot"],
                ),
                CapabilityContract::required(
                    Capability::EventsBookingCreate,
                    ["booking", "event_slot"],
                ),
                CapabilityContract::required(
                    Capability::EventsBookingCheckIn,
                    ["booking"],
                ),
                CapabilityContract::optional(
                    Capability::AdminShellAccess,
                    ["admin_module"],
                ),
                CapabilityContract::optional(Capability::CmsPageRead, ["page"]),
                CapabilityContract::optional(
                    Capability::SeoMetadataEdit,
                    ["event"],
                ),
                CapabilityContract::optional(
                    Capability::I18nTranslationEdit,
                    ["event"],
                ),
                CapabilityContract::optional(
                    Capability::MembershipSubscriptionManage,
                    ["subscription", "membership_tier"],
                ),
                CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
                CapabilityContract::optional(
                    Capability::CheckoutSessionCreate,
                    ["storefront"],
                ),
                CapabilityContract::optional(Capability::OrderRead, ["order"]),
            ])
            .with_module_dependencies(vec![
                ModuleDependency::optional(
                    "admin",
                    "Events contributes booking, slot, and check-in resources to the shared admin shell when installed",
                ),
                ModuleDependency::optional(
                    "cms",
                    "Event pages and discoverability can compose into CMS-driven storefront content",
                ),
                ModuleDependency::optional(
                    "commerce",
                    "Paid bookings can bridge into checkout and order workflows when commerce is installed",
                ),
                ModuleDependency::optional(
                    "memberships",
                    "Membership tiers can gate event eligibility and booking access rules",
                ),
            ])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Auth,
                CoreServiceDependency::Data,
                CoreServiceDependency::Cache,
                CoreServiceDependency::Jobs,
                CoreServiceDependency::I18n,
                CoreServiceDependency::Seo,
                CoreServiceDependency::Template,
                CoreServiceDependency::Observability,
            ])
            .with_migrations(vec![
                MigrationContract::new(
                    "events.catalog",
                    10,
                    "Creates event content, discoverability, and publication state tables",
                ),
                MigrationContract::new(
                    "events.slots",
                    20,
                    "Creates event-slot capacity, timing, and reservation state tables",
                ),
                MigrationContract::new(
                    "events.bookings",
                    30,
                    "Creates booking, waitlist, and check-in lifecycle tables",
                ),
            ])
            .with_route_surfaces(vec![
                RouteSurface::new("events.list", RouteSurfaceKind::FrontendPage, "/events")
                    .localized(),
                RouteSurface::new(
                    "events.detail",
                    RouteSurfaceKind::FrontendPage,
                    "/events/{event_slug}",
                )
                .localized(),
                RouteSurface::new(
                    "events.book",
                    RouteSurfaceKind::FrontendAction,
                    "/events/{event_slug}/book",
                )
                .gated_by(Capability::EventsBookingCreate),
                RouteSurface::new(
                    "events.admin.index",
                    RouteSurfaceKind::AdminPage,
                    "/admin/events",
                )
                .gated_by(Capability::EventsEventPublish),
                RouteSurface::new(
                    "events.admin.bookings",
                    RouteSurfaceKind::AdminPage,
                    "/admin/events/bookings",
                )
                .gated_by(Capability::EventsBookingCreate),
                RouteSurface::new(
                    "events.admin.check-in",
                    RouteSurfaceKind::AdminPage,
                    "/admin/events/check-in",
                )
                .gated_by(Capability::EventsBookingCheckIn),
            ])
            .with_jobs(vec![
                JobContract::new(
                    "events.reservation-expiry",
                    JobTriggerKind::Scheduled,
                    true,
                    "Releases expired reservation holds and promotes waitlisted attendees when capacity returns",
                ),
                JobContract::new(
                    "events.waitlist-promotion",
                    JobTriggerKind::DomainEvent,
                    true,
                    "Promotes waitlist entries after cancellations or released holds",
                ),
                JobContract::new(
                    "events.reminders",
                    JobTriggerKind::Scheduled,
                    true,
                    "Schedules reminder and attendance preparation notifications for upcoming bookings",
                ),
            ])
            .with_event_subscriptions(vec![
                EventSubscription::new(
                    "commerce.order.paid",
                    Some("events.waitlist-promotion"),
                    "Allows paid-booking confirmation flows to reconcile held reservations into confirmed bookings",
                ),
                EventSubscription::new(
                    "membership.subscription.activated",
                    Some("events.reminders"),
                    "Refreshes member-only eligibility and upcoming-event communication windows after subscription changes",
                ),
            ])
            .with_integration_points(vec![
                IntegrationPoint::new(
                    IntegrationKind::FrontendRendering,
                    "events.pages",
                    "Provides public event discovery, detail pages, and booking entry points",
                ),
                IntegrationPoint::new(
                    IntegrationKind::AdminWorkflow,
                    "events.check-in",
                    "Adds check-in, booking review, and slot operations to the shared admin shell",
                ),
                IntegrationPoint::new(
                    IntegrationKind::SeoMetadata,
                    "events.head",
                    "Emits event metadata and rich-result schema for discoverable event pages",
                ),
                IntegrationPoint::new(
                    IntegrationKind::JsonLd,
                    "events.schema",
                    "Supplies JSON-LD for event pages and schedule-rich discovery surfaces",
                ),
                IntegrationPoint::new(
                    IntegrationKind::SearchIndex,
                    "events.index",
                    "Publishes searchable public event and operator booking visibility data",
                ),
                IntegrationPoint::new(
                    IntegrationKind::CommerceBridge,
                    "events.paid-bookings",
                    "Bridges optional paid-booking flows into checkout and order outcomes",
                ),
            ])
            .with_behaviors(vec![
                ModuleBehavior::CacheInvalidation,
                ModuleBehavior::LocalizedContent,
                ModuleBehavior::SeoMetadata,
                ModuleBehavior::JsonLd,
                ModuleBehavior::AccessibleAdminUi,
                ModuleBehavior::AsyncJobs,
            ])
            .with_extension_slots(vec![
                ExtensionSlotDescriptor::new(
                    ExtensionSlotKind::AdminWidget,
                    "events.booking.summary",
                    "Allows bounded widgets to enrich booking and attendance operations",
                ),
                ExtensionSlotDescriptor::new(
                    ExtensionSlotKind::RenderHook,
                    "events.page.render",
                    "Allows controlled customer embellishments around event page rendering",
                ),
            ])
            .with_admin_resources(self.admin_resources.clone())
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            self.name.clone(),
            "module.events.content",
            "Event content, discoverability, SEO metadata, and public page composition",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.events.slots",
            "Timeslots, capacity rules, and session scheduling",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.events.reservations",
            "Reservation holds, expiry handling, and waitlist promotion",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.events.bookings",
            "Confirmed bookings, cancellations, and booking lifecycle state",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.events.waitlists",
            "Waitlist queue management and promotion workflows",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.events.check_in",
            "Operator check-in workflows for attended events",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.events.admin",
            "Event admin resources, slot operations, and booking review",
        )
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        let owner = MigrationOwner::Module(self.name.clone());
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("events_catalog").expect("constant migration id is valid"),
                owner.clone(),
                10,
                "Create event catalog and publication storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS events_catalog (id TEXT PRIMARY KEY, slug TEXT NOT NULL, status TEXT NOT NULL, published_at BIGINT)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("event migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("event_slots").expect("constant migration id is valid"),
                owner.clone(),
                20,
                "Create event slot and capacity storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS event_slots (id TEXT PRIMARY KEY, event_id TEXT NOT NULL, starts_at BIGINT NOT NULL, capacity BIGINT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("event migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("event_bookings").expect("constant migration id is valid"),
                owner,
                30,
                "Create booking, reservation, waitlist, and check-in storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS event_bookings (id TEXT PRIMARY KEY, slot_id TEXT NOT NULL, status TEXT NOT NULL, checked_in_at BIGINT)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("event migration ids are unique");
        Some(plan)
    }
}

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

#[cfg(test)]
mod tests {
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
        assert_eq!(manifest.jobs.len(), 3);
        assert_eq!(manifest.event_subscriptions.len(), 2);
        assert_eq!(manifest.admin_resources.len(), 4);
        assert!(manifest
            .module_dependencies
            .iter()
            .any(|dependency| dependency.module == "commerce"));
        assert!(manifest
            .core_service_dependencies
            .contains(&CoreServiceDependency::Jobs));
        assert!(manifest
            .extension_slots
            .iter()
            .any(|slot| slot.kind == ExtensionSlotKind::AdminWidget));
        assert_eq!(
            module
                .install_migration_plan()
                .expect("events migration plan")
                .ordered_steps()
                .len(),
            3
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
}
