use crate::{RenderedProjectFile, build_descriptor, load_descriptor};
use anyhow::Result;
use coil_app::CustomerAppManifest;
use coil_config::PlatformConfig;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorIssue {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub root: PathBuf,
    pub issues: Vec<DoctorIssue>,
}

pub fn doctor(root: impl AsRef<Path>) -> Result<DoctorReport> {
    let root = root.as_ref();
    let descriptor = load_descriptor(root)?;
    let expected = build_descriptor(&descriptor)?;
    let mut issues = Vec::new();

    for file in expected {
        check_file(root, &file, &mut issues);
    }

    if let Err(error) = CustomerAppManifest::from_file(root.join("app.toml")) {
        issues.push(DoctorIssue {
            path: root.join("app.toml"),
            message: error.to_string(),
        });
    }

    if let Err(error) = PlatformConfig::from_file(root.join("platform.dev.toml")) {
        issues.push(DoctorIssue {
            path: root.join("platform.dev.toml"),
            message: error.to_string(),
        });
    }

    Ok(DoctorReport {
        root: root.to_path_buf(),
        issues,
    })
}

fn check_file(root: &Path, expected: &RenderedProjectFile, issues: &mut Vec<DoctorIssue>) {
    let path = root.join(&expected.path);
    match fs::read_to_string(&path) {
        Ok(contents) => {
            if contents != expected.contents {
                issues.push(DoctorIssue {
                    path,
                    message:
                        "file differs from the current descriptor output; re-run `cargo coil apply`"
                            .to_string(),
                });
            }
        }
        Err(_) => {
            issues.push(DoctorIssue {
                path,
                message: "file is missing".to_string(),
            });
        }
    }
}
