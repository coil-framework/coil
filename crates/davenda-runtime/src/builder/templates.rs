use super::*;
use davenda_template::{
    TemplateDefinition, TemplateKind, TemplateModelError, TemplateNamespace, TemplateSourceParser,
};
use std::fs;
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

    let parser = TemplateSourceParser::new();
    let mut files = Vec::new();
    collect_customer_template_files(&templates_root, &mut files).map_err(RuntimeBuildError::from)?;
    files.sort();

    let mut templates = Vec::with_capacity(files.len());
    for path in files {
        let source = fs::read_to_string(&path).map_err(|error| {
            RuntimeBuildError::from(TemplateModelError::TemplateRead {
                path: path.display().to_string(),
                message: error.to_string(),
            })
        })?;
        let kind = classify_customer_template(&templates_root, &path, &source);
        templates.push(
            parser
                .parse_source(&templates_root, &path, &source, namespace.clone(), kind)
                .map_err(RuntimeBuildError::from)?,
        );
    }
    if templates.is_empty() {
        return Err(RuntimeBuildError::EmptyTemplateTree {
            path: templates_root.display().to_string(),
        });
    }

    Ok(templates)
}

fn collect_customer_template_files(
    dir: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), TemplateModelError> {
    for entry in fs::read_dir(dir).map_err(|error| TemplateModelError::TemplateRead {
        path: dir.display().to_string(),
        message: error.to_string(),
    })? {
        let entry = entry.map_err(|error| TemplateModelError::TemplateRead {
            path: dir.display().to_string(),
            message: error.to_string(),
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_customer_template_files(&path, files)?;
            continue;
        }

        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("html"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }

    Ok(())
}

fn classify_customer_template(root: &Path, path: &Path, source: &str) -> TemplateKind {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let leading_segment = relative
        .components()
        .next()
        .and_then(|component| component.as_os_str().to_str());
    match leading_segment {
        Some("pages") | Some("layouts") => TemplateKind::Layout,
        Some("components") | Some("fragments") => TemplateKind::Fragment,
        _ => {
            let lower = source.to_ascii_lowercase();
            if lower.contains("<!doctype") || lower.contains("<html") || lower.contains("<body") {
                TemplateKind::Layout
            } else {
                TemplateKind::Fragment
            }
        }
    }
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

        let templates = load_customer_templates_from_root(
            &root,
            TemplateNamespace::new("customer-app").unwrap(),
        )
        .unwrap();

        assert_eq!(templates.len(), 2);
        assert!(
            templates
                .iter()
                .any(|template| template.key.name.as_str() == "layouts/base")
        );
        assert!(
            templates
                .iter()
                .any(|template| template.key.name.as_str() == "components/hero")
        );
        assert!(
            templates
                .iter()
                .find(|template| template.key.name.as_str() == "layouts/base")
                .is_some_and(|template| template.kind == TemplateKind::Layout)
        );
        assert!(
            templates
                .iter()
                .find(|template| template.key.name.as_str() == "components/hero")
                .is_some_and(|template| template.kind == TemplateKind::Fragment)
        );
    }

    #[test]
    fn classifies_customer_templates_for_storefront_and_account_surfaces() {
        let root = unique_root("surface-kinds");
        write_file(
            &root.join("templates/navigation/primary.html"),
            r#"<nav class="primary-nav" xmlns:dv="https://davenda.dev" dv:fragment="primary">
  <a href="/account">Account</a>
</nav>"#,
        );
        write_file(
            &root.join("templates/commerce/collection-grid.html"),
            r#"<section class="collection-grid" xmlns:dv="https://davenda.dev" dv:fragment="grid">
  <p>Featured collections</p>
</section>"#,
        );
        write_file(
            &root.join("templates/account/dashboard.html"),
            r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <aside dv:replace="~{account/sidebar}"></aside>
    <main>
      <h1>Dashboard</h1>
    </main>
  </body>
</html>"#,
        );
        write_file(
            &root.join("templates/account/sidebar.html"),
            r#"<aside class="account-sidebar" xmlns:dv="https://davenda.dev" dv:fragment="sidebar">
  <a href="/account">Account</a>
</aside>"#,
        );

        let templates = load_customer_templates_from_root(
            &root,
            TemplateNamespace::new("customer-app").unwrap(),
        )
        .unwrap();

        assert!(
            templates
                .iter()
                .find(|template| template.key.name.as_str() == "navigation/primary")
                .is_some_and(|template| template.kind == TemplateKind::Fragment)
        );
        assert!(
            templates
                .iter()
                .find(|template| template.key.name.as_str() == "commerce/collection-grid")
                .is_some_and(|template| template.kind == TemplateKind::Fragment)
        );
        assert!(
            templates
                .iter()
                .find(|template| template.key.name.as_str() == "account/dashboard")
                .is_some_and(|template| template.kind == TemplateKind::Layout)
        );
        assert!(
            templates
                .iter()
                .find(|template| template.key.name.as_str() == "account/sidebar")
                .is_some_and(|template| template.kind == TemplateKind::Fragment)
        );
    }

    #[test]
    fn rejects_missing_template_tree() {
        let root = unique_root("missing");
        let error = load_customer_templates_from_root(
            &root,
            TemplateNamespace::new("customer-app").unwrap(),
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            format!("customer app root `{}` does not exist", root.display())
        );
    }

    #[test]
    fn rejects_empty_template_tree() {
        let root = unique_root("empty");
        fs::create_dir_all(root.join("templates")).unwrap();

        let error = load_customer_templates_from_root(
            &root,
            TemplateNamespace::new("customer-app").unwrap(),
        )
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
