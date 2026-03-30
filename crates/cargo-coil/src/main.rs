use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use coil_scaffold::{
    DEFAULT_FRAMEWORK_VERSION, DependencySource, LocaleAddOptions, ModuleEditAction,
    ProjectDescriptor, SiteAddOptions, add_locale, add_site, apply_descriptor, create_project,
    doctor, load_descriptor, modify_modules, run_wizard, sanitize_slug,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand};
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

#[derive(Debug, Parser)]
#[command(name = "cargo-coil")]
#[command(bin_name = "cargo coil")]
#[command(about = "Create and evolve Coil customer projects")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    New(NewCommand),
    Init(InitCommand),
    Dev(DevCommand),
    Apply {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Doctor {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Module {
        #[command(subcommand)]
        command: ModuleCommand,
    },
    Site {
        #[command(subcommand)]
        command: SiteCommand,
    },
    Locale {
        #[command(subcommand)]
        command: LocaleCommand,
    },
}

#[derive(Debug, Args)]
struct NewCommand {
    path: PathBuf,

    #[arg(long = "non-interactive", alias = "no-input")]
    non_interactive: bool,

    #[arg(long)]
    name: Option<String>,

    #[arg(long)]
    display_name: Option<String>,

    #[arg(long, default_value = "en-GB")]
    default_locale: String,

    #[arg(long = "locale")]
    locales: Vec<String>,

    #[arg(long = "module")]
    modules: Vec<String>,

    #[arg(long, value_enum)]
    source: Option<DependencySourceArg>,

    #[arg(long)]
    coil_path: Option<PathBuf>,

    #[arg(
        long,
        help = "Framework version to generate against. Use an explicit version or `latest`."
    )]
    framework_version: Option<String>,
}

#[derive(Debug, Args)]
struct InitCommand {
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[arg(long = "non-interactive", alias = "no-input")]
    non_interactive: bool,

    #[arg(long)]
    name: Option<String>,

    #[arg(long)]
    display_name: Option<String>,

    #[arg(long, default_value = "en-GB")]
    default_locale: String,

    #[arg(long = "locale")]
    locales: Vec<String>,

    #[arg(long = "module")]
    modules: Vec<String>,

    #[arg(long, value_enum)]
    source: Option<DependencySourceArg>,

    #[arg(long)]
    coil_path: Option<PathBuf>,

    #[arg(
        long,
        help = "Framework version to generate against. Use an explicit version or `latest`."
    )]
    framework_version: Option<String>,
}

#[derive(Debug, Args)]
struct DevCommand {
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[arg(long, default_value = "platform.dev.toml")]
    config: PathBuf,

    #[arg(long)]
    bind: Option<String>,

    #[arg(long)]
    no_watch: bool,

    #[arg(long)]
    skip_infra: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum DependencySourceArg {
    CratesIo,
    Path,
}

#[derive(Debug, Subcommand)]
enum ModuleCommand {
    Add {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        modules: Vec<String>,
    },
    Remove {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        modules: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum SiteCommand {
    Add {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        id: String,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        brand_name: Option<String>,
        #[arg(long)]
        canonical_domain: Option<String>,
        #[arg(long = "domain")]
        additional_domains: Vec<String>,
        #[arg(long, default_value = "en-GB")]
        default_locale: String,
    },
}

#[derive(Debug, Subcommand)]
enum LocaleCommand {
    Add {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        locale: String,
        #[arg(long)]
        site: Option<String>,
        #[arg(long)]
        default_for_site: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse_from(normalized_args());
    match cli.command {
        Command::New(command) => new_project(command),
        Command::Init(command) => init_project(command),
        Command::Dev(command) => dev_project(command),
        Command::Apply { root } => apply(root),
        Command::Doctor { root } => doctor_command(root),
        Command::Module { command } => module_command(command),
        Command::Site { command } => site_command(command),
        Command::Locale { command } => locale_command(command),
    }
}

fn normalized_args() -> Vec<String> {
    let mut args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("coil") {
        args.remove(1);
    }
    args
}

fn new_project(command: NewCommand) -> Result<()> {
    let framework_version = resolve_framework_version(command.framework_version)?;
    let root = command.path;
    let descriptor = if command.non_interactive {
        descriptor_from_noninteractive(
            command.name,
            command.display_name,
            command.default_locale,
            command.locales,
            command.modules,
            command.source,
            command.coil_path,
            framework_version.clone(),
            &root,
        )?
    } else {
        let wizard = run_wizard(&root)?;
        descriptor_from_wizard(
            wizard,
            command.source,
            command.coil_path,
            framework_version.clone(),
        )?
    };
    let report = create_project(&root, &descriptor)?;
    println!("Created Coil project at {}", report.root.display());
    println!("Wrote {} managed files", report.files_written);
    println!(
        "Next: `cd {}` and run `docker compose up --build`",
        root.display()
    );
    Ok(())
}

fn init_project(command: InitCommand) -> Result<()> {
    let framework_version = resolve_framework_version(command.framework_version)?;
    let root = command.root;
    let descriptor = if command.non_interactive {
        descriptor_from_noninteractive(
            command.name,
            command.display_name,
            command.default_locale,
            command.locales,
            command.modules,
            command.source,
            command.coil_path,
            framework_version.clone(),
            &root,
        )?
    } else {
        let wizard = run_wizard(&root)?;
        descriptor_from_wizard(
            wizard,
            command.source,
            command.coil_path,
            framework_version.clone(),
        )?
    };
    let report = apply_descriptor(&root, &descriptor)?;
    println!("Initialised Coil project at {}", report.root.display());
    println!("Wrote {} managed files", report.files_written);
    Ok(())
}

fn dev_project(command: DevCommand) -> Result<()> {
    let root = command.root.canonicalize().with_context(|| {
        format!(
            "failed to resolve project root `{}`",
            command.root.display()
        )
    })?;
    let descriptor = load_descriptor(&root)?;

    if !command.skip_infra {
        println!("Starting Postgres and Redis with Docker Compose");
        run_docker_compose_up(&root)?;
    }

    let env = dev_environment(&root, &descriptor);
    let cargo_program = cargo_program();
    let mut app_args = vec![
        OsString::from("run"),
        OsString::from("-p"),
        OsString::from(descriptor.bin_crate_package_name()),
        OsString::from("--"),
        OsString::from("--config"),
        command.config.into_os_string(),
        OsString::from("up"),
    ];
    if let Some(bind) = command.bind {
        app_args.push(OsString::from("--bind"));
        app_args.push(OsString::from(bind));
    }

    if command.no_watch {
        println!(
            "Running `{}` without file watching",
            descriptor.bin_crate_package_name()
        );
        run_process(&cargo_program, &app_args, &root, &env)
    } else {
        println!(
            "Watching the workspace and restarting `{}` on change",
            descriptor.bin_crate_package_name()
        );
        watch_process(&cargo_program, &app_args, &root, &env)
    }
}

fn apply(root: PathBuf) -> Result<()> {
    let descriptor = load_descriptor(&root)?;
    let report = apply_descriptor(&root, &descriptor)?;
    println!("Applied descriptor at {}", report.root.display());
    println!("Wrote {} managed files", report.files_written);
    Ok(())
}

fn doctor_command(root: PathBuf) -> Result<()> {
    let report = doctor(&root)?;
    if report.issues.is_empty() {
        println!("Coil project is aligned with its descriptor");
        println!("root: {}", report.root.display());
        return Ok(());
    }
    println!("Coil project has {} issue(s)", report.issues.len());
    println!("root: {}", report.root.display());
    for issue in report.issues {
        println!("- {}: {}", issue.path.display(), issue.message);
    }
    bail!("descriptor drift detected")
}

fn module_command(command: ModuleCommand) -> Result<()> {
    match command {
        ModuleCommand::Add { root, modules } => {
            if modules.is_empty() {
                bail!("specify at least one module to add");
            }
            let report = modify_modules(root, ModuleEditAction::Add, &modules)?;
            println!("Added module(s): {}", modules.join(", "));
            println!("Wrote {} managed files", report.files_written);
        }
        ModuleCommand::Remove { root, modules } => {
            if modules.is_empty() {
                bail!("specify at least one module to remove");
            }
            let report = modify_modules(root, ModuleEditAction::Remove, &modules)?;
            println!("Removed module(s): {}", modules.join(", "));
            println!("Wrote {} managed files", report.files_written);
        }
    }
    Ok(())
}

fn site_command(command: SiteCommand) -> Result<()> {
    match command {
        SiteCommand::Add {
            root,
            id,
            display_name,
            brand_name,
            canonical_domain,
            additional_domains,
            default_locale,
        } => {
            let id = sanitize_slug(&id);
            if id.is_empty() {
                bail!("site id must not be empty");
            }
            let report = add_site(
                root,
                SiteAddOptions {
                    display_name: display_name.unwrap_or_else(|| title_case(&id)),
                    brand_name: brand_name.unwrap_or_else(|| title_case(&id)),
                    canonical_domain: canonical_domain.unwrap_or_else(|| format!("{id}.localhost")),
                    additional_domains,
                    default_locale,
                    id,
                },
            )?;
            println!("Added site");
            println!("Wrote {} managed files", report.files_written);
        }
    }
    Ok(())
}

fn locale_command(command: LocaleCommand) -> Result<()> {
    match command {
        LocaleCommand::Add {
            root,
            locale,
            site,
            default_for_site,
        } => {
            let descriptor = load_descriptor(&root)?;
            let site_id = site.unwrap_or_else(|| descriptor.default_site().id.clone());
            let report = add_locale(
                root,
                LocaleAddOptions {
                    site_id,
                    locale,
                    make_default: default_for_site,
                },
            )?;
            println!("Added locale");
            println!("Wrote {} managed files", report.files_written);
        }
    }
    Ok(())
}

fn descriptor_from_noninteractive(
    name: Option<String>,
    display_name: Option<String>,
    default_locale: String,
    mut locales: Vec<String>,
    modules: Vec<String>,
    source: Option<DependencySourceArg>,
    coil_path: Option<PathBuf>,
    framework_version: String,
    root: &Path,
) -> Result<ProjectDescriptor> {
    let path_name = root
        .file_name()
        .and_then(|segment| segment.to_str())
        .unwrap_or("coil-store");
    let name = sanitize_slug(name.as_deref().unwrap_or(path_name));
    if name.is_empty() {
        bail!("project name must not be empty");
    }
    let display_name = display_name.unwrap_or_else(|| title_case(&name));
    if !locales.contains(&default_locale) {
        locales.insert(0, default_locale.clone());
    }
    let mut descriptor = ProjectDescriptor::new(name, display_name, default_locale);
    if !modules.is_empty() {
        descriptor.modules.enabled = modules;
    }
    descriptor.tooling.framework_version = framework_version;
    descriptor.i18n.supported_locales = dedup(locales);
    descriptor.sites[0].supported_locales = descriptor.i18n.supported_locales.clone();
    descriptor.tooling.dependency_source = resolve_dependency_source(source, coil_path)?;
    descriptor.validate()?;
    Ok(descriptor)
}

fn descriptor_from_wizard(
    wizard: coil_scaffold::WizardInput,
    source: Option<DependencySourceArg>,
    coil_path: Option<PathBuf>,
    framework_version: String,
) -> Result<ProjectDescriptor> {
    let mut descriptor =
        ProjectDescriptor::new(wizard.name, wizard.display_name, wizard.default_locale);
    descriptor.modules.enabled = wizard.modules;
    descriptor.tooling.framework_version = framework_version;
    descriptor.i18n.supported_locales = dedup(wizard.supported_locales);
    descriptor.sites[0].supported_locales = descriptor.i18n.supported_locales.clone();
    descriptor.tooling.dependency_source = resolve_dependency_source(source, coil_path)?;
    for site in wizard.extra_sites {
        descriptor.add_site(site)?;
    }
    descriptor.validate()?;
    Ok(descriptor)
}

fn dedup(values: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

fn title_case(input: &str) -> String {
    input
        .split('-')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => {
                    let mut value = first.to_uppercase().collect::<String>();
                    value.push_str(chars.as_str());
                    value
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn resolve_dependency_source(
    source: Option<DependencySourceArg>,
    coil_path: Option<PathBuf>,
) -> Result<DependencySource> {
    match source {
        Some(DependencySourceArg::CratesIo) => Ok(DependencySource::CratesIo),
        Some(DependencySourceArg::Path) => {
            let repo_root = match coil_path {
                Some(path) => path,
                None => detect_local_coil_repo()?.ok_or_else(|| {
                    anyhow::anyhow!(
                        "could not infer a local Coil checkout; pass `--coil-path /path/to/coil`"
                    )
                })?,
            };
            Ok(DependencySource::Path {
                repo_root: repo_root.display().to_string(),
            })
        }
        None => Ok(DependencySource::CratesIo),
    }
}

fn detect_local_coil_repo() -> Result<Option<PathBuf>> {
    let current_dir = std::env::current_dir()?;
    Ok(find_coil_repo_root(&current_dir))
}

fn find_coil_repo_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        let cargo = candidate.join("Cargo.toml");
        let coil = candidate.join("crates/coil/Cargo.toml");
        let sdk = candidate.join("crates/coil-customer-sdk/Cargo.toml");
        if cargo.exists() && coil.exists() && sdk.exists() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

fn docker_program() -> OsString {
    std::env::var_os("COIL_DEV_DOCKER_BIN").unwrap_or_else(|| OsString::from("docker"))
}

fn cargo_program() -> OsString {
    std::env::var_os("COIL_DEV_CARGO_BIN").unwrap_or_else(|| OsString::from("cargo"))
}

fn run_docker_compose_up(root: &Path) -> Result<()> {
    let mut args = vec![
        OsString::from("compose"),
        OsString::from("up"),
        OsString::from("-d"),
        OsString::from("postgres"),
        OsString::from("redis"),
    ];
    let services = read_compose_services(root);
    for service in ["minio", "minio-init"] {
        if services.iter().any(|candidate| candidate == service) {
            args.push(OsString::from(service));
        }
    }
    run_process(&docker_program(), &args, root, &[]).context("failed to start local infra")?;
    Ok(())
}

fn dev_environment(root: &Path, descriptor: &ProjectDescriptor) -> Vec<(String, String)> {
    let slug = descriptor.project_slug();
    let compose = read_compose(root);
    let compose_env = compose
        .as_ref()
        .and_then(compose_app_environment)
        .unwrap_or_default();
    let postgres_host_port = compose
        .as_ref()
        .and_then(|file| compose_host_port(file, "postgres", 5432))
        .unwrap_or(15432);
    let redis_host_port = compose
        .as_ref()
        .and_then(|file| compose_host_port(file, "redis", 6379))
        .unwrap_or(16379);
    let minio_host_port = compose
        .as_ref()
        .and_then(|file| compose_host_port(file, "minio", 9000))
        .unwrap_or(9000);

    let mut names = BTreeSet::from([
        "DATABASE_URL".to_string(),
        "REDIS_URL".to_string(),
        "OBJECT_STORE_URL".to_string(),
        "COIL_COOKIE_SECRET".to_string(),
        "COIL_CSRF_SECRET".to_string(),
    ]);
    names.extend(compose_env.keys().cloned());

    let mut env = Vec::new();
    for name in names {
        let value = env_var_or_default(&name, || {
            if let Some(value) = compose_env.get(&name) {
                return localize_compose_env_value(
                    &name,
                    value,
                    &slug,
                    postgres_host_port,
                    redis_host_port,
                    minio_host_port,
                )
                .unwrap_or_else(|| fallback_dev_env_value(&name, &slug));
            }
            fallback_dev_env_value(&name, &slug)
        });
        env.push((name, value));
    }
    env
}

fn env_var_or_default(key: &str, default: impl FnOnce() -> String) -> String {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => default(),
    }
}

fn default_object_store_url(slug: &str) -> String {
    format!(
        "endpoint_url=\"http://127.0.0.1:9000\"\nbucket=\"{slug}\"\nregion=\"us-east-1\"\naccess_key_id=\"minio\"\nsecret_access_key=\"minio123\""
    )
}

fn fallback_dev_env_value(name: &str, slug: &str) -> String {
    match name {
        "DATABASE_URL" => format!("postgres://coil:coil@127.0.0.1:15432/{slug}"),
        "REDIS_URL" => "redis://127.0.0.1:16379/0".to_string(),
        "COIL_COOKIE_SECRET" => "local-development-cookie-secret".to_string(),
        "COIL_CSRF_SECRET" => "local-development-csrf-secret".to_string(),
        "OBJECT_STORE_URL" => default_object_store_url(slug),
        "STRIPE_PUBLISHABLE_KEY" => "pk_test_replace_me".to_string(),
        "STRIPE_SECRET_KEY" => "sk_test_replace_me".to_string(),
        "STRIPE_WEBHOOK_SECRET" => "whsec_replace_me".to_string(),
        _ => format!(
            "local-development-{}",
            name.to_ascii_lowercase().replace('_', "-")
        ),
    }
}

#[derive(Debug, Deserialize)]
struct ComposeFile {
    #[serde(default)]
    services: BTreeMap<String, ComposeService>,
}

#[derive(Debug, Deserialize)]
struct ComposeService {
    #[serde(default)]
    environment: Option<serde_yaml::Value>,
    #[serde(default)]
    ports: Vec<String>,
}

fn read_compose(root: &Path) -> Option<ComposeFile> {
    let compose_path = root.join("docker-compose.yml");
    let contents = fs::read_to_string(compose_path).ok()?;
    serde_yaml::from_str(&contents).ok()
}

fn read_compose_services(root: &Path) -> Vec<String> {
    read_compose(root)
        .map(|file| file.services.into_keys().collect())
        .unwrap_or_default()
}

fn compose_app_environment(compose: &ComposeFile) -> Option<BTreeMap<String, String>> {
    let service = compose.services.get("app")?;
    let serde_yaml::Value::Mapping(mapping) = service.environment.as_ref()? else {
        return None;
    };

    let mut env = BTreeMap::new();
    for (key, value) in mapping {
        let Some(key) = key.as_str() else {
            continue;
        };
        let value = match value {
            serde_yaml::Value::String(value) => value.clone(),
            other => serde_yaml::to_string(other).ok()?.trim().to_string(),
        };
        env.insert(key.to_string(), value);
    }
    Some(env)
}

fn compose_host_port(
    compose: &ComposeFile,
    service_name: &str,
    container_port: u16,
) -> Option<u16> {
    let service = compose.services.get(service_name)?;
    service
        .ports
        .iter()
        .find_map(|entry| parse_host_port(entry, container_port))
}

fn parse_host_port(entry: &str, container_port: u16) -> Option<u16> {
    let entry = entry.trim().trim_matches('"').trim_matches('\'');
    let mut parts = entry.rsplitn(2, ':');
    let container = parts.next()?.trim();
    let host = parts.next()?.trim();
    let container = container.parse::<u16>().ok()?;
    if container != container_port {
        return None;
    }
    resolve_compose_scalar(host)?.parse::<u16>().ok()
}

fn resolve_compose_scalar(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let trimmed = if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        &trimmed[1..trimmed.len().saturating_sub(1)]
    } else {
        trimmed
    };
    if let Some(inner) = trimmed
        .strip_prefix("${")
        .and_then(|rest| rest.strip_suffix('}'))
    {
        if let Some((_, default)) = inner.split_once(":-") {
            return Some(default.to_string());
        }
        return std::env::var(inner)
            .ok()
            .filter(|value| !value.trim().is_empty());
    }
    Some(trimmed.to_string())
}

fn localize_compose_env_value(
    name: &str,
    value: &str,
    slug: &str,
    postgres_host_port: u16,
    redis_host_port: u16,
    minio_host_port: u16,
) -> Option<String> {
    match name {
        "DATABASE_URL" => Some(localize_database_url(value, postgres_host_port)),
        "REDIS_URL" => Some(localize_redis_url(value, redis_host_port)),
        "OBJECT_STORE_URL" => Some(localize_object_store_url(value, minio_host_port)),
        _ => resolve_compose_scalar(value).or_else(|| Some(fallback_dev_env_value(name, slug))),
    }
}

fn localize_database_url(value: &str, host_port: u16) -> String {
    localize_network_url(value, host_port)
}

fn localize_redis_url(value: &str, host_port: u16) -> String {
    localize_network_url(value, host_port)
}

fn localize_network_url(value: &str, host_port: u16) -> String {
    let resolved = resolve_compose_scalar(value).unwrap_or_else(|| value.to_string());
    let Some((scheme, rest)) = resolved.split_once("://") else {
        return resolved;
    };
    let (authority, suffix) = if let Some((authority, tail)) = rest.split_once('/') {
        (authority, format!("/{tail}"))
    } else {
        (rest, String::new())
    };
    let prefix = authority
        .split_once('@')
        .map(|(credentials, _)| format!("{credentials}@"))
        .unwrap_or_default();
    format!("{scheme}://{prefix}127.0.0.1:{host_port}{suffix}")
}

fn localize_object_store_url(value: &str, host_port: u16) -> String {
    let mut output = Vec::new();
    for line in value.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("endpoint_url=") {
            output.push(format!("endpoint_url=\"http://127.0.0.1:{host_port}\""));
        } else {
            output.push(resolve_compose_scalar(trimmed).unwrap_or_else(|| trimmed.to_string()));
        }
    }
    output.join("\n")
}

fn run_process(
    program: &OsString,
    args: &[OsString],
    root: &Path,
    env: &[(String, String)],
) -> Result<()> {
    let mut command = ProcessCommand::new(program);
    command.args(args).current_dir(root);
    for (key, value) in env {
        command.env(key, value);
    }
    let status = command.status().with_context(|| {
        format!(
            "failed to run `{}`",
            std::iter::once(program.to_string_lossy().into_owned())
                .chain(args.iter().map(|arg| arg.to_string_lossy().into_owned()))
                .collect::<Vec<_>>()
                .join(" ")
        )
    })?;
    if status.success() {
        Ok(())
    } else {
        bail!(
            "`{}` exited with status {}",
            std::iter::once(program.to_string_lossy().into_owned())
                .chain(args.iter().map(|arg| arg.to_string_lossy().into_owned()))
                .collect::<Vec<_>>()
                .join(" "),
            status
        )
    }
}

fn watch_process(
    program: &OsString,
    args: &[OsString],
    root: &Path,
    env: &[(String, String)],
) -> Result<()> {
    let mut snapshot = watch_snapshot(root)?;
    let mut child = spawn_process(program, args, root, env)?;
    let mut child_exited = false;
    loop {
        thread::sleep(Duration::from_millis(500));
        let current = watch_snapshot(root)?;
        if current != snapshot {
            snapshot = current;
            if child.try_wait()?.is_none() {
                child.kill().ok();
                let _ = child.wait();
            }
            println!("Change detected. Restarting app process.");
            child = spawn_process(program, args, root, env)?;
            child_exited = false;
            continue;
        }

        if !child_exited {
            if let Some(status) = child.try_wait()? {
                child_exited = true;
                if !status.success() {
                    eprintln!(
                        "App process exited with status {}. Waiting for changes to restart.",
                        status
                    );
                }
            }
        }
    }
}

fn spawn_process(
    program: &OsString,
    args: &[OsString],
    root: &Path,
    env: &[(String, String)],
) -> Result<Child> {
    let mut command = ProcessCommand::new(program);
    command.args(args).current_dir(root);
    for (key, value) in env {
        command.env(key, value);
    }
    command.spawn().with_context(|| {
        format!(
            "failed to run `{}`",
            std::iter::once(program.to_string_lossy().into_owned())
                .chain(args.iter().map(|arg| arg.to_string_lossy().into_owned()))
                .collect::<Vec<_>>()
                .join(" ")
        )
    })
}

fn watch_snapshot(root: &Path) -> Result<BTreeMap<PathBuf, u128>> {
    let mut snapshot = BTreeMap::new();
    collect_watch_snapshot(root, root, &mut snapshot)?;
    Ok(snapshot)
}

fn collect_watch_snapshot(
    root: &Path,
    current: &Path,
    snapshot: &mut BTreeMap<PathBuf, u128>,
) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("failed to read watch directory `{}`", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        if should_ignore_watch_path(relative) {
            continue;
        }
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            collect_watch_snapshot(root, &path, snapshot)?;
            continue;
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        snapshot.insert(relative.to_path_buf(), modified);
    }
    Ok(())
}

fn should_ignore_watch_path(relative: &Path) -> bool {
    let mut components = relative.components();
    let Some(first) = components.next() else {
        return false;
    };
    let first = first.as_os_str();
    if first == ".git"
        || first == "target"
        || first == "node_modules"
        || first == ".coil"
        || first == ".cargo"
    {
        return true;
    }

    if relative.starts_with(Path::new("theme/assets")) {
        return true;
    }

    if relative.starts_with(Path::new("extensions"))
        && relative
            .extension()
            .is_some_and(|extension| extension == "wasm")
    {
        return true;
    }

    false
}

fn resolve_framework_version(requested: Option<String>) -> Result<String> {
    resolve_framework_version_with(
        requested.as_deref(),
        fetch_latest_published_framework_version,
    )
}

fn resolve_framework_version_with<F>(requested: Option<&str>, fetch_latest: F) -> Result<String>
where
    F: FnOnce() -> Result<String>,
{
    match requested.map(str::trim).filter(|value| !value.is_empty()) {
        Some("latest") => fetch_latest(),
        Some(version) => Ok(version.to_string()),
        None => match fetch_latest() {
            Ok(version) => Ok(version),
            Err(error) => {
                eprintln!(
                    "info: unable to resolve the latest published Coil framework version ({error:#}); using built-in default {}",
                    DEFAULT_FRAMEWORK_VERSION
                );
                Ok(DEFAULT_FRAMEWORK_VERSION.to_string())
            }
        },
    }
}

fn fetch_latest_published_framework_version() -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent(format!("cargo-coil/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to construct crates.io client")?;

    let version = fetch_max_stable_version(&client, "coil-rs")?;
    ensure_version_exists(&client, "coil-customer-sdk", &version)?;
    Ok(version)
}

fn fetch_max_stable_version(client: &reqwest::blocking::Client, package: &str) -> Result<String> {
    let payload: CratesIoCrateResponse = client
        .get(format!("https://crates.io/api/v1/crates/{package}"))
        .send()
        .with_context(|| format!("failed to query crates.io for `{package}`"))?
        .error_for_status()
        .with_context(|| format!("crates.io returned an error for `{package}`"))?
        .json()
        .with_context(|| format!("failed to parse crates.io response for `{package}`"))?;

    let version = payload.krate.max_stable_version.trim();
    if version.is_empty() {
        bail!("crates.io did not report a stable version for `{package}`");
    }
    Ok(version.to_string())
}

fn ensure_version_exists(
    client: &reqwest::blocking::Client,
    package: &str,
    version: &str,
) -> Result<()> {
    client
        .get(format!(
            "https://crates.io/api/v1/crates/{package}/{version}"
        ))
        .send()
        .with_context(|| format!("failed to verify `{package}` v{version} on crates.io"))?
        .error_for_status()
        .with_context(|| format!("`{package}` v{version} is not published on crates.io"))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CratesIoCrateResponse {
    #[serde(rename = "crate")]
    krate: CratesIoCrateMeta,
}

#[derive(Debug, Deserialize)]
struct CratesIoCrateMeta {
    max_stable_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_framework_version_wins() {
        let version =
            resolve_framework_version_with(Some("0.2.7"), || Ok("9.9.9".to_string())).unwrap();
        assert_eq!(version, "0.2.7");
    }

    #[test]
    fn latest_uses_live_lookup() {
        let version =
            resolve_framework_version_with(Some("latest"), || Ok("0.3.1".to_string())).unwrap();
        assert_eq!(version, "0.3.1");
    }

    #[test]
    fn default_falls_back_to_built_in_version() {
        let version =
            resolve_framework_version_with(None, || bail!("network unavailable")).unwrap();
        assert_eq!(version, DEFAULT_FRAMEWORK_VERSION);
    }

    #[test]
    fn watch_ignores_runtime_state_directories() {
        assert!(should_ignore_watch_path(Path::new(".coil")));
        assert!(should_ignore_watch_path(Path::new(
            ".coil/cache/state.json"
        )));
        assert!(should_ignore_watch_path(Path::new(
            ".coil/metadata/app.sqlite3"
        )));
        assert!(should_ignore_watch_path(Path::new("target/debug/app")));
        assert!(should_ignore_watch_path(Path::new("theme/assets/site.js")));
        assert!(should_ignore_watch_path(Path::new(
            "extensions/example/example.wasm"
        )));
        assert!(should_ignore_watch_path(Path::new(".cargo/config.toml")));
        assert!(!should_ignore_watch_path(Path::new(
            "templates/pages/home.html"
        )));
        assert!(!should_ignore_watch_path(Path::new(
            "theme/frontend/site.ts"
        )));
        assert!(!should_ignore_watch_path(Path::new(
            "extensions/example/package.toml"
        )));
    }

    #[test]
    fn dev_environment_defaults_object_store_url() {
        let dir = tempfile::tempdir().unwrap();
        let descriptor = ProjectDescriptor::new(
            "my-store".to_string(),
            "My Store".to_string(),
            "en-GB".to_string(),
        );
        let env = dev_environment(dir.path(), &descriptor);
        let object_store = env
            .iter()
            .find(|(key, _)| key == "OBJECT_STORE_URL")
            .map(|(_, value)| value.as_str())
            .unwrap();
        assert!(object_store.contains("endpoint_url=\"http://127.0.0.1:9000\""));
        assert!(object_store.contains("bucket=\"my-store\""));
    }

    #[test]
    fn read_compose_services_extracts_top_level_service_names() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("docker-compose.yml"),
            "services:\n  postgres:\n    image: postgres:16\n  redis:\n    image: redis:7\n  minio:\n    image: minio/minio:latest\nvolumes:\n  data:\n",
        )
        .unwrap();
        assert_eq!(
            read_compose_services(dir.path()),
            vec![
                "minio".to_string(),
                "postgres".to_string(),
                "redis".to_string(),
            ]
        );
    }

    #[test]
    fn dev_environment_uses_localized_compose_defaults() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("docker-compose.yml"),
            r#"
services:
  app:
    environment:
      DATABASE_URL: postgres://coil:devpass@postgres:5432/coil_shoppr
      REDIS_URL: redis://redis:6379
      OBJECT_STORE_URL: |-
        endpoint_url="http://minio:9000"
        bucket="shoppr"
        region="us-east-1"
        access_key_id="minio"
        secret_access_key="minio123"
      STRIPE_PUBLISHABLE_KEY: "${STRIPE_PUBLISHABLE_KEY:-pk_test_replace_me}"
      STRIPE_SECRET_KEY: "${STRIPE_SECRET_KEY:-sk_test_replace_me}"
      STRIPE_WEBHOOK_SECRET: "${STRIPE_WEBHOOK_SECRET:-whsec_replace_me}"
  postgres:
    ports:
      - "15432:5432"
  redis:
    ports:
      - "16379:6379"
  minio:
    ports:
      - "9000:9000"
"#,
        )
        .unwrap();
        let descriptor = ProjectDescriptor::new(
            "shoppr".to_string(),
            "Shoppr".to_string(),
            "en-GB".to_string(),
        );
        let env = dev_environment(dir.path(), &descriptor)
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            env.get("DATABASE_URL").map(String::as_str),
            Some("postgres://coil:devpass@127.0.0.1:15432/coil_shoppr")
        );
        assert_eq!(
            env.get("REDIS_URL").map(String::as_str),
            Some("redis://127.0.0.1:16379")
        );
        assert_eq!(
            env.get("STRIPE_SECRET_KEY").map(String::as_str),
            Some("sk_test_replace_me")
        );
        assert!(
            env.get("OBJECT_STORE_URL")
                .is_some_and(|value| value.contains("endpoint_url=\"http://127.0.0.1:9000\""))
        );
        assert!(
            env.get("OBJECT_STORE_URL")
                .is_some_and(|value| value.contains("bucket=\"shoppr\""))
        );
    }
}
