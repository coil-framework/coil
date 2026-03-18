# Asset Pipeline and Hashed Build Artifacts

The platform distinguishes sharply between deploy-time build artifacts and managed media. Build artifacts are the compiled CSS, JavaScript, fonts, icons, and other static files that belong to a release of a customer app or official module. They are not treated like uploaded content, and they do not flow through the auth-governed media publication model.

## Build Artifacts Are Release Outputs

Core defines the asset publication contract: source inputs are compiled into immutable files whose names are derived from content hashes, and a manifest maps logical asset names to those hashed outputs. Templates and rendering helpers consume the manifest, never literal file paths. That makes cache busting deterministic and lets deployments switch releases by switching manifests rather than by rewriting templates or invalidating arbitrary URLs.

Official modules can contribute asset sources for their own UI, and customer apps can declare the entrypoints that compose those sources into branded bundles. The customer app is where the final storefront and admin bundles are chosen because it owns the theme and determines which official modules are installed. Core only guarantees that the resulting bundles are described in a stable manifest shape the runtime can consume.

## Publication Happens at Build or Deploy Time

Build artifacts are published before traffic sees them. They are not lazily synchronized on first request. A build produces hashed files, integrity metadata where needed, and a manifest for the exact release. Deployment then uploads those files to the configured public asset store, typically an S3-compatible backend behind a CDN, and activates the manifest for the new release.

This aligns with the broader storage model. Public theme and site assets are always publishable once a release is activated, so they do not need per-object auth. They are operational artifacts, not managed business resources. If a deployment must be rolled back, the system points back to the previous manifest and bundle set rather than attempting to edit individual files in place.

## Development Uses the Same Logical Contract

Local development can serve assets from a watcher or development server for speed, but it still has to respect the manifest boundary. The runtime should never care whether a logical bundle came from a production upload or a live local build. This is how customer apps can debug theme changes, enhancement scripts, and admin styling locally without training the templates to rely on environment-specific asset paths.

Source maps follow the same rule. They may be emitted during development and optionally stored for production diagnostics, but they are governed separately from the public bundles. The platform should not accidentally publish internal source maps to the same immutable public namespace used for shipping assets unless that behavior is configured explicitly.

## Boundaries With Extensions and Media

WASM extensions do not publish arbitrary first-class bundles directly into the global shell at runtime. If an extension needs front-end behavior, it should do so through declared host capabilities or through asset contributions that the customer app chooses to include in its build. That keeps the public asset surface reviewable and prevents runtime extension code from mutating a deployed release out from under the application.

The separation from managed media is equally important. A hashed CSS bundle for the storefront is a deployment artifact. A downloadable member guide, product image, or event hero asset is managed content and belongs to the media and storage-policy systems described later. The platform remains understandable only if those two worlds stay separate.
