# Security Policy

## Reporting A Vulnerability

Do not open a public issue for a security vulnerability.

Instead:

- prepare a minimal reproduction or clear description
- include affected crates, apps, or workflows
- include impact and any known mitigations
- send the report privately to the maintainers through the security contact configured for the public repository

If the public repository has GitHub Security Advisories enabled, use that path.

## What To Include

- affected component
- attack preconditions
- impact
- reproduction steps
- any proposed remediation

## Response Goals

Coil aims to:

- acknowledge valid reports promptly
- reproduce and assess severity
- prepare a fix and coordinated disclosure plan
- publish remediation notes when a fix is available

## Scope

Security reports are especially relevant for:

- auth and capability enforcement
- session and CSRF handling
- storage and asset publication
- extension isolation
- config and secret loading
- admin or operator surfaces
- CI and release workflows
