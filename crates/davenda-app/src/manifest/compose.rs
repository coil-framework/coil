use super::*;

impl CustomerAppManifest {
    pub fn compose<P>(
        &self,
        auth_package: &P,
        manifests: &[ModuleManifest],
    ) -> Result<CustomerAppComposition, AppModelError>
    where
        P: AuthModelPackage,
    {
        self.validate()?;

        if auth_package.manifest().name != self.auth.package_name {
            return Err(AppModelError::AuthPackageMismatch {
                app_id: self.id.to_string(),
                configured: self.auth.package_name.clone(),
                actual: auth_package.manifest().name.clone(),
            });
        }

        let installed_ids = self
            .modules
            .iter()
            .map(|module| module.id.to_string())
            .collect::<Vec<_>>();

        let mut selected = Vec::new();
        for module in &self.modules {
            let manifest = manifests
                .iter()
                .find(|candidate| candidate.name == module.id.as_str())
                .ok_or_else(|| AppModelError::UnknownInstalledModule {
                    app_id: self.id.to_string(),
                    module: module.id.to_string(),
                })?;

            validate_module_capabilities(auth_package, manifest)?;
            selected.push(manifest.clone());
        }

        for manifest in &selected {
            for dependency in &manifest.module_dependencies {
                if dependency.kind == ModuleDependencyKind::Required
                    && !installed_ids
                        .iter()
                        .any(|module| module == &dependency.module)
                {
                    return Err(AppModelError::MissingModuleDependency {
                        module: manifest.name.clone(),
                        dependency: dependency.module.clone(),
                    });
                }
            }
        }

        let mut module_inventory = Vec::new();
        let mut required_core_services = Vec::new();
        let mut migrations = self.customer_migrations.clone();
        let mut route_surfaces = Vec::new();
        let mut jobs = Vec::new();
        let mut event_subscriptions = Vec::new();
        let mut admin_resources = Vec::new();
        let mut search_contributions = Vec::new();
        let mut report_definitions = Vec::new();
        let mut bulk_operations = Vec::new();

        for manifest in &selected {
            for dependency in &manifest.core_service_dependencies {
                if !required_core_services.contains(dependency) {
                    required_core_services.push(*dependency);
                }
            }

            migrations.extend(manifest.migrations.clone());
            route_surfaces.extend(manifest.route_surfaces.clone());
            jobs.extend(manifest.jobs.clone());
            event_subscriptions.extend(manifest.event_subscriptions.clone());
            admin_resources.extend(manifest.admin_resources.clone());
            search_contributions.extend(manifest.search_contributions.clone());
            report_definitions.extend(manifest.report_definitions.clone());
            bulk_operations.extend(manifest.bulk_operations.clone());

            let installed_spec = self
                .modules
                .iter()
                .find(|spec| spec.id.as_str() == manifest.name)
                .expect("selected manifests always correspond to installed modules");
            module_inventory.push(InstalledModuleSummary {
                id: installed_spec.id.clone(),
                version_req: installed_spec.version_req.clone(),
                module_dependencies: manifest.module_dependencies.clone(),
                core_service_dependencies: manifest.core_service_dependencies.clone(),
                migrations: manifest.migrations.clone(),
                route_surfaces: manifest.route_surfaces.clone(),
                jobs: manifest.jobs.clone(),
                event_subscriptions: manifest.event_subscriptions.clone(),
                admin_resources: manifest.admin_resources.clone(),
                search_contributions: manifest.search_contributions.clone(),
                report_definitions: manifest.report_definitions.clone(),
                bulk_operations: manifest.bulk_operations.clone(),
            });
        }

        Ok(CustomerAppComposition {
            app_id: self.id.clone(),
            display_name: self.display_name.clone(),
            domains: self.domains.clone(),
            default_locale: self.default_locale.clone(),
            supported_locales: self.supported_locales.clone(),
            installed_modules: self.modules.clone(),
            module_inventory,
            required_core_services,
            migrations,
            route_surfaces,
            jobs,
            event_subscriptions,
            admin_resources,
            search_contributions,
            report_definitions,
            bulk_operations,
            theme: self.theme.clone(),
            content_models: self.content_models.clone(),
            extensions: self.extensions.clone(),
            auth: self.auth.clone(),
        })
    }
}
