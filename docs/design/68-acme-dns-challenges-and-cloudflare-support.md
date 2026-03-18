# ACME, DNS Challenges, and Cloudflare Support

**Part:** Operations  
**Chapter:** 68

ACME is the default public-certificate path for the platform, and DNS automation is a first-class deployment concern rather than an optional convenience. The platform should support `http-01`, `tls-alpn-01`, and `dns-01`, with `dns-01` treated as the preferred challenge type for wildcard certificates, multi-node deployments, and CDN-fronted origins.

## Challenge Strategy

The challenge types are useful in different operational shapes:

- `http-01` works for simple direct-to-origin deployments where the application edge is publicly reachable
- `tls-alpn-01` is viable when the platform controls TLS termination directly and wants certificate proof at the edge listener
- `dns-01` is the most robust default for wildcard coverage, Cloudflare-fronted deployments, and horizontally scaled origins

The platform should choose the most stable method for the deployment instead of forcing one challenge path everywhere.

## Cloudflare Modes

Cloudflare support exists in two explicit modes:

- ACME public certificates with Cloudflare DNS automation for `dns-01`
- Cloudflare Origin CA certificates when the origin is only meant to serve Cloudflare

These modes solve different problems and should not be blurred together. Public browser trust still comes from ACME or another public CA. Origin CA is an origin-to-Cloudflare trust story, not a replacement for public certificates.

For customer apps routed through Cloudflare, the default security stance is Full (strict). The platform should not normalize weaker proxy modes because they hide certificate misconfiguration instead of fixing it.

## Secrets and Automation

DNS automation requires provider credentials, which must live in the platform's secrets system and be scoped to the minimum necessary zone operations. Extensions do not receive these credentials. Renewal jobs, DNS updates, and challenge cleanup are all host-managed operations.

Operators should expect the automation path to handle:

- wildcard certificates
- per-customer hostnames
- staged issuance and renewal
- retries and alerting when DNS propagation or provider API calls fail

## Multi-Node Considerations

`dns-01` is preferred in multi-node environments because it does not require the validation request to reach one specific origin node. That makes it a better fit for load-balanced clusters and CDN-fronted deployments where request routing may be opaque.

## Failure Handling

A failed challenge or renewal should leave the currently valid certificate in place, surface an alert, and produce actionable diagnostics. The platform should never delete working certificate material before replacement succeeds. In practice, ACME and Cloudflare support are only production-ready when issuance, renewal, and fallback states are all observable.
