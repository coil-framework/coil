# cargo-coil

`cargo-coil` is the Cargo subcommand for creating, running, and evolving Coil customer projects.

Install it once:

```bash
cargo install cargo-coil --locked
```

Then use it as:

```bash
cargo coil <command>
```

## What It Does

`cargo-coil` owns the project lifecycle around a customer-root Coil workspace:

- interactive or non-interactive project generation
- in-place project initialisation
- descriptor-backed regeneration
- drift detection
- module, site, and locale edits
- host-native local development with Docker-backed infra

All generated and maintained state flows through `.coil/project.toml`.

## Main Commands

### `cargo coil new`

Create a new Coil project in a new directory.

Interactive by default:

```bash
cargo coil new my-store
```

Non-interactive:

```bash
cargo coil new my-store \
  --non-interactive \
  --name my-store \
  --display-name "My Store" \
  --default-locale en-GB \
  --locale fr-FR \
  --module cms \
  --module commerce \
  --framework-version latest
```

Important flags:

- `--non-interactive`
  - skip the wizard and use command-line values only
  - `--no-input` is still accepted as a compatibility alias
- `--framework-version <version|latest>`
  - use a specific published Coil framework version
  - `latest` resolves from crates.io
  - if omitted, `cargo-coil` tries crates.io first and falls back to its built-in default
- `--source crates-io`
  - generate dependencies that resolve from crates.io
- `--source path --coil-path /path/to/coil`
  - generate path dependencies against a local Coil checkout

### `cargo coil init`

Initialise a Coil project in the current directory or another existing directory.

```bash
cargo coil init
```

Non-interactive:

```bash
cargo coil init \
  --root . \
  --non-interactive \
  --name my-store \
  --display-name "My Store"
```

`init` accepts the same project-shaping flags as `new`.

### `cargo coil dev`

Run the generated customer app on the host while using Docker Compose for local infra.

Default behaviour:

```bash
cargo coil dev
```

What it does:

1. loads `.coil/project.toml`
2. starts `postgres` and `redis` with `docker compose up -d`
3. injects local development defaults for:
   - `DATABASE_URL`
   - `REDIS_URL`
   - `COIL_COOKIE_SECRET`
   - `COIL_CSRF_SECRET`
4. runs the customer app through Cargo
5. watches the workspace and restarts the customer app when files change

Examples:

```bash
cargo coil dev
cargo coil dev --no-watch
cargo coil dev --skip-infra
cargo coil dev --bind 127.0.0.1:9090
cargo coil dev --config platform.dev.toml
```

Notes:

- `--no-watch` runs a single host-native app process
- `--skip-infra` is useful if Postgres and Redis are already running
- environment variables you set in your shell override the default local values
- watch mode preserves the customer binary's normal stdout and stderr, so bootstrap and runtime
  failures are shown directly rather than collapsed into a watcher wrapper error

### `cargo coil apply`

Reconcile the generated workspace from `.coil/project.toml`.

```bash
cargo coil apply
```

Use this after editing the descriptor directly or after resolving merge conflicts in generated files.

### `cargo coil doctor`

Check whether the workspace matches `.coil/project.toml`.

```bash
cargo coil doctor
```

This is the drift-detection command. It exits with an error if generated files no longer match the descriptor.

### `cargo coil module add`

Enable modules in the project descriptor and regenerate managed files.

```bash
cargo coil module add memberships events
```

### `cargo coil module remove`

Disable modules in the project descriptor and regenerate managed files.

```bash
cargo coil module remove events
```

### `cargo coil site add`

Add a new site to a multi-site project.

```bash
cargo coil site add eu \
  --display-name "EU Store" \
  --brand-name "My Store" \
  --canonical-domain eu.my-store.localhost \
  --domain www.eu.my-store.localhost \
  --default-locale fr-FR
```

### `cargo coil locale add`

Add a locale to a specific site or to the default site.

```bash
cargo coil locale add fr-FR --site eu
cargo coil locale add pl-PL --default-for-site
```

## Typical Workflow

Create a store:

```bash
cargo coil new my-store
cd my-store
docker compose up --build
```

Evolve it later:

```bash
cargo coil module add memberships
cargo coil site add eu --canonical-domain eu.my-store.localhost --default-locale fr-FR
cargo coil locale add de-DE --site eu
cargo coil apply
cargo coil doctor
```

Use the faster host-native loop:

```bash
cargo install cargo-watch --locked
cargo coil dev
```

## Version Selection

Generated projects pin an explicit Coil framework version in `.coil/project.toml`.

This version is separate from the `cargo-coil` release itself. That matters because:

- `cargo-coil` can release more frequently than the framework
- generated projects stay reproducible
- upgrades can be intentional instead of accidental

Examples:

```bash
cargo coil new my-store --framework-version 0.1.0
cargo coil new my-store --framework-version latest
```

## Generated Project Contract

`cargo-coil` treats `.coil/project.toml` as the source of truth.

That means:

- `new` and `init` create the descriptor
- `module add`, `site add`, and `locale add` edit the descriptor
- `apply` rewrites managed files from the descriptor
- `doctor` checks whether the workspace still matches it

The intended flow is to evolve the project through `cargo-coil`, not by hand-editing generated files and hoping they stay aligned.
