---
title: Observability, Monitoring, And Audit
---

Coil already exposes a real operational boundary in the checked-in apps, but the public examples
show that boundary in a narrower way than a full production observability stack.

Use this page to understand what is demonstrably present today:

- readiness and health probes
- runtime observability toggles
- Shoppr audit surfaces
- product-visible operator signals around orders and admin actions

## The Smallest Real Contract

Both public apps enable observability in runtime config:

```toml
[observability]
metrics = true
tracing = true
```

You can see that in:

- `apps/shoppr/platform.dev.toml`
- `apps/gitly/platform.dev.toml`

That means observability is not a postscript. It is part of the runtime contract the customer app
ships with.

## Health And Readiness

The first signals a new developer should use are still the simplest ones:

- `/ready` for the main app
- `/health` for sidecars or integration adapters where present

In the checked-in local stacks, the Docker healthchecks already rely on those endpoints. That is
the right mental model to copy: deployment control should use the same probes the operator uses.

## Shoppr Is The Main Audit Example

Shoppr’s admin audit page is the strongest public operator-trust example in the repo.

The template itself tells you what the runtime is shaping:

```html
<p>
  Backend <code coil:text="${auditBackend}">local-sqlite</code> at
  <code coil:text="${auditLocation}">/var/lib/coil/shared-state</code> with
  <strong coil:text="${auditEntryCount}">0</strong> recorded entries.
</p>
...
<tr coil:each="entry : ${auditEntries}">
  <td coil:text="${entry.when}">1764223200</td>
  <td coil:text="${entry.actor}">operator-live-1</td>
  <td coil:text="${entry.action}">Issue refund</td>
  <td coil:text="${entry.capability}">order.refund.issue</td>
</tr>
```

That is a real boundary, not marketing copy. The public app is teaching that operators should be
able to inspect:

- who acted
- what they did
- which capability/resource it mapped to
- whether the action succeeded

## Shoppr Also Shows Product-Side Operational Truth

The orders page is part of the observability story too:

```html
<p>
  This queue is store-wide. Use it to confirm payment state, review checkout email and totals,
  and move into the per-order support detail view before escalating a checkout case.
</p>
```

That is important because observability is not only about logs and dashboards. It is also about
whether the product exposes truthful operator state where humans actually work.

## What To Use Locally

For first-pass diagnosis, use:

```bash
docker compose ps
docker compose logs app
curl -fsS http://localhost:8080/ready
```

Then verify app-specific surfaces:

- Shoppr: `/admin`, `/admin/audit`, `/admin/orders`
- Gitly: `/`, `/explore`, `/forgeflow/platform-ui/actions`

Gitly is not the audit example. It is the lightweight “runtime toggles plus product shell” example.

## What The Public Repo Does Not Yet Teach End To End

The checked-in apps do not currently give you:

- a ready-made metrics dashboard definition
- a production trace backend walkthrough
- a second app with a Shoppr-level audit UI

So the honest public claim is narrower:

- the runtime exposes the observability switches
- health/readiness are part of the deployment contract
- Shoppr proves audit and operator visibility in a real app

That is enough to teach the boundary without pretending the repo already ships every downstream
monitoring integration cookbook.

## Common Mistakes

### Treating `/ready` as optional

If rollout or local diagnosis ignores readiness, operators will end up debugging by guesswork.

### Treating audit as just another log stream

Audit is the durable privileged-action lane. Logs and traces do not replace it.

### Claiming “full observability” because metrics and tracing are enabled

The toggles are real, but the public examples still stop short of shipping a complete dashboard and
trace-export tutorial.

## Read Next

- [Jobs and schedulers](jobs-and-schedulers.md)
- [Troubleshooting](troubleshooting.md)
- [Shoppr Observability And Audit](../use-cases/shoppr/observability-and-audit.md)
