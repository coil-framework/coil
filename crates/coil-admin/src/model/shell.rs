use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminShell {
    accessibility: AccessibilityContract,
    resources: Vec<AdminResourceDescriptor>,
    widgets: Vec<AdminWidgetDescriptor>,
    workflows: Vec<WorkflowAction>,
    audit_log: Vec<AuditEntry>,
}

impl AdminShell {
    pub fn new(
        accessibility: AccessibilityContract,
        resources: Vec<AdminResourceDescriptor>,
        widgets: Vec<AdminWidgetDescriptor>,
        workflows: Vec<WorkflowAction>,
    ) -> Result<Self, AdminModelError> {
        ensure_unique_resources(&resources)?;
        ensure_unique_widgets(&widgets)?;
        ensure_unique_workflows(&workflows)?;
        Ok(Self {
            accessibility,
            resources,
            widgets,
            workflows,
            audit_log: Vec::new(),
        })
    }

    pub fn accessibility(&self) -> &AccessibilityContract {
        &self.accessibility
    }

    pub fn resources(&self) -> &[AdminResourceDescriptor] {
        &self.resources
    }

    pub fn widgets(&self) -> &[AdminWidgetDescriptor] {
        &self.widgets
    }

    pub fn workflows(&self) -> &[WorkflowAction] {
        &self.workflows
    }

    pub fn visible_resources(
        &self,
        operator: &OperatorAccessContext,
    ) -> Vec<AdminResourceDescriptor> {
        self.resources
            .iter()
            .filter(|resource| operator.allows(resource.required_capability))
            .cloned()
            .collect()
    }

    pub fn compose_module_resources(
        manifests: &[ModuleManifest],
    ) -> Result<Vec<AdminResourceDescriptor>, AdminModelError> {
        let mut resources = Vec::new();
        for manifest in manifests {
            for contribution in &manifest.admin_resources {
                resources.push(AdminResourceDescriptor::from_contribution(contribution)?);
            }
        }
        ensure_unique_resources(&resources)?;
        Ok(resources)
    }

    pub fn compose_module_workflows(
        manifests: &[ModuleManifest],
    ) -> Result<Vec<WorkflowAction>, AdminModelError> {
        let mut workflows = Vec::new();
        for manifest in manifests {
            for definition in &manifest.bulk_operations {
                workflows.push(WorkflowAction::from_bulk_operation(definition)?);
            }
        }
        ensure_unique_workflows(&workflows)?;
        Ok(workflows)
    }

    pub fn compose_extension_widgets(
        registry: &ExtensionRegistry,
    ) -> Result<Vec<AdminWidgetDescriptor>, AdminModelError> {
        let mut widgets = Vec::new();

        for handler in registry.registered_handlers() {
            if handler.point != coil_wasm::ExtensionPointKind::AdminWidget {
                continue;
            }

            widgets.push(AdminWidgetDescriptor::new(
                AdminWidgetId::new(format!(
                    "ext.{}.{}",
                    handler.extension_id, handler.handler_id
                ))?,
                format!("{} widget", handler.extension_id),
                map_extension_widget_slot(&handler.surface),
                Some(Capability::AdminShellAccess),
                None,
            )?);
        }

        ensure_unique_widgets(&widgets)?;
        Ok(widgets)
    }

    pub fn navigation_by_section(
        &self,
        operator: &OperatorAccessContext,
    ) -> HashMap<NavigationSection, Vec<AdminResourceDescriptor>> {
        let mut grouped = HashMap::new();
        for resource in self.visible_resources(operator) {
            grouped
                .entry(resource.section)
                .or_insert_with(Vec::new)
                .push(resource);
        }
        grouped
    }

    pub fn visible_widgets(&self, operator: &OperatorAccessContext) -> Vec<AdminWidgetDescriptor> {
        self.widgets
            .iter()
            .filter(|widget| {
                widget
                    .required_capability
                    .is_none_or(|capability| operator.allows(capability))
            })
            .cloned()
            .collect()
    }

    pub fn build_bulk_action_plan(
        &self,
        workflow_id: &WorkflowId,
        resource_count: usize,
        operator: &OperatorAccessContext,
    ) -> Option<BulkActionPlan> {
        let workflow = self
            .workflows
            .iter()
            .find(|workflow| &workflow.id == workflow_id)?;
        if !operator.allows(workflow.required_capability) {
            return None;
        }

        Some(BulkActionPlan {
            workflow_id: workflow.id.clone(),
            resource_count,
            message: workflow.success_message.clone(),
        })
    }

    pub fn record_audit_entry(&mut self, entry: AuditEntry) {
        self.audit_log.push(entry);
    }

    pub fn visible_audit_entries(&self, operator: &OperatorAccessContext) -> &[AuditEntry] {
        if operator.allows(Capability::AdminAuditRead) {
            &self.audit_log
        } else {
            &[]
        }
    }
}

fn ensure_unique_resources(resources: &[AdminResourceDescriptor]) -> Result<(), AdminModelError> {
    let mut seen = HashSet::new();
    for resource in resources {
        if !seen.insert(resource.id.clone()) {
            return Err(AdminModelError::DuplicateResource {
                resource_id: resource.id.to_string(),
            });
        }
    }
    Ok(())
}

fn ensure_unique_widgets(widgets: &[AdminWidgetDescriptor]) -> Result<(), AdminModelError> {
    let mut seen = HashSet::new();
    for widget in widgets {
        if !seen.insert(widget.id.clone()) {
            return Err(AdminModelError::DuplicateWidget {
                widget_id: widget.id.to_string(),
            });
        }
    }
    Ok(())
}

fn ensure_unique_workflows(workflows: &[WorkflowAction]) -> Result<(), AdminModelError> {
    let mut seen = HashSet::new();
    for workflow in workflows {
        if !seen.insert(workflow.id.clone()) {
            return Err(AdminModelError::DuplicateWorkflow {
                workflow_id: workflow.id.to_string(),
            });
        }
    }
    Ok(())
}
