use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplateRegistry {
    templates: BTreeMap<TemplateKey, TemplateDefinition>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, template: TemplateDefinition) -> Result<(), TemplateModelError> {
        if self.templates.contains_key(&template.key) {
            return Err(TemplateModelError::DuplicateTemplate {
                key: template.key.clone(),
            });
        }

        self.templates.insert(template.key.clone(), template);
        Ok(())
    }

    pub(crate) fn resolve(
        &self,
        namespaces: &[TemplateNamespace],
        selector: &TemplateSelector,
    ) -> Result<&TemplateDefinition, TemplateModelError> {
        for namespace in namespaces {
            let key = TemplateKey::new(namespace.clone(), selector.name().clone());
            if let Some(template) = self.templates.get(&key) {
                return Ok(template);
            }
        }

        Err(TemplateModelError::TemplateNotFound {
            name: selector.name().clone(),
        })
    }
}
