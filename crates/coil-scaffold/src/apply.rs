use crate::{DependencySource, ProjectDescriptor, SiteDescriptor};
use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const DESCRIPTOR_DIR: &str = ".coil";
const DESCRIPTOR_FILE: &str = "project.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectLocation<'a> {
    New(&'a Path),
    Existing(&'a Path),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleEditAction {
    Add,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteAddOptions {
    pub id: String,
    pub display_name: String,
    pub brand_name: String,
    pub canonical_domain: String,
    pub additional_domains: Vec<String>,
    pub default_locale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleAddOptions {
    pub site_id: String,
    pub locale: String,
    pub make_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedProjectFile {
    pub path: PathBuf,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReport {
    pub root: PathBuf,
    pub files_written: usize,
}

pub fn descriptor_path(root: impl AsRef<Path>) -> PathBuf {
    root.as_ref().join(DESCRIPTOR_DIR).join(DESCRIPTOR_FILE)
}

pub fn save_descriptor(root: impl AsRef<Path>, descriptor: &ProjectDescriptor) -> Result<()> {
    descriptor.validate()?;
    let path = descriptor_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&path, toml::to_string_pretty(descriptor)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn load_descriptor(root: impl AsRef<Path>) -> Result<ProjectDescriptor> {
    let path = descriptor_path(root);
    let input =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let descriptor: ProjectDescriptor = toml::from_str(&input)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    descriptor.validate()?;
    Ok(descriptor)
}

pub fn create_project(root: impl AsRef<Path>, descriptor: &ProjectDescriptor) -> Result<ApplyReport> {
    let root = root.as_ref();
    if root.exists() {
        let mut entries = fs::read_dir(root)
            .with_context(|| format!("failed to inspect {}", root.display()))?;
        if entries.next().transpose()?.is_some() {
            bail!("destination `{}` is not empty", root.display());
        }
    } else {
        fs::create_dir_all(root)
            .with_context(|| format!("failed to create {}", root.display()))?;
    }
    apply_descriptor(root, descriptor)
}

pub fn apply_descriptor(root: impl AsRef<Path>, descriptor: &ProjectDescriptor) -> Result<ApplyReport> {
    let root = root.as_ref();
    descriptor.validate()?;
    save_descriptor(root, descriptor)?;
    let files = build_descriptor(descriptor)?;
    for file in &files {
        let path = root.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, &file.contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(ApplyReport {
        root: root.to_path_buf(),
        files_written: files.len() + 1,
    })
}

pub fn build_descriptor(descriptor: &ProjectDescriptor) -> Result<Vec<RenderedProjectFile>> {
    descriptor.validate()?;
    let mut files = vec![
        RenderedProjectFile {
            path: PathBuf::from("Cargo.toml"),
            contents: root_cargo_toml(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from(".gitignore"),
            contents: gitignore(),
        },
        RenderedProjectFile {
            path: PathBuf::from(".env.example"),
            contents: env_example(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from(".dockerignore"),
            contents: dockerignore(),
        },
        RenderedProjectFile {
            path: PathBuf::from("Dockerfile"),
            contents: dockerfile(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from("README.md"),
            contents: project_readme(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from("docker-compose.yml"),
            contents: docker_compose(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from("app.toml"),
            contents: app_manifest(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from("platform.dev.toml"),
            contents: platform_dev(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from("platform.toml"),
            contents: platform_prod(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from("templates/layouts/base.html"),
            contents: base_layout(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from("templates/pages/home.html"),
            contents: home_template(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from("theme/assets/site.css"),
            contents: site_css(),
        },
        RenderedProjectFile {
            path: PathBuf::from("theme/assets/site.js"),
            contents: site_js(),
        },
        RenderedProjectFile {
            path: PathBuf::from(format!(
                "crates/{}/Cargo.toml",
                descriptor.bin_crate_dir_name()
            )),
            contents: bin_cargo_toml(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from(format!(
                "crates/{}/src/main.rs",
                descriptor.bin_crate_dir_name()
            )),
            contents: bin_main_rs(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from(format!(
                "crates/{}/Cargo.toml",
                descriptor.backend_crate_name()
            )),
            contents: backend_cargo_toml(descriptor),
        },
        RenderedProjectFile {
            path: PathBuf::from(format!(
                "crates/{}/src/lib.rs",
                descriptor.backend_crate_name()
            )),
            contents: backend_lib_rs(descriptor),
        },
    ];

    if descriptor.tooling.wasm_directory {
        files.push(RenderedProjectFile {
            path: PathBuf::from("extensions/.gitkeep"),
            contents: String::new(),
        });
    }

    let locales: BTreeSet<&str> = descriptor
        .i18n
        .supported_locales
        .iter()
        .map(String::as_str)
        .collect();
    for locale in locales {
        files.push(RenderedProjectFile {
            path: PathBuf::from(format!("translations/{locale}.toml")),
            contents: translation_catalog(descriptor, locale),
        });
    }

    Ok(files)
}

pub fn modify_modules(
    root: impl AsRef<Path>,
    action: ModuleEditAction,
    modules: &[String],
) -> Result<ApplyReport> {
    let root = root.as_ref();
    let mut descriptor = load_descriptor(root)?;
    let mut enabled: BTreeSet<String> = descriptor.modules.enabled.iter().cloned().collect();
    for module in modules {
        match action {
            ModuleEditAction::Add => {
                enabled.insert(module.clone());
            }
            ModuleEditAction::Remove => {
                enabled.remove(module);
            }
        }
    }
    descriptor.modules.enabled = enabled.into_iter().collect();
    if descriptor.modules.enabled.is_empty() {
        bail!("at least one module must remain enabled");
    }
    apply_descriptor(root, &descriptor)
}

pub fn add_site(root: impl AsRef<Path>, options: SiteAddOptions) -> Result<ApplyReport> {
    let root = root.as_ref();
    let mut descriptor = load_descriptor(root)?;
    if !descriptor
        .i18n
        .supported_locales
        .contains(&options.default_locale)
    {
        descriptor
            .i18n
            .supported_locales
            .push(options.default_locale.clone());
    }
    descriptor.add_site(SiteDescriptor {
        id: options.id,
        display_name: options.display_name,
        brand_name: options.brand_name,
        canonical_domain: options.canonical_domain,
        additional_domains: options.additional_domains,
        default_locale: options.default_locale.clone(),
        supported_locales: vec![options.default_locale],
    })?;
    apply_descriptor(root, &descriptor)
}

pub fn add_locale(root: impl AsRef<Path>, options: LocaleAddOptions) -> Result<ApplyReport> {
    let root = root.as_ref();
    let mut descriptor = load_descriptor(root)?;
    descriptor.add_locale(options.locale.clone(), &options.site_id)?;
    if options.make_default {
        if options.site_id == descriptor.default_site().id {
            descriptor.i18n.default_locale = options.locale.clone();
        }
        let site = descriptor
            .sites
            .iter_mut()
            .find(|site| site.id == options.site_id)
            .ok_or_else(|| anyhow!("site `{}` does not exist", options.site_id))?;
        site.default_locale = options.locale;
    }
    apply_descriptor(root, &descriptor)
}

fn root_cargo_toml(descriptor: &ProjectDescriptor) -> String {
    let dependency_block = match &descriptor.tooling.dependency_source {
        DependencySource::CratesIo => format!(
            "coil = {{ package = \"coil-rs\", version = \"{version}\" }}\ncoil-customer-sdk = \"{version}\"",
            version = descriptor.tooling.coil_version
        ),
        DependencySource::Path { repo_root } => format!(
            "coil = {{ package = \"coil-rs\", path = \"{repo_root}/crates/coil\" }}\ncoil-customer-sdk = {{ path = \"{repo_root}/crates/coil-customer-sdk\" }}"
        ),
    };
    format!(
        r#"# Generated by cargo-coil. Re-run `cargo coil apply` after editing .coil/project.toml.
[workspace]
resolver = "3"
members = [
    "crates/{backend}",
    "crates/{bin_dir}",
]
default-members = ["crates/{bin_dir}"]

[workspace.package]
edition = "2024"
version = "0.1.0"
rust-version = "1.85"

[workspace.dependencies]
anyhow = "1"
clap = {{ version = "4.5", features = ["derive"] }}
tokio = {{ version = "1", features = ["macros", "net", "rt-multi-thread", "signal"] }}
{dependency_block}
"#,
        backend = descriptor.backend_crate_name(),
        bin_dir = descriptor.bin_crate_dir_name(),
        dependency_block = dependency_block,
    )
}

fn gitignore() -> String {
    r#"/target
/.coil/cache
.DS_Store
.env
"#
    .to_string()
}

fn dockerignore() -> String {
    r#"/target
/.git
/.coil/cache
.DS_Store
"#
    .to_string()
}

fn env_example(descriptor: &ProjectDescriptor) -> String {
    format!(
        "DATABASE_URL=postgres://coil:coil@127.0.0.1:15432/{name}\nREDIS_URL=redis://127.0.0.1:16379/0\nCOIL_COOKIE_SECRET=replace-me-with-a-long-random-secret\nCOIL_CSRF_SECRET=replace-me-with-a-long-random-secret\n",
        name = descriptor.project_slug()
    )
}

fn dockerfile(descriptor: &ProjectDescriptor) -> String {
    format!(
        r#"FROM rust:1.88-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
COPY . .

RUN cargo build --release -p "{bin_package}"

CMD ["./target/release/{bin_package}", "serve"]
"#,
        bin_package = descriptor.bin_crate_package_name(),
    )
}

fn project_readme(descriptor: &ProjectDescriptor) -> String {
    let dependency_source = match &descriptor.tooling.dependency_source {
        DependencySource::CratesIo => {
            "Dependencies resolve from crates.io, so `docker compose up --build` works out of the box.".to_string()
        }
        DependencySource::Path { repo_root } => format!(
            "Dependencies resolve from the local Coil checkout at `{repo_root}`. Host-native `cargo run` workflows are the safer default when using path dependencies."
        ),
    };
    format!(
        r#"# {display_name}

This project was generated by `cargo coil`.

## Local development

1. Start the full local stack:

   ```bash
   docker compose up --build
   ```

   This starts Postgres, Redis, and the generated Coil app on `http://127.0.0.1:8080`.

2. Validate the workspace on the host:

   ```bash
   cargo run -p {bin_package} -- validate
   ```

## Run on the host instead

If you want to run the app directly from your shell instead of Docker, export the required environment and start the binary:

   ```bash
   export DATABASE_URL=postgres://coil:coil@127.0.0.1:15432/{slug}
   export REDIS_URL=redis://127.0.0.1:16379/0
   export COIL_COOKIE_SECRET=replace-me-with-a-long-random-secret
   export COIL_CSRF_SECRET=replace-me-with-a-long-random-secret
   cargo run -p {bin_package} -- validate
   cargo run -p {bin_package} -- serve
   ```

The app root is the workspace root. Edit `.coil/project.toml`, then re-run `cargo coil apply` to reconcile the generated files.

{dependency_source}
"#,
        display_name = descriptor.project.display_name,
        slug = descriptor.project_slug(),
        bin_package = descriptor.bin_crate_package_name(),
        dependency_source = dependency_source,
    )
}

fn docker_compose(descriptor: &ProjectDescriptor) -> String {
    format!(
        r#"services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_USER: coil
      POSTGRES_PASSWORD: coil
      POSTGRES_DB: {name}
    ports:
      - "15432:5432"
    volumes:
      - postgres-data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U coil -d {name}"]
      interval: 5s
      timeout: 5s
      retries: 20

  redis:
    image: redis:7
    ports:
      - "16379:6379"
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 5s
      retries: 20

  app:
    build:
      context: .
      dockerfile: Dockerfile
    environment:
      DATABASE_URL: postgres://coil:coil@postgres:5432/{name}
      REDIS_URL: redis://redis:6379/0
      COIL_COOKIE_SECRET: local-development-cookie-secret
      COIL_CSRF_SECRET: local-development-csrf-secret
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    ports:
      - "8080:8080"

volumes:
  postgres-data:
"#,
        name = descriptor.project_slug()
    )
}

fn app_manifest(descriptor: &ProjectDescriptor) -> String {
    let supported_locales = toml_array(&descriptor.i18n.supported_locales);
    let translations = descriptor
        .i18n
        .supported_locales
        .iter()
        .map(|locale| {
            format!(
                "[[translations.catalogs]]\nlocale = \"{locale}\"\npath = \"translations/{locale}.toml\"\n"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let sites = descriptor
        .sites
        .iter()
        .map(|site| {
            format!(
                "[[sites]]\nid = \"{id}\"\ndisplay_name = \"{display_name}\"\nbrand_name = \"{brand_name}\"\ncanonical_domain = \"{canonical_domain}\"\nadditional_domains = {additional_domains}\ndefault_locale = \"{default_locale}\"\nsupported_locales = {supported_locales}\n",
                id = site.id,
                display_name = site.display_name,
                brand_name = site.brand_name,
                canonical_domain = site.canonical_domain,
                additional_domains = toml_array(&site.additional_domains),
                default_locale = site.default_locale,
                supported_locales = toml_array(&site.supported_locales),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"[app]
name = "{name}"
display_name = "{display_name}"

[domains]
canonical = "{canonical}"
additional = {additional_domains}

[i18n]
default_locale = "{default_locale}"
supported_locales = {supported_locales}
localized_routes = {localized_routes}

[translations]

{translations}
[theme]
active = "starter"
template_namespaces = ["customer-app"]
asset_roots = ["theme/assets"]

[auth]
mode = "extend"
package = "coil-default-auth"

[modules]
enabled = {modules}

{sites}
"#,
        name = descriptor.project.name,
        display_name = descriptor.project.display_name,
        canonical = descriptor.default_site().canonical_domain,
        additional_domains = toml_array(&descriptor.default_site().additional_domains),
        default_locale = descriptor.i18n.default_locale,
        supported_locales = supported_locales,
        localized_routes = descriptor.i18n.localized_routes,
        translations = translations,
        modules = toml_array(&descriptor.modules.enabled),
        sites = sites,
    )
}

fn platform_dev(descriptor: &ProjectDescriptor) -> String {
    platform_config(descriptor, false)
}

fn platform_prod(descriptor: &ProjectDescriptor) -> String {
    platform_config(descriptor, true)
}

fn platform_config(descriptor: &ProjectDescriptor, production: bool) -> String {
    let environment = if production {
        "production"
    } else {
        "development"
    };
    let secure = if production { "true" } else { "false" };
    let session_store = "redis";
    let l2 = "l2 = \"redis\"\n";
    let tls_mode = if production { "external" } else { "external" };
    let sites = descriptor
        .sites
        .iter()
        .map(|site| {
            let mut hosts = Vec::with_capacity(1 + site.additional_domains.len());
            hosts.push(site.canonical_domain.clone());
            hosts.extend(site.additional_domains.iter().cloned());
            format!(
                "[[sites]]\nid = \"{id}\"\ndisplay_name = \"{display_name}\"\nbrand_name = \"{brand_name}\"\ncanonical_host = \"{canonical_host}\"\nhosts = {hosts}\ndefault_locale = \"{default_locale}\"\nsupported_locales = {supported_locales}\n",
                id = site.id,
                display_name = site.display_name,
                brand_name = site.brand_name,
                canonical_host = site.canonical_domain,
                hosts = toml_array(&hosts),
                default_locale = site.default_locale,
                supported_locales = toml_array(&site.supported_locales),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"[app]
name = "{name}"
environment = "{environment}"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = []

[http.session]
store = "{session_store}"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "coil_session"
path = "/"
same_site = "lax"
secure = {secure}
http_only = true

[http.flash_cookie]
name = "coil_flash"
path = "/"
same_site = "lax"
secure = {secure}
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "{tls_mode}"

[database]
url = {{ kind = "env", var = "DATABASE_URL" }}
schema = "public"
migrations_table = "_coil_migrations"
min_connections = 4
max_connections = 32
statement_timeout_secs = 30

[storage]
default_class = "public_upload"
deployment = "single_node"
single_node_escape_hatch = "explicit_single_node"
local_root = ".coil"

[cache]
l1 = "moka"
{l2}
[i18n]
default_locale = "{default_locale}"
supported_locales = {supported_locales}
fallback_locale = "{default_locale}"
localized_routes = {localized_routes}

[seo]
canonical_host = "{canonical_host}"
emit_json_ld = true

[auth]
package = "coil-default-auth"
explain_api = false
tenant_id = 1

[modules]
enabled = {modules}

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = false

{sites}
"#,
        name = descriptor.project.name,
        environment = environment,
        session_store = session_store,
        secure = secure,
        tls_mode = tls_mode,
        l2 = l2,
        default_locale = descriptor.i18n.default_locale,
        supported_locales = toml_array(&descriptor.i18n.supported_locales),
        localized_routes = descriptor.i18n.localized_routes,
        canonical_host = descriptor.default_site().canonical_domain,
        modules = toml_array(&descriptor.modules.enabled),
        sites = sites,
    )
}

fn base_layout(descriptor: &ProjectDescriptor) -> String {
    format!(
        r#"<!doctype html>
<html xmlns:coil="https://coil.rs" coil:fragment="shell" lang="{locale}" coil:attr="lang=${{locale}}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${{pageTitle}}">{title}</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
    <script src="/theme/assets/site.js" coil:src="asset('theme/assets/site.js')" defer="defer"></script>
  </head>
  <body>
    <main coil:slot="content">
      <section>Starter content</section>
    </main>
  </body>
</html>
"#,
        locale = descriptor.i18n.default_locale,
        title = descriptor.project.display_name,
    )
}

fn home_template(descriptor: &ProjectDescriptor) -> String {
    format!(
        r#"<!doctype html>
<html xmlns:coil="https://coil.rs" coil:with="pageTitle=t('home.meta.title')" lang="{locale}" coil:attr="lang=${{locale}}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${{pageTitle}}">{title}</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
    <script src="/theme/assets/site.js" coil:src="asset('theme/assets/site.js')" defer="defer"></script>
  </head>
  <body class="coil-starter">
    <header class="hero">
      <p class="eyebrow" coil:t="home.eyebrow">Coil storefront starter</p>
      <h1 coil:t="home.title">{title}</h1>
      <p class="summary" coil:t="home.summary">A customer-root storefront generated by cargo coil.</p>
    </header>
    <section class="switchers">
      <article>
        <h2 coil:t="nav.market">Market</h2>
        <ul>
          <li coil:each="item : ${{links.siteSwitches}}">
            <a href="/" coil:attr="href=${{item.href}}" coil:text="${{item.label}}">Primary site</a>
            <span coil:if="${{item.active}}" coil:t="nav.current">Current</span>
          </li>
        </ul>
      </article>
      <article>
        <h2 coil:t="nav.language">Language</h2>
        <ul>
          <li coil:each="item : ${{links.localeSwitches}}">
            <a href="/" coil:attr="href=${{item.href}}" coil:text="${{item.label}}">English</a>
            <span coil:if="${{item.active}}" coil:t="nav.current">Current</span>
          </li>
        </ul>
      </article>
    </section>
    <section class="actions">
      <a class="button" href="/admin" coil:t="home.primary_cta">Open admin</a>
      <a class="button button--secondary" href="/__dev" coil:t="home.secondary_cta">Open dev tools</a>
    </section>
  </body>
</html>
"#,
        locale = descriptor.i18n.default_locale,
        title = descriptor.project.display_name,
    )
}

fn site_css() -> String {
    r#":root {
  color-scheme: light dark;
  --bg: #f6f2eb;
  --panel: rgba(255, 255, 255, 0.88);
  --text: #16120d;
  --muted: #6b6257;
  --line: rgba(22, 18, 13, 0.12);
  --accent: #1f5eff;
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg: #120f0b;
    --panel: rgba(26, 21, 18, 0.92);
    --text: #f6f2eb;
    --muted: #c7beb2;
    --line: rgba(246, 242, 235, 0.14);
    --accent: #8cb0ff;
  }
}

* { box-sizing: border-box; }

body.coil-starter {
  margin: 0;
  min-height: 100vh;
  font-family: "Manrope", system-ui, sans-serif;
  color: var(--text);
  background:
    radial-gradient(circle at top left, rgba(31, 94, 255, 0.12), transparent 30rem),
    linear-gradient(180deg, var(--bg), color-mix(in srgb, var(--bg) 88%, #ffffff 12%));
  padding: 3rem 1.5rem 4rem;
}

.hero,
.switchers,
.actions {
  max-width: 70rem;
  margin: 0 auto 2rem;
}

.hero {
  padding: 2rem;
  border: 1px solid var(--line);
  border-radius: 1.5rem;
  background: var(--panel);
  backdrop-filter: blur(18px);
}

.eyebrow {
  margin: 0 0 0.5rem;
  font-size: 0.8rem;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--muted);
}

.hero h1 {
  margin: 0 0 1rem;
  font-size: clamp(2.5rem, 6vw, 4.75rem);
  line-height: 0.95;
}

.summary {
  max-width: 42rem;
  font-size: 1.05rem;
  color: var(--muted);
}

.switchers {
  display: grid;
  gap: 1rem;
  grid-template-columns: repeat(auto-fit, minmax(18rem, 1fr));
}

.switchers article {
  padding: 1.5rem;
  border: 1px solid var(--line);
  border-radius: 1.25rem;
  background: var(--panel);
}

.switchers ul {
  list-style: none;
  padding: 0;
  margin: 1rem 0 0;
  display: grid;
  gap: 0.75rem;
}

.switchers li {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.switchers a,
.button {
  color: inherit;
  text-decoration: none;
}

.actions {
  display: flex;
  gap: 1rem;
  flex-wrap: wrap;
}

.button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0.9rem 1.2rem;
  border-radius: 999px;
  border: 1px solid transparent;
  background: var(--accent);
  color: white;
  font-weight: 700;
}

.button--secondary {
  background: transparent;
  border-color: var(--line);
  color: var(--text);
}
"#
    .to_string()
}

fn site_js() -> String {
    r#"document.documentElement.dataset.coilStarter = "ready";
"#
    .to_string()
}

fn bin_cargo_toml(descriptor: &ProjectDescriptor) -> String {
    format!(
        r#"[package]
name = "{package_name}"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish = false

[dependencies]
anyhow.workspace = true
clap.workspace = true
coil.workspace = true
{backend} = {{ path = "../{backend}" }}
tokio.workspace = true

[[bin]]
name = "{bin_name}"
path = "src/main.rs"
"#,
        package_name = descriptor.bin_crate_package_name(),
        backend = descriptor.backend_crate_name(),
        bin_name = descriptor.bin_crate_package_name(),
    )
}

fn bin_main_rs(descriptor: &ProjectDescriptor) -> String {
    let backend_module = descriptor.backend_crate_name().replace('-', "_");
    format!(
        r#"use std::path::PathBuf;

use anyhow::Result;
use clap::{{Parser, Subcommand}};
use {backend_module} as customer_backend;

#[derive(Debug, Parser)]
#[command(name = "{bin_name}")]
#[command(about = "Customer workspace binary for {display_name}")]
struct Cli {{
    #[arg(long)]
    app_root: Option<PathBuf>,

    #[arg(long, default_value = "platform.dev.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Command,
}}

#[derive(Debug, Subcommand)]
enum Command {{
    Validate,
    Serve {{
        #[arg(long)]
        bind: Option<String>,
    }},
    Up {{
        #[arg(long)]
        bind: Option<String>,
    }},
}}

fn main() -> Result<()> {{
    let cli = Cli::parse();
    let app_root = match cli.app_root {{
        Some(path) => path,
        None => std::env::current_dir()?,
    }};

    match cli.command {{
        Command::Validate => validate(&app_root, &cli.config),
        Command::Serve {{ bind }} => serve(&app_root, &cli.config, bind),
        Command::Up {{ bind }} => {{
            println!("Starting {display_name}");
            serve(&app_root, &cli.config, bind)
        }}
    }}
}}

fn validate(app_root: &std::path::Path, config_path: &std::path::Path) -> Result<()> {{
    let manifest_path = app_root.join("app.toml");
    let config_path = app_root.join(config_path);
    let manifest = coil::app::CustomerAppManifest::from_file(&manifest_path)?;
    let config = coil::config::PlatformConfig::from_file(&config_path)?;

    println!("{display_name} validation passed");
    println!("app root: {{}}", app_root.display());
    println!("config: {{}}", config_path.display());
    println!("app id: {{}}", manifest.id);
    println!(
        "modules: {{}}",
        manifest
            .modules
            .iter()
            .map(|module| module.id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "configured modules: {{}}",
        config.modules.enabled.join(", ")
    );
    Ok(())
}}

fn serve(app_root: &std::path::Path, config_path: &std::path::Path, bind: Option<String>) -> Result<()> {{
    coil::builder()
        .with_customer_plugin(customer_backend::plugin())
        .run_from_paths(app_root, app_root.join(config_path), bind)?;
    Ok(())
}}
"#,
        bin_name = descriptor.bin_crate_package_name(),
        display_name = descriptor.project.display_name,
        backend_module = backend_module,
    )
}

fn backend_cargo_toml(descriptor: &ProjectDescriptor) -> String {
    format!(
        r#"[package]
name = "{name}"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish = false

[dependencies]
coil-customer-sdk.workspace = true
"#,
        name = descriptor.backend_crate_name()
    )
}

fn backend_lib_rs(descriptor: &ProjectDescriptor) -> String {
    let type_name = crate_type_name(&descriptor.backend_crate_name());
    let display_name = &descriptor.project.display_name;
    let plugin_id = descriptor.backend_crate_name();
    format!(
        r#"use coil_customer_sdk::{{BackendError, CustomerBackendPlugin, CustomerHookRegistry, CustomerPluginDescriptor}};

pub fn plugin() -> {type_name} {{
    {type_name}
}}

pub struct {type_name};

impl CustomerBackendPlugin for {type_name} {{
    fn descriptor(&self) -> CustomerPluginDescriptor {{
        CustomerPluginDescriptor::new("{plugin_id}", "{display_name} Backend", "0.1.0")
    }}

    fn register(&self, _registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {{
        Ok(())
    }}
}}
"#,
        type_name = type_name,
        plugin_id = plugin_id,
        display_name = display_name,
    )
}

fn translation_catalog(descriptor: &ProjectDescriptor, locale: &str) -> String {
    let market = market_label(locale);
    format!(
        r#"[nav]
market = "{market_label}"
language = "{language_label}"
current = "{current}"

[home.meta]
title = "{title}"

[home]
eyebrow = "{eyebrow}"
title = "{title}"
summary = "{summary}"
primary_cta = "{primary}"
secondary_cta = "{secondary}"
"#,
        market_label = match locale {
            "fr-FR" => "Marché",
            "pl-PL" => "Rynek",
            _ => "Market",
        },
        language_label = match locale {
            "fr-FR" => "Langue",
            "pl-PL" => "Język",
            _ => "Language",
        },
        current = match locale {
            "fr-FR" => "Actuel",
            "pl-PL" => "Bieżący",
            _ => "Current",
        },
        title = descriptor.project.display_name,
        eyebrow = match locale {
            "fr-FR" => format!("Boutique {market} générée par cargo coil"),
            "pl-PL" => format!("Sklep {market} wygenerowany przez cargo coil"),
            _ => format!("Cargo coil generated {market} storefront"),
        },
        summary = match locale {
            "fr-FR" => "Commencez avec une application cliente Coil, multilingue et prête à évoluer.",
            "pl-PL" => "Zacznij od klientowskiej aplikacji Coil z wieloma językami i miejscem na dalszy rozwój.",
            _ => "Start from a customer-root Coil app with translations, linked Rust, and room to grow.",
        },
        primary = match locale {
            "fr-FR" => "Ouvrir l’administration",
            "pl-PL" => "Otwórz panel administracyjny",
            _ => "Open admin",
        },
        secondary = match locale {
            "fr-FR" => "Ouvrir les outils de développement",
            "pl-PL" => "Otwórz narzędzia deweloperskie",
            _ => "Open dev tools",
        },
    )
}

fn toml_array(values: &[String]) -> String {
    let inner = values
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{inner}]")
}

fn market_label(locale: &str) -> &'static str {
    match locale {
        "fr-FR" => "France",
        "pl-PL" => "Polska",
        "de-DE" => "Deutschland",
        _ => "flagship",
    }
}

fn crate_type_name(name: &str) -> String {
    name.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut value = first.to_uppercase().collect::<String>();
                    value.push_str(chars.as_str());
                    value
                }
                None => String::new(),
            }
        })
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use coil_app::CustomerAppManifest;
    use coil_config::PlatformConfig;
    use tempfile::tempdir;

    #[test]
    fn descriptor_apply_writes_valid_customer_root_files() {
        let workspace = tempdir().unwrap();
        let mut descriptor = ProjectDescriptor::new(
            "acme".to_string(),
            "Acme".to_string(),
            "en-GB".to_string(),
        );
        descriptor.add_locale("fr-FR".to_string(), "acme").unwrap();

        let report = apply_descriptor(workspace.path(), &descriptor).unwrap();

        assert!(report.files_written > 0);
        assert!(workspace.path().join(".coil/project.toml").exists());
        assert!(workspace.path().join("templates/pages/home.html").exists());
        assert!(workspace.path().join("translations/en-GB.toml").exists());
        assert!(workspace.path().join("translations/fr-FR.toml").exists());

        let manifest = CustomerAppManifest::from_file(workspace.path().join("app.toml")).unwrap();
        let config = PlatformConfig::from_file(workspace.path().join("platform.dev.toml")).unwrap();

        assert_eq!(manifest.id.as_str(), "acme");
        assert_eq!(config.app.name, "acme");
    }

    #[test]
    fn doctor_reports_clean_generated_workspace() {
        let workspace = tempdir().unwrap();
        let descriptor = ProjectDescriptor::new(
            "atelier".to_string(),
            "Atelier".to_string(),
            "en-GB".to_string(),
        );
        apply_descriptor(workspace.path(), &descriptor).unwrap();

        let report = crate::doctor(workspace.path()).unwrap();

        assert!(report.issues.is_empty(), "{:?}", report.issues);
    }

    #[test]
    fn platform_config_includes_canonical_host_in_site_hosts() {
        let workspace = tempdir().unwrap();
        let mut descriptor = ProjectDescriptor::new(
            "shop".to_string(),
            "Shop".to_string(),
            "en-GB".to_string(),
        );
        descriptor
            .add_site(SiteDescriptor {
                id: "shop-fr".to_string(),
                display_name: "Shop France".to_string(),
                brand_name: "Shop".to_string(),
                canonical_domain: "shop-fr.localhost".to_string(),
                additional_domains: Vec::new(),
                default_locale: "fr-FR".to_string(),
                supported_locales: vec!["fr-FR".to_string()],
            })
            .unwrap();

        apply_descriptor(workspace.path(), &descriptor).unwrap();

        let config = std::fs::read_to_string(workspace.path().join("platform.dev.toml")).unwrap();
        assert!(config.contains("canonical_host = \"shop-fr.localhost\""));
        assert!(config.contains("hosts = [\"shop-fr.localhost\"]"));
    }
}
