# TLS and Certificate Lifecycle Management

**Part:** Operations  
**Chapter:** 67

TLS is a core operational service. The platform can terminate TLS itself or run behind external termination, but the certificate lifecycle is still treated as first-class platform behavior rather than an afterthought left to each customer app. The core owns certificate issuance workflows, secure storage of certificate material, SNI mapping, renewal scheduling, and hot reload.

## Deployment Modes

The platform supports three operational modes:

- built-in TLS termination at the application edge
- external termination behind a load balancer or CDN
- Cloudflare-origin deployments where the origin is intended to be reached only through Cloudflare

Customer apps choose hostname and certificate strategy, but TLS machinery remains in core. WASM extensions never participate in certificate issuance or gain access to raw private key material.

## Certificate Providers

The expected provider order is:

- ACME first
- Cloudflare Origin CA second
- manual certificate import third

ACME is the default path for public hostnames. Cloudflare Origin CA is appropriate when the origin is not directly exposed to browsers and Cloudflare is the required edge. Manual import remains available for exceptional environments, but it is not the preferred operating mode.

## Lifecycle

The lifecycle should be uniform regardless of customer app:

1. A hostname is bound to a customer app and validated against the chosen certificate policy.
2. The platform obtains or imports the certificate through the configured provider.
3. Certificate and key material are stored in encrypted platform-managed secrets storage, not in extension space.
4. Serving nodes reload the updated material without a full deployment when practical.
5. A renewal job runs ahead of expiry and emits alerts long before an outage window.

In multi-node deployments, certificate state must be shared. Renewal on one node cannot require manual redistribution to peers.

## Operational Boundaries

TLS choices interact with storage, proxying, and caching:

- external proxies must forward trusted scheme and host information correctly
- canonical URL generation and secure cookie behavior depend on accurate termination metadata
- Cloudflare deployments should default to Full (strict), not flexible modes
- certificate failures should degrade visibly and trigger operator action, not silently fall back to insecure behavior

## Customer-App Responsibility

Customer apps decide which domains they serve, whether they use public ACME certificates or Cloudflare-origin-only certificates, and how those hostnames map to brands or sites. They do not own renewal code, private key handling, or TLS policy enforcement. That split keeps domain onboarding flexible while leaving the security-critical machinery in the platform core.
