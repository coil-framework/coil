use std::collections::BTreeMap;

use super::*;
use davenda_i18n::{LocaleTag, LocalizedUrls};
use davenda_seo::{HeadMetadata, OpenGraphData, OpenGraphType, RobotsDirective, page_node};
use davenda_wasm::RobotsDirective as TypedRobotsDirective;

impl RuntimePlan {
    pub(super) fn decorate_page_document(
        &self,
        execution: &RequestExecution,
        template_name: &str,
        document_html: String,
        extra_metadata: Option<&TypedMetadata>,
    ) -> Result<String, RuntimeRenderError> {
        let metadata =
            self.head_metadata_for_execution(execution, template_name, extra_metadata)?;
        let json_ld = if self.seo.allows_json_ld() {
            vec![page_node(
                metadata.title.clone(),
                metadata.canonical_url.clone(),
                metadata.description.clone(),
            )?]
        } else {
            Vec::new()
        };
        let extra_json_ld = extra_metadata
            .map(|extra_metadata| {
                extra_metadata
                    .json_ld
                    .iter()
                    .map(davenda_wasm::JsonLdNode::render)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let head_markup = render_head_markup(&metadata, &json_ld, &extra_json_ld);
        Ok(inject_head_markup(document_html, &head_markup))
    }

    fn head_metadata_for_execution(
        &self,
        execution: &RequestExecution,
        template_name: &str,
        extra_metadata: Option<&TypedMetadata>,
    ) -> Result<HeadMetadata, RuntimeRenderError> {
        let title = format!(
            "{} · {}",
            execution.route.route_name, execution.customer_app
        );
        let description = format!(
            "{} response for {} using {}",
            execution.route.route_name, execution.customer_app, template_name
        );
        let urls = self.localized_urls_for_execution(execution)?;
        let open_graph = Some(OpenGraphData::new(
            title.clone(),
            description.clone(),
            None,
            OpenGraphType::Website,
        )?);

        let mut metadata = HeadMetadata::new(
            title,
            description,
            urls,
            [RobotsDirective::Index, RobotsDirective::Follow],
            open_graph,
        )?;

        if let Some(extra_metadata) = extra_metadata {
            if let Some(title) = &extra_metadata.title {
                metadata.title = title.clone();
            }
            if let Some(description) = &extra_metadata.description {
                metadata.description = description.clone();
            }
            if let Some(canonical_url) = &extra_metadata.canonical_url {
                metadata.canonical_url = canonical_url.clone();
            }
            metadata
                .alternate_urls
                .extend(extra_metadata.alternate_urls.clone());
            metadata.robots.extend(
                extra_metadata
                    .robots
                    .iter()
                    .map(|directive| match directive {
                        TypedRobotsDirective::Index => RobotsDirective::Index,
                        TypedRobotsDirective::NoIndex => RobotsDirective::NoIndex,
                        TypedRobotsDirective::Follow => RobotsDirective::Follow,
                        TypedRobotsDirective::NoFollow => RobotsDirective::NoFollow,
                        TypedRobotsDirective::NoArchive => RobotsDirective::NoArchive,
                    }),
            );
            metadata.open_graph = Some(OpenGraphData::new(
                metadata.title.clone(),
                metadata.description.clone(),
                None,
                OpenGraphType::Website,
            )?);
        }

        Ok(metadata)
    }

    fn localized_urls_for_execution(
        &self,
        execution: &RequestExecution,
    ) -> Result<LocalizedUrls, RuntimeRenderError> {
        let route = self
            .http
            .routes
            .iter()
            .find(|route| route.name == execution.route.route_name)
            .expect("request execution routes must resolve from the runtime plan");
        let current_locale = LocaleTag::new(execution.locale.clone())?;
        let locales = if route.locale_policy == LocalePolicy::Localized {
            let mut locales = vec![current_locale.clone()];
            locales.extend(
                self.i18n
                    .supported_locales
                    .iter()
                    .filter(|locale| **locale != current_locale)
                    .cloned(),
            );
            locales
        } else {
            vec![self.i18n.default_locale.clone()]
        };
        let canonical = self.http.absolute_url_for(
            &self.config,
            &execution.route.route_name,
            &execution.route.params,
            Some(current_locale.as_str()),
        )?;
        let mut alternate_hreflang = BTreeMap::new();
        for locale in locales {
            alternate_hreflang.insert(
                locale.to_string(),
                self.http.absolute_url_for(
                    &self.config,
                    &execution.route.route_name,
                    &execution.route.params,
                    Some(locale.as_str()),
                )?,
            );
        }

        Ok(LocalizedUrls {
            canonical,
            alternate_hreflang,
        })
    }
}

fn render_head_markup(
    metadata: &HeadMetadata,
    json_ld: &[davenda_seo::JsonLdNode],
    extra_json_ld: &[String],
) -> String {
    let mut markup = String::new();
    markup.push_str(&format!(
        "<meta name=\"description\" content=\"{}\">",
        escape_html_attribute(&metadata.description)
    ));
    markup.push_str(&format!(
        "<link rel=\"canonical\" href=\"{}\">",
        escape_html_attribute(&metadata.canonical_url)
    ));
    if !metadata.robots.is_empty() {
        markup.push_str(&format!(
            "<meta name=\"robots\" content=\"{}\">",
            escape_html_attribute(&metadata.robots_content())
        ));
    }
    for (locale, url) in &metadata.alternate_urls {
        markup.push_str(&format!(
            "<link rel=\"alternate\" hreflang=\"{}\" href=\"{}\">",
            escape_html_attribute(locale),
            escape_html_attribute(url)
        ));
    }
    if let Some(open_graph) = &metadata.open_graph {
        markup.push_str(&format!(
            "<meta property=\"og:title\" content=\"{}\">",
            escape_html_attribute(&open_graph.title)
        ));
        markup.push_str(&format!(
            "<meta property=\"og:description\" content=\"{}\">",
            escape_html_attribute(&open_graph.description)
        ));
        markup.push_str(&format!(
            "<meta property=\"og:type\" content=\"{}\">",
            open_graph.graph_type
        ));
        if let Some(image_url) = &open_graph.image_url {
            markup.push_str(&format!(
                "<meta property=\"og:image\" content=\"{}\">",
                escape_html_attribute(image_url)
            ));
        }
    }
    for node in json_ld {
        markup.push_str(&format!(
            "<script type=\"application/ld+json\">{}</script>",
            node.render()
        ));
    }
    for node in extra_json_ld {
        markup.push_str(&format!(
            "<script type=\"application/ld+json\">{}</script>",
            node
        ));
    }
    markup
}

fn inject_head_markup(document_html: String, head_markup: &str) -> String {
    if head_markup.is_empty() {
        return document_html;
    }

    if let Some(index) = document_html.find("</head>") {
        let mut html = document_html;
        html.insert_str(index, head_markup);
        return html;
    }

    if let Some(index) = document_html.find("<body") {
        let mut html = document_html;
        html.insert_str(index, &format!("<head>{head_markup}</head>"));
        return html;
    }

    format!("<head>{head_markup}</head>{document_html}")
}

fn escape_html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
