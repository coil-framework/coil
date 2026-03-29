use super::*;
use coil_template::{
    TemplateDefinition, TemplateKind, TemplateModelError, TemplateName, TemplateNamespace,
    TemplateSourceParser,
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

pub(crate) fn supplement_customer_templates(
    templates: &mut Vec<TemplateDefinition>,
    namespace: TemplateNamespace,
    module_names: &[String],
) -> Result<(), RuntimeBuildError> {
    if !module_names.iter().any(|name| name == "events") {
        return Ok(());
    }

    let parser = TemplateSourceParser::new();
    if !templates
        .iter()
        .any(|template| template.key.name.as_str() == "events/list")
    {
        templates.push(parser.parse_layout(
            namespace.clone(),
            TemplateName::new("events/list")?,
            events_list_template(),
        )?);
    }
    if !templates
        .iter()
        .any(|template| template.key.name.as_str() == "events/detail")
    {
        templates.push(parser.parse_layout(
            namespace,
            TemplateName::new("events/detail")?,
            events_detail_template(),
        )?);
    }

    Ok(())
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
    collect_customer_template_files(&templates_root, &mut files)
        .map_err(RuntimeBuildError::from)?;
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

fn events_list_template() -> &'static str {
    r#"<!doctype html>
<html xmlns:coil="https://coil.rs" coil:attr="lang=${locale}" lang="en-GB">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Events · Shoppr</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="@{theme/assets/site.css}" />
  </head>
  <body class="harbor events">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main">
      <section class="home-page events-page">
        <article class="catalog-section">
          <p class="catalog-section__eyebrow">Public events</p>
          <h1>Events are enabled, but the sample catalog is still being wired.</h1>
          <p class="events-route" coil:text="${route_name}">events.list</p>
          <p>
            This route is live and locale-aware. Published event records will appear here once the
            events catalog is connected; until then, the page keeps the member journey honest.
          </p>
          <div class="checkout-actions">
            <a class="button" href="/shop/collections/events" coil:attr="href=${links.events_collection}">
              Browse event-linked offers
            </a>
            <a
              class="button button--secondary"
              href="/account/memberships"
              coil:attr="href=${links.memberships}"
            >
              Review memberships
            </a>
          </div>
        </article>
        <article class="catalog-section">
          <p class="catalog-section__eyebrow">Member path</p>
          <h2>Prepare to book</h2>
          <p>
            The checkout and account journey already exists. This route keeps the events entry
            point close to those live surfaces without inventing event inventory.
          </p>
          <div class="checkout-actions">
            <a class="button" href="/account" coil:attr="href=${links.account}">Open account</a>
            <a
              class="button button--secondary"
              href="/shop/collections/memberships"
              coil:attr="href=${links.memberships_collection}"
            >
              Membership offers
            </a>
          </div>
        </article>
      </section>
    </main>
  </body>
</html>"#
}

fn events_detail_template() -> &'static str {
    r#"<!doctype html>
<html xmlns:coil="https://coil.rs" coil:attr="lang=${locale}" lang="en-GB">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Event detail · Shoppr</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="@{theme/assets/site.css}" />
  </head>
  <body class="harbor events">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{navigation/primary}"></nav>
    </header>
    <main class="site-main">
      <section class="home-page events-page">
        <article class="catalog-section">
          <p class="catalog-section__eyebrow">Event detail</p>
          <h1 coil:text="${route_params.event_slug}">spring-tasting</h1>
          <p>
            Event records are not published in the checked-in Shoppr sample yet, so this
            route shows the real customer entry point without pretending there is catalog data
            behind it.
          </p>
          <p class="catalog-section__eyebrow">Current route</p>
          <p class="events-route" coil:text="${route_name}">events.detail</p>
          <div class="checkout-actions">
            <a class="button" href="/events">Back to events</a>
            <a class="button button--secondary" href="/account" coil:attr="href=${links.account}">
              Open account
            </a>
          </div>
        </article>
        <article class="catalog-section">
          <p class="catalog-section__eyebrow">Membership path</p>
          <h2>Use the live membership flow</h2>
          <p>
            Events in this installation are tied to membership and booking capability. The sample
            app keeps the route live while the event catalog is still being connected.
          </p>
          <div class="checkout-actions">
            <a
              class="button"
              href="/account/memberships"
              coil:attr="href=${links.memberships}"
            >
              Review memberships
            </a>
            <a
              class="button button--secondary"
              href="/shop/collections/events"
              coil:attr="href=${links.events_collection}"
            >
              Browse event-linked offers
            </a>
          </div>
        </article>
      </section>
    </main>
  </body>
</html>"#
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
        std::env::temp_dir().join(format!("coil-runtime-template-loader-{label}-{unique}"))
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
<html xmlns:coil="https://coil.rs" coil:fragment="shell">
  <body>
    <main coil:insert="~{::content}">
      <section coil:fragment="content"><p>Fallback</p></section>
    </main>
  </body>
</html>"#,
        );
        write_file(
            &root.join("templates/components/hero.html"),
            r#"<section class="hero" xmlns:coil="https://coil.rs" coil:fragment="hero">Hero</section>"#,
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
            r#"<nav class="primary-nav" xmlns:coil="https://coil.rs" coil:fragment="primary">
  <a href="/account">Account</a>
</nav>"#,
        );
        write_file(
            &root.join("templates/commerce/collection-grid.html"),
            r#"<section class="collection-grid" xmlns:coil="https://coil.rs" coil:fragment="grid">
  <p>Featured collections</p>
</section>"#,
        );
        write_file(
            &root.join("templates/account/dashboard.html"),
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <aside coil:replace="~{account/sidebar}"></aside>
    <main>
      <h1>Dashboard</h1>
    </main>
  </body>
</html>"#,
        );
        write_file(
            &root.join("templates/account/sidebar.html"),
            r#"<aside class="account-sidebar" xmlns:coil="https://coil.rs" coil:fragment="sidebar">
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
    fn supplements_honest_events_surfaces_when_events_module_is_installed() {
        let mut templates = vec![
            TemplateSourceParser::new()
                .parse_layout(
                    TemplateNamespace::new("customer-app").unwrap(),
                    TemplateName::new("pages/home").unwrap(),
                    r#"<!doctype html>
<html xmlns:coil="https://coil.rs"><body><main>Home</main></body></html>"#,
                )
                .unwrap(),
        ];

        supplement_customer_templates(
            &mut templates,
            TemplateNamespace::new("customer-app").unwrap(),
            &[String::from("events")],
        )
        .unwrap();

        assert!(
            templates
                .iter()
                .any(|template| template.key.name.as_str() == "events/list")
        );
        assert!(
            templates
                .iter()
                .any(|template| template.key.name.as_str() == "events/detail")
        );
    }

    #[test]
    fn does_not_supplement_events_surfaces_without_events_module() {
        let mut templates = vec![
            TemplateSourceParser::new()
                .parse_layout(
                    TemplateNamespace::new("customer-app").unwrap(),
                    TemplateName::new("pages/home").unwrap(),
                    r#"<!doctype html>
<html xmlns:coil="https://coil.rs"><body><main>Home</main></body></html>"#,
                )
                .unwrap(),
        ];

        supplement_customer_templates(
            &mut templates,
            TemplateNamespace::new("customer-app").unwrap(),
            &[String::from("commerce")],
        )
        .unwrap();

        assert!(
            templates
                .iter()
                .all(|template| template.key.name.as_str() != "events/list")
        );
        assert!(
            templates
                .iter()
                .all(|template| template.key.name.as_str() != "events/detail")
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
