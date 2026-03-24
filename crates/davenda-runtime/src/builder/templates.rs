use super::*;
use davenda_template::{TemplateDefinition, TemplateNamespace, TemplateSourceParser};
use std::path::{Path, PathBuf};

pub(crate) fn load_customer_templates_from_roots(
    roots: &[PathBuf],
    namespace: TemplateNamespace,
) -> Result<Vec<TemplateDefinition>, RuntimeBuildError> {
    let mut templates = Vec::new();
    for root in roots {
        templates.extend(load_customer_templates_from_root(root, namespace.clone())?);
    }
    Ok(templates)
}

pub(crate) fn load_customer_templates_from_root(
    root: &Path,
    namespace: TemplateNamespace,
) -> Result<Vec<TemplateDefinition>, RuntimeBuildError> {
    if !root.exists() {
        return Err(RuntimeBuildError::MissingCustomerAppRoot {
            path: root.display().to_string(),
        });
    }
    if !root.is_dir() {
        return Err(RuntimeBuildError::CustomerAppRootNotDirectory {
            path: root.display().to_string(),
        });
    }

    let templates_root = root.join("templates");
    if !templates_root.exists() {
        return Err(RuntimeBuildError::MissingTemplateTree {
            path: templates_root.display().to_string(),
        });
    }
    if !templates_root.is_dir() {
        return Err(RuntimeBuildError::CustomerAppRootNotDirectory {
            path: templates_root.display().to_string(),
        });
    }

    let templates = TemplateSourceParser::new()
        .load_directory(&templates_root, namespace)
        .map_err(RuntimeBuildError::from)?;
    if templates.is_empty() {
        return Err(RuntimeBuildError::EmptyTemplateTree {
            path: templates_root.display().to_string(),
        });
    }

    Ok(templates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("davenda-runtime-template-loader-{label}-{unique}"))
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn discovers_customer_templates_under_template_tree() {
        let root = unique_root("discover");
        write_file(
            &root.join("templates/layouts/base.html"),
            r#"<!doctype html>
<html xmlns:dv="https://davenda.dev" dv:fragment="shell">
  <body>
    <main dv:insert="~{::content}">
      <section dv:fragment="content"><p>Fallback</p></section>
    </main>
  </body>
</html>"#,
        );
        write_file(
            &root.join("templates/components/hero.html"),
            r#"<section class="hero" xmlns:dv="https://davenda.dev" dv:fragment="hero">Hero</section>"#,
        );

        let templates =
            load_customer_templates_from_root(&root, TemplateNamespace::new("customer-app").unwrap())
                .unwrap();

        assert_eq!(templates.len(), 2);
        assert!(templates
            .iter()
            .any(|template| template.key.name.as_str() == "layouts/base"));
        assert!(templates
            .iter()
            .any(|template| template.key.name.as_str() == "components/hero"));
    }

    #[test]
    fn rejects_missing_template_tree() {
        let root = unique_root("missing");
        let error =
            load_customer_templates_from_root(&root, TemplateNamespace::new("customer-app").unwrap())
                .unwrap_err();

        assert_eq!(
            error.to_string(),
            format!(
                "customer app root `{}` does not exist",
                root.display()
            )
        );
    }

    #[test]
    fn rejects_empty_template_tree() {
        let root = unique_root("empty");
        fs::create_dir_all(root.join("templates")).unwrap();

        let error =
            load_customer_templates_from_root(&root, TemplateNamespace::new("customer-app").unwrap())
                .unwrap_err();

        assert_eq!(
            error.to_string(),
            format!(
                "customer app templates directory `{}` does not contain any `.html` templates",
                root.join("templates").display()
            )
        );
    }
}
