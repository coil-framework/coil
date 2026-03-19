use super::*;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapabilityValidationError {
    #[error(
        "module `{module}` requires capability `{capability}` but the active auth package does not bind it"
    )]
    MissingCapability {
        module: String,
        capability: Capability,
    },
    #[error("module `{module}` does not declare a capability contract for `{capability}`")]
    MissingCapabilityContract {
        module: String,
        capability: Capability,
    },
    #[error(
        "module `{module}` declares capability `{capability}` as {actual} but {expected} was required"
    )]
    CapabilityContractRoleMismatch {
        module: String,
        capability: Capability,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("module `{module}` declares capability `{capability}` without any resource kinds")]
    EmptyCapabilityResourceKinds {
        module: String,
        capability: Capability,
    },
    #[error(
        "module `{module}` declares a capability contract for `{capability}` without listing it as required or optional"
    )]
    UndeclaredCapabilityContract {
        module: String,
        capability: Capability,
    },
    #[error("module `{module}` declares duplicate job `{job}`")]
    DuplicateModuleJob { module: String, job: String },
    #[error("module `{module}` subscribes to `{event}` with unknown job `{job}`")]
    UnknownSubscriptionJob {
        module: String,
        event: String,
        job: String,
    },
    #[error("module `{module}` declares duplicate {kind} `{id}`")]
    DuplicateOperationalContribution {
        module: String,
        kind: &'static str,
        id: String,
    },
    #[error("module `{module}` has invalid {kind} `{id}`: {reason}")]
    InvalidOperationalContribution {
        module: String,
        kind: &'static str,
        id: String,
        reason: String,
    },
    #[error("module `{module}` declares duplicate extension slot `{kind:?}` on `{surface}`")]
    DuplicateExtensionSlot {
        module: String,
        kind: ExtensionSlotKind,
        surface: String,
    },
    #[error("module `{module}` has invalid extension slot `{kind:?}` on `{surface}`: {reason}")]
    InvalidExtensionSlot {
        module: String,
        kind: ExtensionSlotKind,
        surface: String,
        reason: String,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModuleInstallationError {
    #[error("module `{module}` requires module dependency `{dependency}`")]
    MissingModuleDependency { module: String, dependency: String },
    #[error(
        "module `{module}` requires core dependency `{dependency:?}` but service `{service_id}` is not available"
    )]
    MissingCoreServiceDependency {
        module: String,
        dependency: CoreServiceDependency,
        service_id: String,
    },
}

pub fn validate_module_capabilities<P>(
    package: &P,
    manifest: &ModuleManifest,
) -> Result<(), CapabilityValidationError>
where
    P: AuthModelPackage,
{
    for capability in &manifest.required_capabilities {
        if package.binding_for(*capability).is_none() {
            return Err(CapabilityValidationError::MissingCapability {
                module: manifest.name.clone(),
                capability: *capability,
            });
        }

        validate_capability_contract(manifest, *capability, true)?;
    }

    for capability in &manifest.optional_capabilities {
        validate_capability_contract(manifest, *capability, false)?;
    }

    for contract in &manifest.capability_contracts {
        let declared = manifest
            .required_capabilities
            .contains(&contract.capability)
            || manifest
                .optional_capabilities
                .contains(&contract.capability);
        if !declared {
            return Err(CapabilityValidationError::UndeclaredCapabilityContract {
                module: manifest.name.clone(),
                capability: contract.capability,
            });
        }
    }

    let mut seen_jobs = std::collections::BTreeSet::new();
    for job in &manifest.jobs {
        if !seen_jobs.insert(job.name.clone()) {
            return Err(CapabilityValidationError::DuplicateModuleJob {
                module: manifest.name.clone(),
                job: job.name.clone(),
            });
        }
    }

    for subscription in &manifest.event_subscriptions {
        if let Some(job) = &subscription.job {
            if !manifest.jobs.iter().any(|declared| declared.name == *job) {
                return Err(CapabilityValidationError::UnknownSubscriptionJob {
                    module: manifest.name.clone(),
                    event: subscription.event.clone(),
                    job: job.clone(),
                });
            }
        }
    }

    let mut seen_search = BTreeSet::new();
    for contribution in &manifest.search_contributions {
        if !seen_search.insert(contribution.id.clone()) {
            return Err(
                CapabilityValidationError::DuplicateOperationalContribution {
                    module: manifest.name.clone(),
                    kind: "search contribution",
                    id: contribution.id.clone(),
                },
            );
        }

        if contribution.fields.is_empty() {
            return Err(CapabilityValidationError::InvalidOperationalContribution {
                module: manifest.name.clone(),
                kind: "search contribution",
                id: contribution.id.clone(),
                reason: "at least one indexed field is required".to_string(),
            });
        }

        if matches!(contribution.visibility, SearchVisibility::Public)
            && !contribution.publication_required
        {
            return Err(CapabilityValidationError::InvalidOperationalContribution {
                module: manifest.name.clone(),
                kind: "search contribution",
                id: contribution.id.clone(),
                reason: "public search indexes must require publication state".to_string(),
            });
        }

        if let SearchVisibility::Capability(capability) = contribution.visibility {
            validate_declared_capability(manifest, capability)?;
        }
    }

    let mut seen_reports = BTreeSet::new();
    for definition in &manifest.report_definitions {
        if !seen_reports.insert(definition.id.clone()) {
            return Err(
                CapabilityValidationError::DuplicateOperationalContribution {
                    module: manifest.name.clone(),
                    kind: "report definition",
                    id: definition.id.clone(),
                },
            );
        }

        validate_declared_capability(manifest, definition.required_capability)?;
    }

    let mut seen_bulk = BTreeSet::new();
    for definition in &manifest.bulk_operations {
        if !seen_bulk.insert(definition.id.clone()) {
            return Err(
                CapabilityValidationError::DuplicateOperationalContribution {
                    module: manifest.name.clone(),
                    kind: "bulk operation",
                    id: definition.id.clone(),
                },
            );
        }

        validate_declared_capability(manifest, definition.required_capability)?;
    }

    let mut seen_extension_slots = BTreeSet::new();
    for slot in &manifest.extension_slots {
        if slot.surface.trim().is_empty() {
            return Err(CapabilityValidationError::InvalidExtensionSlot {
                module: manifest.name.clone(),
                kind: slot.kind,
                surface: slot.surface.clone(),
                reason: "surface must not be empty".to_string(),
            });
        }

        if slot.description.trim().is_empty() {
            return Err(CapabilityValidationError::InvalidExtensionSlot {
                module: manifest.name.clone(),
                kind: slot.kind,
                surface: slot.surface.clone(),
                reason: "description must not be empty".to_string(),
            });
        }

        if !seen_extension_slots.insert((slot.kind, slot.surface.clone())) {
            return Err(CapabilityValidationError::DuplicateExtensionSlot {
                module: manifest.name.clone(),
                kind: slot.kind,
                surface: slot.surface.clone(),
            });
        }
    }

    Ok(())
}

pub fn validate_module_installation(
    manifest: &ModuleManifest,
    installed_modules: &[String],
    core_service_ids: &[&str],
) -> Result<(), ModuleInstallationError> {
    for dependency in &manifest.module_dependencies {
        if dependency.kind == ModuleDependencyKind::Required
            && !installed_modules.contains(&dependency.module)
        {
            return Err(ModuleInstallationError::MissingModuleDependency {
                module: manifest.name.clone(),
                dependency: dependency.module.clone(),
            });
        }
    }

    for dependency in &manifest.core_service_dependencies {
        for service_id in dependency.required_service_ids() {
            if !core_service_ids.contains(service_id) {
                return Err(ModuleInstallationError::MissingCoreServiceDependency {
                    module: manifest.name.clone(),
                    dependency: *dependency,
                    service_id: (*service_id).to_string(),
                });
            }
        }
    }

    Ok(())
}

fn validate_capability_contract(
    manifest: &ModuleManifest,
    capability: Capability,
    required: bool,
) -> Result<(), CapabilityValidationError> {
    let Some(contract) = manifest
        .capability_contracts
        .iter()
        .find(|contract| contract.capability == capability)
    else {
        return Err(CapabilityValidationError::MissingCapabilityContract {
            module: manifest.name.clone(),
            capability,
        });
    };

    if contract.required != required {
        return Err(CapabilityValidationError::CapabilityContractRoleMismatch {
            module: manifest.name.clone(),
            capability,
            expected: if required { "required" } else { "optional" },
            actual: if contract.required {
                "required"
            } else {
                "optional"
            },
        });
    }

    if contract.resource_kinds.is_empty() {
        return Err(CapabilityValidationError::EmptyCapabilityResourceKinds {
            module: manifest.name.clone(),
            capability,
        });
    }

    Ok(())
}

fn validate_declared_capability(
    manifest: &ModuleManifest,
    capability: Capability,
) -> Result<(), CapabilityValidationError> {
    if package_capability_is_required(manifest, capability) {
        validate_capability_contract(manifest, capability, true)
    } else if manifest.optional_capabilities.contains(&capability) {
        validate_capability_contract(manifest, capability, false)
    } else {
        Err(CapabilityValidationError::UndeclaredCapabilityContract {
            module: manifest.name.clone(),
            capability,
        })
    }
}

fn package_capability_is_required(manifest: &ModuleManifest, capability: Capability) -> bool {
    manifest.required_capabilities.contains(&capability)
}
