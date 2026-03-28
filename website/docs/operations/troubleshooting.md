---
title: Troubleshooting
---

Troubleshooting in Davenda should start with explicit evidence, not guesswork.

Use this page as a framework for incident response and operator debugging.

## First Questions

Before changing anything, determine:

- is the problem request-path, job-path, provider-path, or deployment-path
- did the problem begin after a config or release change
- is it isolated to one site, locale, or host
- is it affecting only one operator workflow or customer journey

That classification narrows the search quickly.

## Startup Failures

If the runtime will not start, check:

- manifest and config alignment
- auth package loading
- required secrets
- database, cache, and storage configuration
- extension package resolution
- linked customer plugin registration

Most startup failures should fail closed and surface clearly before the service accepts traffic.

## Request Path Problems

If public or admin requests fail, inspect:

- request logs and traces
- route and host resolution
- locale resolution
- session and CSRF behavior
- auth and capability checks
- template or render model mismatches

For multi-site apps, confirm the issue is not site-specific before assuming a global regression.

## Jobs Problems

If background work fails or stalls, inspect:

- queue depth
- in-flight jobs
- retry churn
- dead-letter entries
- scheduler promotion behavior
- dependency failures shared by those jobs

Do not treat a dead-letter queue as an archive. It is an operational signal that needs ownership.

## Provider And Integration Failures

If payments, storage, TLS, or outbound integrations fail, verify:

- the configured provider is actually enabled
- required secrets are present and current
- provider callbacks or webhook signatures are valid
- the failure is not a rollout mismatch between old and new config

When an external dependency is involved, capture provider-specific errors in the incident record.

## Release And Migration Problems

If a deployment looks wrong, verify:

- the expected binary and config were promoted
- migrations applied as planned
- assets were published for the current release
- cache state matches the release
- cutover actually switched the intended target

Never debug a release issue without first confirming what was actually deployed.

## Safe Response Rules

During troubleshooting:

- prefer inspection before mutation
- prefer reversible actions before destructive ones
- record what changed and why
- avoid manual fixes that bypass the documented operator path unless the incident requires it

Emergency changes without records often become the next incident.

## Build Your Own Runbooks

Each customer product should extend this with product-specific runbooks for:

- checkout failures
- webhook failures
- asset publication failures
- admin or editorial lockouts
- jobs backlog incidents
- cutover rollback

Davenda gives you the operational surfaces. Teams still need product-specific incident habits.
