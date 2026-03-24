# Background Jobs, Scheduling, and Domain Events

The platform is monolith first, not request only. Background work is part of the normal architecture because the target workloads include payments, bookings, notifications, imports, asset processing, storage publication, search indexing, report generation, and certificate management. If the runtime tried to keep all of that on the request path, it would lose the latency and operational predictability that motivated the rewrite in the first place.

Jobs should be typed, idempotent, and observable. They take explicit input payloads, resolve normal services through the runtime, and emit logs, metrics, and traces just like request handling does. Retry policy is part of the job contract, not a transport accident. If a webhook delivery, image transformation, or storage synchronization job fails, the system needs a clear retry and dead-letter story rather than silent repetition or silent loss.

The scheduler is a core service rather than a loose cron convention. Typical scheduled work includes sitemap generation, search maintenance, cache warming, asset publication housekeeping, certificate renewal, data imports, and routine cleanup. In multi-node environments the scheduler must coordinate leadership explicitly so that one logical task does not execute everywhere at once.

Domain events connect modules and jobs without reintroducing hook soup. They represent meaningful transitions such as page publication, order completion, membership activation, booking cancellation, managed-asset publication, or certificate-renewal status changes. Event schemas should be typed, owned, and versioned where necessary. A module can react to them, but it should do so through deliberate handlers, not broad interception points with hidden ordering.

The relationship between jobs and storage policy is worth calling out. Public uploads may trigger object-store writes and CDN invalidation. Private shared assets may need signing metadata or proxy-ready indexing. Local-only sensitive files may require different replication or backup handling. These workflows belong in the job system because they are operational side effects with retry and observability requirements.

The same is true for customer-facing workflows. Booking confirmation mail, waitlist promotion, subscription follow-up, or bulk admin actions should generally run through queued work and typed events rather than being embedded inside request handlers. That keeps latency predictable and makes recovery from failures far easier.

Taken together, jobs, scheduling, and domain events are how the platform stays monolithic without becoming synchronous everywhere. They let the system remain one coherent application while still handling real operational workloads with explicit infrastructure instead of hidden callbacks.
