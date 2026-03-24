# Accessibility as a Platform Contract

Accessibility is not a theme preference, a late-stage audit task, or something left entirely to customer teams. It is a platform contract. The framework is explicitly designed for server-rendered, content-rich, transactional applications, which means it has unusually strong leverage over semantics, form behavior, focus management, and keyboard interaction. That leverage has to be used deliberately.

## What the Platform Must Guarantee

Core should ship accessible primitives for the things every customer app and official module will need: form fields and error summaries, label and description wiring, dialog and drawer foundations, tabular data patterns, navigation landmarks, skip links, heading structure helpers, status and validation announcements, and focus-management hooks for fragment updates. The platform also owns locale and direction metadata such as `lang` and `dir`, which are essential for screen readers and multilingual interfaces.

The baseline expectation for first-party UI is an AA-level accessible experience. That does not mean every possible interface concern can be solved in core, but it does mean the shipped defaults cannot be cavalier about semantics, contrast, keyboard support, reduced motion, or focus visibility. If those guarantees are absent from the primitives, customer apps will have to rediscover them badly and repeatedly.

## Official Modules Must Inherit, Not Undo, the Contract

Official modules are responsible for shipping accessible default interfaces in the domains they own. The CMS module must render authoring forms that expose errors correctly. The events module must render booking controls and availability messaging in a way assistive technology can understand. The admin shell must provide navigable tables, action bars, and dialogs that work without a mouse. Accessibility is part of "batteries included," not an optional enhancement pack.

Customer apps then theme and compose those surfaces, but they do not get to break the contract by hiding focus states, flattening headings into decorative markup, or relying on color alone to communicate state. Design tokens and theme linting should help here by making contrast, focus, and motion preferences testable at the theme level.

## Progressive Enhancement Has Accessibility Implications

Because the platform uses fragment updates heavily, accessibility rules have to extend beyond the initial page render. When part of a page changes, focus must move intentionally or remain stable for a good reason. Significant updates should be announced through appropriate live regions or status messaging. Enhanced controls must continue to fall back to ordinary forms and links when scripting is unavailable.

This is why accessibility belongs in the rendering and interaction contracts, not in a separate checklist. An inaccessible fragment endpoint is just as broken as an inaccessible full page.

## Extensions and Review

WASM extensions that contribute page fragments, admin widgets, or workflow UI inherit the same obligations. The host platform should provide accessible shells and primitives so extension authors are not tempted to reinvent them from scratch, but the extension review and test harness still need to verify keyboard, semantics, and error behavior for what the extension renders.

In practice, accessibility quality comes from three things working together: good primitives in core, disciplined defaults in official modules, and automated plus scenario-based checks in customer apps. If any one of those layers treats accessibility as someone else's job, the platform has failed its own contract.
