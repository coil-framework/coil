# Admin, Operations, and Integrations

## Purpose

Coil must support back-office workflows for editorial teams, operators, customer support, and business operations. For the target customer shape, operational tooling is part of the product itself and must be treated as a first-class module surface.

## Capability Summary

The platform must support:

- admin resource registration
- search, filters, tables, and detail screens
- bulk actions
- exports
- operational state transitions
- webhooks
- scheduled jobs
- notifications
- integration settings
- audit trails

## Admin Shell Requirements

The admin shell must provide:

- resource-oriented navigation
- detail and list screens
- reusable tables
- forms
- filters
- bulk actions
- modal or confirmation flows where appropriate
- capability-aware navigation and actions

The shell must be reusable across:

- CMS
- events
- bookings
- memberships
- reports
- integrations

## Operational Resource Types

The shell needs to handle at least these resource shapes:

- pages and shared blocks
- events and timeslots
- bookings and reservations
- customers and subscriptions
- event passes
- reports and exports
- integration logs
- background jobs and delivery state

Different resources will need different action sets, but the shell should provide common interaction primitives.

## Search, Filtering, and Export

The back office must support rich filtering for high-volume operational screens.

Examples:

- date range
- booking status
- event
- timeslot
- membership tier
- brand or region
- customer identity
- capacity state

Exports must be first-class, not ad hoc scripts. Large exports should be able to run asynchronously through the jobs system.

## Operational Actions

The admin shell must support actions such as:

- check in booking
- cancel booking
- resend confirmation
- manual booking
- reassign or migrate timeslot
- bulk customer changes
- bulk subscription changes
- content publish or rollback

Each action should:

- declare required capabilities
- write audit records
- report outcome clearly

## Jobs and Schedulers

The platform must support recurring and deferred work for:

- reminders
- follow-up emails
- attendance confirmation requests
- subscription lifecycle tasks
- cleanup and expiry tasks
- export generation

Editorial and operational settings should be able to enable or disable relevant scheduled behaviors where appropriate.

## Webhooks and External Event Ingestion

Coil needs a clear webhook model for:

- payment provider events
- commerce or POS sync events
- marketing or CRM events
- internal customer app events when exposed externally

The platform should provide:

- verification
- replay protection
- auditability
- idempotency
- routing to official modules or customer hooks

## Integration Settings

Integration configuration must be split cleanly between:

- secrets and environment-owned settings
- editable business configuration

Examples of editable business configuration:

- brand-specific messaging IDs
- region-to-account mappings
- scheduled reminder switches
- event-pass display settings

These do not belong only in code if operators need to change them.

## Notifications and Audit

The platform must keep an audit trail for:

- publish operations
- booking and subscription state changes
- admin overrides
- integration failures
- check-in operations
- bulk operations

Notification surfaces should include:

- email
- admin notifications
- optional real-time or push channels where the customer needs them

## Reporting

The admin surface should support reporting modules for:

- attendance
- bookings by event and timeslot
- subscription state
- event-pass issuance and redemption
- audience and region breakdowns

Some reports can be official modules. Others will remain customer-owned, but the shell and export framework should make them straightforward to add.

## Required Official Module Boundaries

The recommended split is:

- `coil-admin`
  owns the shell, shared tables/forms, audit framing, and operator UX primitives
- `coil-ops`
  owns reports, exports, back-office operational helpers, and maintenance workflows
- `coil-jobs`
  owns scheduled and deferred execution
- `coil-observability`
  owns logs, metrics, traces, and operational diagnostics
- customer linked Rust
  owns customer-specific operational policies and custom integrations

## Immediate Implications for Coil

Coil should avoid treating admin as only a content-management shell. The target customer shape requires a platform that can support content editors and operational teams in the same product without collapsing them into the same data model or permission boundary.
