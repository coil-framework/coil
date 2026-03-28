# Reference CLI Commands

**Part:** Appendices  
**Chapter:** 92

The platform CLI is the operator and developer control surface for migrations, configuration validation, release planning, auth diagnostics, storage work, TLS maintenance, and local development. The examples below use `coil` as the canonical binary name. Customer apps may re-export the same command tree under an app-specific binary, but behavior should remain consistent.

## Output Conventions

Every command should support:

- human-readable output by default
- `--json` for automation
- non-zero exit codes for validation or execution failure
- `--dry-run` where the command changes data, routing, auth state, or release state

Long-running commands should emit progress records rather than silent waits. Commands that calculate change plans should distinguish “no-op,” “warning,” and “unsafe to continue.”

## Core Command Groups

| Command group | Purpose |
| --- | --- |
| `coil dev` | Local development server, background worker bootstrap, and fixture loading |
| `coil config` | Validate, render, and diff effective configuration |
| `coil migrate` | Plan and apply core, module, and customer-app schema changes |
| `coil auth` | Validate auth packages, inspect bindings, run checks, and explain decisions |
| `coil module` | Install, enable, disable, and inspect official modules |
| `coil cache` | Warm, inspect, and invalidate cache scopes or tags |
| `coil storage` | Validate storage policy, sync managed assets, and inspect object-store state |
| `coil assets` | Publish build artifacts and verify asset manifests |
| `coil tls` | Check certificate status, renew, and validate challenge setup |
| `coil jobs` | Run workers, inspect queue health, and retry failed jobs |
| `coil import` | Run staged content or data imports |
| `coil release` | Produce upgrade plans, run compatibility checks, and mark release state |

## Representative Commands

```bash
coil config validate
coil migrate plan
coil migrate apply --dry-run
coil auth package validate auth/coil-default-auth
coil auth explain --subject user:42 --capability cms.page.publish --resource page:home
coil module list
coil cache warm --scope public --route /events
coil storage verify --policy
coil assets publish
coil tls renew
coil import run imports/wordpress-events.toml
coil release doctor
```

These examples are normative at the behavior level even if subcommand naming evolves slightly. Operators need a consistent mental model.

## Release And Migration Behavior

The CLI should treat upgrade planning as a first-class workflow. `coil release doctor` or its equivalent should check:

- core and module compatibility
- auth model package and capability registry compatibility
- pending config migrations
- pending schema migrations
- incompatible WASM extension host requirements

`coil migrate plan` should present changes grouped by owner, for example core, module, customer app, or auth package. This mirrors the architectural split and makes rollback reasoning much clearer.

## Auth Diagnostics

The auth command group is especially important because the platform uses relationship-based authorization. Operators and developers should be able to:

- validate an auth package before deployment
- inspect capability bindings
- run a batch of auth-model tests
- explain why a check passed or failed

Direct tuple-table inspection is not an adequate operator workflow by itself.

## Storage And TLS Operations

The storage and TLS command groups expose operationally sensitive work and should therefore require explicit confirmation or `--yes` in destructive paths. Examples include:

- revoking or replacing certificates
- forcing asset-policy rewrites
- reclassifying a managed asset from public to private
- re-running an object-store sync that may overwrite metadata

The CLI exists to make critical operations legible and scriptable, not hidden behind ad hoc scripts.
