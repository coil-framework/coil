# Frontend Architecture with Stimulus, Turbo, PostCSS, esbuild, and SSR Fragments

**Part:** Rendering and Frontend
**Chapter:** 99

Coil is already SSR-first. The missing piece is a concrete frontend architecture that says how
interactive behavior, asset loading, customer overrides, and extension UI contributions are meant
to work without creating a second client-side application architecture next to the server-rendered
one.

This chapter defines that architecture. It assumes:

- HTML documents and HTML fragments remain the primary response types
- Turbo is the default transport for enhanced navigation and fragment replacement
- Stimulus is the default controller model for browser-side behavior
- PostCSS compiles stylesheet sources
- esbuild produces the final JavaScript and CSS bundles and their manifest entries

Those choices are deliberately narrow. The goal is operational clarity and strong boundaries, not
frontend maximalism.

## The Frontend Stack Is an Implementation of the HTML-First Model

Stimulus and Turbo are not an exception to Coil's SSR-first architecture. They are the default way
to implement it on the client side.

Turbo handles:

- enhanced navigation
- frame replacement
- form submission improvements
- HTML-over-the-wire updates

Stimulus handles:

- attaching behavior to server-rendered markup
- local presentation state
- wiring controls to fragment refresh or form submission behavior

PostCSS and esbuild handle:

- compiling the declared asset graph
- producing logical entrypoints and hashed outputs
- preserving the asset-manifest contract the runtime already expects

That means the browser-side stack is explicitly subordinate to the server-owned rendering model.
The document and fragment contract still starts on the server.

## Frontend Contributions Must Be Declared

The fundamental frontend unit is a declared contribution, not an arbitrary script tag or global CSS
file.

A contribution is a named frontend asset input that can belong to:

- core
- an official module
- a customer app
- a supported extension

Each contribution should declare at least:

- a stable logical name
- contribution type:
  - `controller`
  - `stylesheet`
  - `bundle`
  - `fragment_enhancement`
  - `admin_widget_bundle`
- intended surface:
  - `storefront`
  - `account`
  - `cms_page`
  - `admin`
  - `editor`
  - `shared`
- source entrypoint
- dependency edges on other declared contributions if needed

The runtime should not care whether a contribution originated in an official module or the customer
app. It should only care that the final route or fragment declares which logical bundles it needs.

## Route and Surface Loading Must Be Explicit

The rendered route or shell chooses the asset set, not the template ad hoc.

At minimum, the architecture should support these logical entrypoint groups:

- `storefront-shell`
- `account-shell`
- `admin-shell`
- `editor-shell`
- route- or fragment-specific additions such as:
  - `commerce-cart`
  - `cms-page-builder`
  - `admin-media-library`

The selection flow should be:

1. route resolves to a known surface
2. surface declares its base frontend bundles
3. participating module fragments add declared optional contributions
4. customer app may add or replace allowed contributions
5. runtime resolves the final logical bundle names through the asset manifest
6. layout emits the required `<script>` and `<link>` tags

This keeps the server in charge of which assets a page actually needs.

## Fragments Are Also Frontend Contracts

Fragments are not just HTML snippets. In this frontend architecture they are the stable unit of
partial rendering and enhancement.

A fragment contract should include:

- fragment identifier
- server-owned input model
- slot contract
- target surface
- declared frontend contributions
- Turbo usage mode if any:
  - frame content
  - stream target
  - ordinary fragment replacement
- accessibility obligations
- cache/auth scope

This matters because the same fragment may appear:

- inside a full document render
- inside a Turbo frame response
- as a partial refresh after a form post

The fragment must preserve the same semantics in all three places.

## Stimulus Controllers Attach to Server-Owned Markup

Stimulus is the preferred controller model because it keeps behavior close to rendered HTML without
moving business state into the browser.

The attachment contract should be:

- controllers attach through stable `data-controller` attributes
- values, targets, and actions are declared in markup
- controller behavior depends on server-rendered identifiers, URLs, and state markers
- controllers may manage ephemeral UI state, not authoritative business state

Examples of appropriate Stimulus behavior:

- expanding or collapsing sections
- wiring a filter form to a Turbo frame submission
- updating preview affordances after a server-rendered editor response
- managing a drag interaction that still posts the final state to a real handler

Examples of inappropriate behavior:

- deciding whether a publish transition is authorized
- calculating availability or membership entitlement in the browser
- reconstructing full screens from private JSON that bypasses SSR fragments

## Turbo Owns the Default HTML-over-the-Wire Path

Turbo is the preferred transport for enhanced interactions because it lines up with the platform's
existing fragment model.

Use it for:

- navigation that should preserve progress indicators and partial page replacement
- form submissions that should still degrade to normal POST + redirect
- frame-scoped refresh for lists, filters, side panels, and page-builder regions
- stream or targeted partial replacement where the server already owns the HTML fragment

Do not use it as cover for a second API architecture. If an interaction is naturally served by a
fragment response, the platform should return HTML, not JSON plus bespoke client rendering logic.

## Customer Apps Own the Final Frontend Composition

Official modules may declare frontend contributions, but customer apps own the final storefront and
admin composition.

That means the customer app should control:

- the top-level layout templates
- the base shell bundles
- design tokens and theme CSS layers
- presentation-level fragment overrides
- customer-owned Stimulus controllers
- whether optional module contributions are included in the final route or surface

This is the frontend equivalent of module composition. Modules contribute capability. Customer apps
compose the product.

## Official Modules Must Contribute Frontend Behavior Without Owning the Whole Shell

An official module should be able to ship:

- default templates
- default fragments
- Stimulus controllers for its own interactive regions
- stylesheet contributions for those regions
- admin/editor widgets for its own operational workflows

It should not have to ship:

- the entire storefront shell
- the entire admin shell
- a global client bundle that every app must accept unchanged

If a customer app needs to change layout or branding, it should be able to keep the module's
business fragment contract and replace only the presentation layer or wrapper layout.

## Extensions Contribute Through Declared Slots and Widgets

Extensions remain constrained. They do not inject arbitrary bundles into every page at runtime.

The safe extension model is:

- extension registers a slot or widget contribution for a documented surface
- extension may declare a small frontend bundle or controller set for that slot
- host decides whether the contribution is included in the final surface asset graph
- runtime enforces auth, rendering, and placement boundaries

Supported extension scenarios:

- admin dashboard widget
- sidebar panel in an editor surface
- storefront enhancement attached to a declared promotional slot

Unsupported scenarios:

- arbitrary head injection
- arbitrary shell rewrite
- undeclared global script execution

## PostCSS and esbuild Are Build Tools, Not Runtime Policy

The runtime should consume a stable manifest regardless of whether the frontend was built by one
tool or another. But for the first-party Coil architecture, PostCSS and esbuild are the standard
tools that produce that manifest.

### PostCSS Responsibilities

- token expansion and CSS transforms
- imports and composition
- nesting or other agreed stylesheet syntax
- autoprefixing or equivalent browser-target transforms

### esbuild Responsibilities

- JavaScript entrypoint bundling
- CSS entrypoint bundling where appropriate
- code splitting if needed for large admin/editor surfaces
- emitting hashed outputs
- emitting sourcemaps under explicit policy
- producing the manifest consumed by the runtime

The important point is not the tooling brand. It is that the dev and production pipeline remain
predictable and manifest-driven.

## Development Mode Must Preserve the Same Logical Contract

Development mode may use:

- an esbuild watcher
- a lightweight asset dev server
- incremental PostCSS rebuilds

But it must still preserve:

- the same logical entrypoint names used in production
- the same route-to-surface asset resolution model
- the same distinction between storefront and admin/editor bundles

Templates should never have to know whether an asset came from a dev watcher or a production upload.
They should ask for the logical asset and let the runtime resolve it.

## Admin and Editorial Surfaces Need a Distinct Frontend Discipline

Admin and editorial surfaces are the place where teams will feel pressure to abandon the HTML-first
model. That pressure is real, but the answer is not to turn the editor into a separate SPA by
default.

The better model is:

- richer Stimulus controllers
- more frequent Turbo frame or fragment updates
- stronger editor-specific bundle separation
- SSR previews and SSR-rendered validation feedback

Examples:

- page-builder side panel updates can be fragment-driven
- shared-block save/update flows can post to real handlers and return updated editor fragments
- editor preview can remain server-rendered while client code manages pane toggles or local drag
  affordances

This keeps the editor on the same operational model as the rest of the product:

- forms are still real forms
- authorization is still server-owned
- preview output is still SSR
- partial replacement is still HTML-over-the-wire

## Head and Asset Injection Must Stay Server-Owned

The layout and route model still own the page head and asset tags.

Frontend contributions may request:

- a stylesheet
- a deferred script
- a module script
- preload hints where explicitly supported

But they should not directly mutate the head at arbitrary runtime points. The resolved route or
fragment surface should declare the contribution set, and the layout renderer should emit the tags
deterministically.

That keeps CSP, cache behavior, and debugging tractable.

## Recommended Surface Taxonomy

The simplest coherent taxonomy is:

- `storefront`
- `account`
- `cms-public`
- `admin`
- `editor`
- `shared`

Why this split works:

- public storefront and account often share some shell concerns but not all controllers
- CMS public pages may need some editorially driven blocks but should not inherit editor tooling
- admin and editor need richer bundles and different a11y workflows
- shared contributions remain possible without forcing one giant global bundle

## Customer Override Order

Frontend override order should mirror the existing product shape:

1. core defines rendering and asset-manifest rules
2. official modules define default templates, fragments, and contributions
3. customer app selects modules, layouts, bundles, and allowed overrides
4. extensions contribute only through declared slots and surfaces

This is the only override order that keeps the customer app in control without letting extensions or
modules take over the shell unexpectedly.

## Testing Implications

This architecture needs test coverage at three layers:

- render tests:
  - route emits the right bundles
  - fragment emits the right contract markers
- browser behavior tests:
  - Turbo-enhanced flows still work as plain HTML when scripts are absent
  - Stimulus controllers attach through stable attributes
- asset-pipeline tests:
  - manifest contains declared logical entries
  - storefront and admin/editor bundles remain separate

The key invariant is simple: if the scripts fail to load, the document still works. If the scripts
do load, they enhance the server-rendered contract rather than replacing it.

## Migration Implications

Existing Coil apps do not need to adopt every part of this architecture at once.

The smallest honest migration path is:

1. keep SSR templates and fragment responses as they are
2. define declared logical bundles for existing public and admin surfaces
3. move browser behavior into Stimulus controllers attached to server-rendered markup
4. use Turbo for enhanced navigation and fragment submission where that fits
5. reserve JSON for explicit APIs and genuinely client-heavy exceptions

That path improves structure without requiring a frontend rewrite.

## Bottom Line

The Coil frontend architecture should not be "SSR plus whatever JavaScript happens to accumulate."
It should be:

- SSR documents and SSR fragments as the default contract
- Turbo as the default HTML-over-the-wire transport
- Stimulus as the default controller model
- PostCSS and esbuild as the standard build pipeline
- customer apps owning final frontend composition
- official modules contributing frontend behavior through declared contracts
- extensions participating only through declared slots and surfaces

That is narrow enough to stay maintainable and flexible enough to support real storefront,
membership, CMS, and admin/editor products.
