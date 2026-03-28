use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand};
use coil_scaffold::{
    DependencySource, LocaleAddOptions, ModuleEditAction, ProjectDescriptor, SiteAddOptions,
    add_locale, add_site, apply_descriptor, create_project, doctor, load_descriptor,
    modify_modules, run_wizard, sanitize_slug,
};
use std::path::{Path, PathBuf};

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

    #[arg(long)]
    no_input: bool,

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
}

#[derive(Debug, Args)]
struct InitCommand {
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[arg(long)]
    no_input: bool,

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
    let cli = Cli::parse();
    match cli.command {
        Command::New(command) => new_project(command),
        Command::Init(command) => init_project(command),
        Command::Apply { root } => apply(root),
        Command::Doctor { root } => doctor_command(root),
        Command::Module { command } => module_command(command),
        Command::Site { command } => site_command(command),
        Command::Locale { command } => locale_command(command),
    }
}

fn new_project(command: NewCommand) -> Result<()> {
    let root = command.path;
    let descriptor = if command.no_input {
        descriptor_from_noninteractive(
            command.name,
            command.display_name,
            command.default_locale,
            command.locales,
            command.modules,
            command.source,
            command.coil_path,
            &root,
        )?
    } else {
        let wizard = run_wizard(&root)?;
        descriptor_from_wizard(wizard, command.source, command.coil_path)?
    };
    let report = create_project(&root, &descriptor)?;
    println!("Created Coil project at {}", report.root.display());
    println!("Wrote {} managed files", report.files_written);
    println!("Next: `cd {}` and run `cargo run -p {} -- validate`", root.display(), descriptor.bin_crate_package_name());
    Ok(())
}

fn init_project(command: InitCommand) -> Result<()> {
    let root = command.root;
    let descriptor = if command.no_input {
        descriptor_from_noninteractive(
            command.name,
            command.display_name,
            command.default_locale,
            command.locales,
            command.modules,
            command.source,
            command.coil_path,
            &root,
        )?
    } else {
        let wizard = run_wizard(&root)?;
        descriptor_from_wizard(wizard, command.source, command.coil_path)?
    };
    let report = apply_descriptor(&root, &descriptor)?;
    println!("Initialised Coil project at {}", report.root.display());
    println!("Wrote {} managed files", report.files_written);
    Ok(())
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
) -> Result<ProjectDescriptor> {
    let mut descriptor = ProjectDescriptor::new(
        wizard.name,
        wizard.display_name,
        wizard.default_locale,
    );
    descriptor.modules.enabled = wizard.modules;
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
        None => {
            if let Some(repo_root) = coil_path.or(detect_local_coil_repo()?) {
                Ok(DependencySource::Path {
                    repo_root: repo_root.display().to_string(),
                })
            } else {
                Ok(DependencySource::CratesIo)
            }
        }
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
