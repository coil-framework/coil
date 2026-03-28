#![forbid(unsafe_code)]

mod apply;
mod descriptor;
mod doctor;
mod wizard;

pub use apply::{
    ApplyReport, LocaleAddOptions, ModuleEditAction, ProjectLocation, RenderedProjectFile,
    SiteAddOptions, add_locale, add_site, apply_descriptor, build_descriptor, create_project,
    descriptor_path, load_descriptor, modify_modules, save_descriptor,
};
pub use descriptor::{DependencySource, ProjectDescriptor, ProjectProduct, SiteDescriptor};
pub use doctor::{DoctorIssue, DoctorReport, doctor};
pub use wizard::{WizardInput, run_wizard, sanitize_slug};
