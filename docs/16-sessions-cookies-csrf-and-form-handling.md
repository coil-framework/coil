# Sessions, Cookies, CSRF, and Form Handling

Stateful browser interaction is a first-class concern in this platform. Account management, checkout, memberships, bookings, admin tools, and editorial workflows all depend on reliable session and form behavior. Treating those concerns as lightweight add-ons would undermine the platform's main workload profile.

Sessions should be server-side in meaning even if the browser carries a signed identifier or compact envelope. The runtime needs the ability to invalidate sessions, constrain their lifetime, rotate keys, and choose storage backends appropriate to the deployment mode. In smaller installations that may mean a local or database-backed session store; in horizontally scaled environments it usually means a distributed backing service. What the platform should not do is treat the browser as an uncontrolled state database.

Cookies need safe defaults. Integrity-sensitive cookies should be signed, confidentiality-sensitive cookies should be encrypted, and security attributes such as `Secure`, `HttpOnly`, `SameSite`, domain, and path scope should be explicit in configuration rather than copied ad hoc by modules. Session cookies, remember-me cookies, and one-off browser state should all pass through the same core policy layer.

CSRF protection applies to any state-changing browser flow that uses ambient credentials. The platform should make token generation and validation a default behavior for first-party form helpers and route types that need it. The point is not only to reject malicious submissions, but also to make the safe path the easy path for module and customer-app developers.

Form handling itself should follow conventional HTML application patterns. Post-redirect-get should be the default for non-fragment flows. Validation errors, flash messages, field repopulation, and error summaries should have one transport model so that CMS, admin, checkout, and booking flows all behave consistently. Because accessibility is a platform contract, form helpers should also produce correct label wiring, error associations, and focus-management hooks by default.

Progressive enhancement adds another requirement: the form system must support both full-page and fragment-oriented responses. A customer app should be able to enhance a checkout step or an admin filter form without abandoning the same validation and CSRF pipeline that the full-page version uses. Browser state and form safety are therefore not module conventions. They are core runtime behavior that higher-level packages build on.
