use super::*;

fn namespaces() -> Vec<TemplateNamespace> {
    vec![
        TemplateNamespace::new("customer-app").unwrap(),
        TemplateNamespace::new("events").unwrap(),
        TemplateNamespace::new("core").unwrap(),
    ]
}

fn selector(name: &str) -> TemplateSelector {
    TemplateSelector::new(TemplateName::new(name).unwrap())
}

fn model() -> RenderModel {
    RenderModel::new()
        .with_value("title", RenderValue::text("Event <Launch>"))
        .unwrap()
        .with_value("headline", RenderValue::text("Book & Save"))
        .unwrap()
        .with_value("cta_class", RenderValue::text("primary\" onclick=\"oops"))
        .unwrap()
        .with_value(
            "trusted_badge",
            RenderValue::trusted_html(
                TrustedHtml::new("<strong class=\"badge\">Live</strong>").unwrap(),
            ),
        )
        .unwrap()
}

fn model_with_assets() -> RenderModel {
    model()
        .with_asset_path(
            "theme/assets/site.css",
            "https://cdn.example.com/theme/assets/site.abc123.css",
        )
        .unwrap()
}

fn model_with_translations() -> RenderModel {
    model()
        .with_translation("home.title", "Bonjour & bienvenue")
        .unwrap()
        .with_translation("home.cta", "Voir les nouveautes")
        .unwrap()
}

fn block_model() -> RenderModel {
    RenderModel::new()
        .with_list(
            "blocks",
            vec![
                RenderModel::new()
                    .with_value("type", RenderValue::text("hero_section"))
                    .unwrap()
                    .with_object(
                        "fields",
                        RenderModel::new()
                            .with_value("title", RenderValue::text("Hero title"))
                            .unwrap(),
                    )
                    .unwrap(),
                RenderModel::new()
                    .with_value("type", RenderValue::text("featured_events"))
                    .unwrap()
                    .with_object(
                        "fields",
                        RenderModel::new()
                            .with_value("title", RenderValue::text("Featured events"))
                            .unwrap(),
                    )
                    .unwrap(),
            ],
        )
        .unwrap()
}

fn base_registry() -> TemplateRegistry {
    let mut registry = TemplateRegistry::new();

    registry
        .register(TemplateDefinition::layout(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("storefront.layout").unwrap(),
            vec![
                Node::static_text("<!DOCTYPE html>"),
                Node::Element(
                    ElementNode::new(
                        "html",
                        vec![Node::Element(
                            ElementNode::new(
                                "body",
                                vec![
                                    Node::Slot(
                                        SlotNode::new(SlotName::new("hero").unwrap())
                                            .with_fallback(vec![Node::static_text(
                                                "<div class=\"hero-fallback\"></div>",
                                            )]),
                                    ),
                                    Node::Element(
                                        ElementNode::new(
                                            "main",
                                            vec![Node::Slot(SlotNode::new(
                                                SlotName::new("content").unwrap(),
                                            ))],
                                        )
                                        .unwrap(),
                                    ),
                                ],
                            )
                            .unwrap(),
                        )],
                    )
                    .unwrap(),
                ),
            ],
        ))
        .unwrap();

    registry
        .register(TemplateDefinition::fragment(
            TemplateNamespace::new("events").unwrap(),
            TemplateName::new("hero").unwrap(),
            vec![Node::Element(
                ElementNode::new(
                    "section",
                    vec![
                        Node::Element(
                            ElementNode::new("h1", vec![Node::value("headline").unwrap()]).unwrap(),
                        ),
                        Node::raw_value("trusted_badge").unwrap(),
                    ],
                )
                .unwrap()
                .with_attribute(AttributeNode::static_value("class", "hero").unwrap()),
            )],
        ))
        .unwrap();

    registry
        .register(TemplateDefinition::fragment(
            TemplateNamespace::new("events").unwrap(),
            TemplateName::new("booking.panel").unwrap(),
            vec![Node::Element(
                ElementNode::new("div", vec![Node::value("title").unwrap()])
                    .unwrap()
                    .with_attribute(
                        AttributeNode::static_value("data-fragment", "booking").unwrap(),
                    )
                    .with_attribute(AttributeNode::dynamic_text("class", "cta_class").unwrap()),
            )],
        ))
        .unwrap();

    registry
        .register(TemplateDefinition::fragment(
            TemplateNamespace::new("customer-app").unwrap(),
            TemplateName::new("hero").unwrap(),
            vec![Node::Element(
                ElementNode::new(
                    "section",
                    vec![Node::Element(
                        ElementNode::new("h1", vec![Node::static_text("Branded Hero")]).unwrap(),
                    )],
                )
                .unwrap()
                .with_attribute(AttributeNode::static_value("class", "hero customer").unwrap()),
            )],
        ))
        .unwrap();

    registry
}

#[test]
fn template_registry_rejects_duplicate_registration() {
    let mut registry = TemplateRegistry::new();
    let template = TemplateDefinition::fragment(
        TemplateNamespace::new("events").unwrap(),
        TemplateName::new("hero").unwrap(),
        vec![Node::static_text("<p>Hero</p>")],
    );

    registry.register(template.clone()).unwrap();

    assert_eq!(
        registry.register(template).unwrap_err(),
        TemplateModelError::DuplicateTemplate {
            key: TemplateKey::new(
                TemplateNamespace::new("events").unwrap(),
                TemplateName::new("hero").unwrap()
            )
        }
    );
}

#[test]
fn template_model_rejects_invalid_tokens_and_names() {
    assert!(TemplateNamespace::new("bad namespace").is_err());
    assert!(TemplateName::new("bad name").is_err());
    assert!(SlotName::new("bad slot").is_err());
    assert!(Node::value("bad key!").is_err());
    assert!(Node::raw_value("bad key!").is_err());
    assert!(Node::conditional("bad condition!", vec![]).is_err());
    assert!(Node::each("bad item!", "collections", vec![]).is_err());
    assert!(Node::each("item", "bad collection!", vec![]).is_err());
    assert!(ElementNode::new("bad tag!", vec![]).is_err());
    assert!(AttributeNode::static_value("bad attr!", "x").is_err());
    assert!(AttributeNode::dynamic_text("bad attr!", "render_key").is_err());
}

#[test]
fn static_attributes_allow_empty_values() {
    let attribute = AttributeNode::static_value("alt", "").unwrap();
    assert_eq!(attribute.value, AttributeValue::Static(String::new()));
}

#[test]
fn parser_and_runtime_accept_empty_and_boolean_html_attributes() {
    let parser = TemplateSourceParser::new();
    let template = parser
        .parse_fragment(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("html-attrs").unwrap(),
            r#"
<section coil:fragment="html-attrs" xmlns:coil="https://coil.rs">
  <img src="/hero.png" alt="" />
  <script src="/theme/assets/site.js" defer></script>
  <option selected>Featured</option>
</section>
"#,
        )
        .unwrap();

    let mut registry = TemplateRegistry::new();
    registry.register(template).unwrap();

    let html = TemplateRuntime::new(registry)
        .render_fragment(
            &[TemplateNamespace::new("core").unwrap()],
            FragmentRenderRequest::new(selector("html-attrs"), RenderModel::new()),
        )
        .unwrap()
        .html;

    assert!(html.contains(r#"alt="""#), "{html}");
    assert!(html.contains(r#"defer="""#), "{html}");
    assert!(html.contains(r#"selected="""#), "{html}");
}

#[test]
fn render_values_support_bool_and_list_types() {
    let bool_model = RenderModel::new()
        .with_bool("enabled", true)
        .unwrap()
        .with_value("flag_text", RenderValue::text("on"))
        .unwrap();
    let list_model = RenderModel::new()
        .with_list(
            "items",
            vec![
                RenderModel::new()
                    .with_value("name", RenderValue::text("One"))
                    .unwrap(),
            ],
        )
        .unwrap();

    assert_eq!(bool_model.get("enabled").unwrap().render_html(), "true");
    assert_eq!(bool_model.get("flag_text").unwrap().render_html(), "on");
    assert_eq!(list_model.get("items").unwrap().render_html(), "");
    assert_eq!(
        bool_model
            .get("enabled")
            .unwrap()
            .as_bool("enabled")
            .unwrap(),
        true
    );
    assert_eq!(
        bool_model
            .get("flag_text")
            .unwrap()
            .as_bool("flag_text")
            .is_err(),
        true
    );
    assert_eq!(
        list_model
            .get("items")
            .unwrap()
            .as_list("items")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn asset_expressions_resolve_through_the_render_model_manifest_map() {
    let mut registry = TemplateRegistry::new();
    registry
        .register(TemplateDefinition::layout(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("shell").unwrap(),
            vec![Node::Element(
                ElementNode::new("link", Vec::new())
                    .unwrap()
                    .with_attribute(AttributeNode::static_value("rel", "stylesheet").unwrap())
                    .with_attribute(
                        AttributeNode::dynamic_expression(
                            "href",
                            TemplateExpression::AssetPath("theme/assets/site.css".to_string()),
                        )
                        .unwrap(),
                    ),
            )],
        ))
        .unwrap();

    let html = TemplateRuntime::new(registry)
        .render_document(
            &[TemplateNamespace::new("core").unwrap()],
            DocumentRenderRequest::new(selector("shell"), model_with_assets()),
        )
        .unwrap()
        .html;

    assert!(html.contains("https://cdn.example.com/theme/assets/site.abc123.css"));
    assert!(!html.contains("href=\"theme/assets/site.css\""));
}

#[test]
fn translation_expressions_render_locale_aware_text_from_the_model() {
    let parser = TemplateSourceParser::new();
    let template = parser
        .parse_fragment(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("home").unwrap(),
            r#"
<section coil:fragment="home" xmlns:coil="https://coil.rs">
  <h1 coil:t="home.title">Fallback</h1>
  <a coil:title="${t('home.cta')}" coil:t="home.cta">Fallback</a>
</section>
"#,
        )
        .unwrap();

    let mut registry = TemplateRegistry::new();
    registry.register(template).unwrap();

    let html = TemplateRuntime::new(registry)
        .render_fragment(
            &[TemplateNamespace::new("core").unwrap()],
            FragmentRenderRequest::new(selector("home"), model_with_translations()),
        )
        .unwrap()
        .html;

    assert!(html.contains("<h1>Bonjour &amp; bienvenue</h1>"));
    assert!(html.contains("title=\"Voir les nouveautes\""));
    assert!(html.contains(">Voir les nouveautes</a>"));
}

#[test]
fn comparison_expressions_render_and_drive_conditionals() {
    let parser = TemplateSourceParser::new();
    let template = parser
        .parse_fragment(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("comparison").unwrap(),
            r#"
<section coil:fragment="comparison" xmlns:coil="https://coil.rs">
  <span class="flag" coil:text="${headline == 'Book & Save'}">false</span>
  <strong coil:if="${headline == 'Book & Save'}">matched</strong>
  <em coil:unless="${headline != 'Book & Save'}">same</em>
</section>
"#,
        )
        .unwrap();

    let mut registry = TemplateRegistry::new();
    registry.register(template).unwrap();

    let html = TemplateRuntime::new(registry)
        .render_fragment(
            &[TemplateNamespace::new("core").unwrap()],
            FragmentRenderRequest::new(selector("comparison"), model()),
        )
        .unwrap()
        .html;

    assert!(html.contains(r#"<span class="flag">true</span>"#), "{html}");
    assert!(html.contains("<strong>matched</strong>"), "{html}");
    assert!(html.contains("<em>same</em>"), "{html}");
}

#[test]
fn thymeleaf_style_operators_render_with_expected_precedence() {
    let parser = TemplateSourceParser::new();
    let template = parser
        .parse_fragment(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("operators").unwrap(),
            r#"
<section coil:fragment="operators" xmlns:coil="https://coil.rs">
  <span class="gt" coil:text="${headline gt 'A'}">false</span>
  <span class="lt" coil:text="${headline lt 'Zzz'}">false</span>
  <span class="ge" coil:text="${headline ge 'Book & Save'}">false</span>
  <span class="le" coil:text="${headline le 'Book & Save'}">false</span>
  <span class="eq" coil:text="${headline eq 'Book & Save'}">false</span>
  <span class="ne" coil:text="${headline ne 'Else'}">false</span>
  <span class="neq" coil:text="${headline neq 'Else'}">false</span>
  <span class="not-bang" coil:text="${!is_archived}">false</span>
  <span class="not-word" coil:text="${not is_archived}">false</span>
  <span class="and-word" coil:text="${headline eq 'Book & Save' and !is_archived}">false</span>
  <span class="or-word" coil:text="${headline eq 'Else' or featured}">false</span>
  <span class="elvis" coil:text="${optional_subtitle ?: 'Fallback subtitle'}">missing</span>
  <span class="ternary" coil:text="${featured ? 'featured' : 'standard'}">missing</span>
  <span class="paren" coil:text="${(headline eq 'Else' or featured) and not is_archived}">false</span>
</section>
"#,
        )
        .unwrap();

    let mut registry = TemplateRegistry::new();
    registry.register(template).unwrap();

    let mut model = model()
        .with_value("is_archived", RenderValue::bool(false))
        .unwrap()
        .with_value("featured", RenderValue::bool(true))
        .unwrap();
    model = model.with_value("optional_subtitle", RenderValue::text("")).unwrap();

    let html = TemplateRuntime::new(registry)
        .render_fragment(
            &[TemplateNamespace::new("core").unwrap()],
            FragmentRenderRequest::new(selector("operators"), model),
        )
        .unwrap()
        .html;

    for class in [
        "gt",
        "lt",
        "ge",
        "le",
        "eq",
        "ne",
        "neq",
        "not-bang",
        "not-word",
        "and-word",
        "or-word",
        "paren",
    ] {
        assert!(
            html.contains(&format!(r#"<span class="{class}">true</span>"#)),
            "{class}: {html}"
        );
    }
    assert!(
        html.contains(r#"<span class="elvis">Fallback subtitle</span>"#),
        "{html}"
    );
    assert!(
        html.contains(r#"<span class="ternary">featured</span>"#),
        "{html}"
    );
}

#[test]
fn switch_case_renders_matching_branch_and_default() {
    let parser = TemplateSourceParser::new();
    let template = parser
        .parse_fragment(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("switcher").unwrap(),
            r#"
<div coil:fragment="switcher" xmlns:coil="https://coil.rs">
  <div coil:switch="${headline}">
    <section coil:case="'Miss'">miss</section>
    <section coil:case="'Book & Save'">match</section>
    <section coil:default>default</section>
  </div>
</div>
"#,
        )
        .unwrap();

    let mut registry = TemplateRegistry::new();
    registry.register(template).unwrap();

    let html = TemplateRuntime::new(registry)
        .render_fragment(
            &[TemplateNamespace::new("core").unwrap()],
            FragmentRenderRequest::new(selector("switcher"), model()),
        )
        .unwrap()
        .html;

    assert!(html.contains("<section>match</section>"), "{html}");
    assert!(!html.contains("<section>default</section>"), "{html}");
}

#[test]
fn block_dispatch_exposes_variant_booleans_and_fragment_paths() {
    let parser = TemplateSourceParser::new();
    let template = parser
        .parse_layout(
            TemplateNamespace::new("customer-app").unwrap(),
            TemplateName::new("pages/home").unwrap(),
            r#"
<!doctype html>
<html xmlns:coil="https://coil.rs">
<body>
  <main>
    <div coil:each="block : ${blocks}">
      <section coil:if="${block.is_hero_section}" class="marker" coil:text="${block.fields.title}">title</section>
      <coil:block coil:replace-fragment="${block.render_fragment}"></coil:block>
    </div>
  </main>
</body>
</html>
"#,
        )
        .unwrap();
    let hero_fragment = parser
        .parse_fragment(
            TemplateNamespace::new("customer-app").unwrap(),
            TemplateName::new("pages/home/blocks/hero_section").unwrap(),
            r#"
<section coil:fragment="hero_section" xmlns:coil="https://coil.rs" class="hero">
  <h2 coil:text="${block.fields.title}">Hero</h2>
</section>
"#,
        )
        .unwrap();
    let featured_fragment = parser
        .parse_fragment(
            TemplateNamespace::new("customer-app").unwrap(),
            TemplateName::new("pages/home/blocks/featured_events").unwrap(),
            r#"
<section coil:fragment="featured_events" xmlns:coil="https://coil.rs" class="events">
  <h2 coil:text="${block.fields.title}">Events</h2>
</section>
"#,
        )
        .unwrap();

    let mut registry = TemplateRegistry::new();
    registry.register(template).unwrap();
    registry.register(hero_fragment).unwrap();
    registry.register(featured_fragment).unwrap();

    let html = TemplateRuntime::new(registry)
        .render_document(
            &[TemplateNamespace::new("customer-app").unwrap()],
            DocumentRenderRequest::new(selector("pages/home"), block_model()),
        )
        .unwrap()
        .html;

    assert!(html.contains(r#"<section class="marker">Hero title</section>"#), "{html}");
    assert!(html.contains(r#"<section class="hero"><h2>Hero title</h2></section>"#), "{html}");
    assert!(html.contains(r#"<section class="events"><h2>Featured events</h2></section>"#), "{html}");
}

#[test]
fn runtime_block_dispatch_binds_nested_block_object_fields() {
    let entry = RenderModel::new()
        .with_value("type", RenderValue::text("hero_section"))
        .unwrap()
        .with_object(
            "fields",
            RenderModel::new()
                .with_value("title", RenderValue::text("Hero title"))
                .unwrap(),
        )
        .unwrap();
    let current_template = TemplateName::new("pages/home").unwrap();
    let dispatch_entry =
        crate::runtime::augment_block_dispatch(&entry, &current_template, &[String::from("hero_section")])
            .unwrap();
    let loop_model = RenderModel::new()
        .merged_with(&dispatch_entry)
        .with_object("block", dispatch_entry)
        .unwrap();

    assert_eq!(
        loop_model
            .get_path("block.is_hero_section")
            .and_then(|value| value.as_bool("block.is_hero_section").ok()),
        Some(true)
    );
    assert_eq!(
        loop_model
            .get_path("block.render_fragment")
            .and_then(|value| value.as_text("block.render_fragment").ok()),
        Some("pages/home/blocks/hero_section")
    );
}

#[test]
fn missing_translation_keys_fail_closed_at_render_time() {
    let parser = TemplateSourceParser::new();
    let template = parser
        .parse_fragment(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("home").unwrap(),
            r#"<h1 coil:fragment="home" xmlns:coil="https://coil.rs" coil:t="home.title">Fallback</h1>"#,
        )
        .unwrap();

    let mut registry = TemplateRegistry::new();
    registry.register(template).unwrap();

    assert_eq!(
        TemplateRuntime::new(registry)
            .render_fragment(
                &[TemplateNamespace::new("core").unwrap()],
                FragmentRenderRequest::new(selector("home"), RenderModel::new()),
            )
            .unwrap_err(),
        TemplateModelError::MissingTranslation {
            key: "home.title".to_string(),
        }
    );
}

#[test]
fn document_rendering_accepts_inline_slot_nodes_and_uses_fallbacks_only_when_needed() {
    let mut registry = TemplateRegistry::new();
    registry
        .register(TemplateDefinition::layout(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("shell").unwrap(),
            vec![
                Node::static_text("<!DOCTYPE html>"),
                Node::Element(
                    ElementNode::new(
                        "html",
                        vec![Node::Element(
                            ElementNode::new(
                                "body",
                                vec![
                                    Node::Slot(
                                        SlotNode::new(SlotName::new("hero").unwrap())
                                            .with_fallback(vec![Node::static_text(
                                                "<div class=\"fallback\">Fallback</div>",
                                            )]),
                                    ),
                                    Node::Element(
                                        ElementNode::new(
                                            "main",
                                            vec![Node::Slot(SlotNode::new(
                                                SlotName::new("content").unwrap(),
                                            ))],
                                        )
                                        .unwrap(),
                                    ),
                                ],
                            )
                            .unwrap(),
                        )],
                    )
                    .unwrap(),
                ),
            ],
        ))
        .unwrap();

    let runtime = TemplateRuntime::new(registry);
    let output = runtime
        .render_document(
            &[TemplateNamespace::new("core").unwrap()],
            DocumentRenderRequest::new(selector("shell"), RenderModel::new())
                .with_slot_fill(
                    SlotName::new("hero").unwrap(),
                    SlotFill::Nodes(vec![Node::Element(
                        ElementNode::new("section", vec![Node::static_text("Inline Hero")])
                            .unwrap(),
                    )]),
                )
                .with_slot_fill(
                    SlotName::new("content").unwrap(),
                    SlotFill::Nodes(vec![Node::Element(
                        ElementNode::new("p", vec![Node::static_text("Inline body")]).unwrap(),
                    )]),
                ),
        )
        .unwrap();

    assert!(output.html.contains("<section>Inline Hero</section>"));
    assert!(output.html.contains("<main><p>Inline body</p></main>"));
    assert!(!output.html.contains("Fallback"));
}

#[test]
fn document_rendering_rejects_fragment_templates_as_layouts() {
    let mut registry = TemplateRegistry::new();
    registry
        .register(TemplateDefinition::fragment(
            TemplateNamespace::new("events").unwrap(),
            TemplateName::new("hero").unwrap(),
            vec![Node::static_text("<section>Hero</section>")],
        ))
        .unwrap();

    let runtime = TemplateRuntime::new(registry);

    assert_eq!(
        runtime
            .render_document(
                &[TemplateNamespace::new("events").unwrap()],
                DocumentRenderRequest::new(selector("hero"), RenderModel::new()),
            )
            .unwrap_err(),
        TemplateModelError::TemplateKindMismatch {
            name: TemplateName::new("hero").unwrap(),
            expected: TemplateKind::Layout,
            actual: TemplateKind::Fragment,
        }
    );
}

#[test]
fn conditional_and_each_nodes_render_using_bool_and_list_values() {
    let mut registry = TemplateRegistry::new();
    registry
        .register(TemplateDefinition::fragment(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("collections").unwrap(),
            vec![Node::Element(
                ElementNode::new(
                    "section",
                    vec![
                        Node::conditional(
                            "featured",
                            vec![Node::Element(
                                ElementNode::new("h2", vec![Node::static_text("Featured")])
                                    .unwrap(),
                            )],
                        )
                        .unwrap(),
                        Node::Element(
                            ElementNode::new(
                                "ul",
                                vec![
                                    Node::each(
                                        "collection",
                                        "featured_collections",
                                        vec![Node::Element(
                                            ElementNode::new(
                                                "li",
                                                vec![Node::Element(
                                                    ElementNode::new(
                                                        "a",
                                                        vec![Node::value("name").unwrap()],
                                                    )
                                                    .unwrap(),
                                                )],
                                            )
                                            .unwrap()
                                            .with_attribute(
                                                AttributeNode::dynamic_text("href", "url").unwrap(),
                                            ),
                                        )],
                                    )
                                    .unwrap(),
                                ],
                            )
                            .unwrap(),
                        ),
                    ],
                )
                .unwrap(),
            )],
        ))
        .unwrap();

    let runtime = TemplateRuntime::new(registry);
    let collections = vec![
        RenderModel::new()
            .with_value("name", RenderValue::text("Spring"))
            .unwrap()
            .with_value("url", RenderValue::text("/collections/spring"))
            .unwrap(),
        RenderModel::new()
            .with_value("name", RenderValue::text("Summer"))
            .unwrap()
            .with_value("url", RenderValue::text("/collections/summer"))
            .unwrap(),
    ];

    let output = runtime
        .render_fragment(
            &[TemplateNamespace::new("core").unwrap()],
            FragmentRenderRequest::new(
                selector("collections"),
                RenderModel::new()
                    .with_bool("featured", true)
                    .unwrap()
                    .with_list("featured_collections", collections)
                    .unwrap(),
            ),
        )
        .unwrap();

    assert!(output.html.contains("<h2>Featured</h2>"));
    assert!(output.html.contains("href=\"/collections/spring\""));
    assert!(output.html.contains("href=\"/collections/summer\""));
    assert!(output.html.contains("Spring"));
    assert!(output.html.contains("Summer"));
}

#[test]
fn document_rendering_composes_layout_slots_and_fragments() {
    let runtime = TemplateRuntime::new(base_registry());
    let output = runtime
        .render_document(
            &namespaces(),
            DocumentRenderRequest::new(selector("storefront.layout"), model())
                .with_slot_fill(
                    SlotName::new("hero").unwrap(),
                    SlotFill::Template(selector("hero")),
                )
                .with_slot_fill(
                    SlotName::new("content").unwrap(),
                    SlotFill::Template(selector("booking.panel")),
                ),
        )
        .unwrap();

    assert!(output.html.starts_with("<!DOCTYPE html><html><body>"));
    assert!(
        output
            .html
            .contains("<section class=\"hero customer\"><h1>Branded Hero</h1></section>")
    );
    assert!(output.html.contains("<main><div data-fragment=\"booking\" class=\"primary&quot; onclick=&quot;oops\">Event &lt;Launch&gt;</div></main>"));
}

#[test]
fn fragment_rendering_reuses_same_fragment_for_partial_output() {
    let runtime = TemplateRuntime::new(base_registry());
    let output = runtime
        .render_fragment(
            &namespaces(),
            FragmentRenderRequest::new(selector("booking.panel"), model()),
        )
        .unwrap();

    assert_eq!(
        output.html,
        "<div data-fragment=\"booking\" class=\"primary&quot; onclick=&quot;oops\">Event &lt;Launch&gt;</div>"
    );
}

#[test]
fn dynamic_values_escape_html_by_default_and_trusted_html_is_explicit() {
    let runtime = TemplateRuntime::new(base_registry());
    let output = runtime
        .render_fragment(
            &namespaces(),
            FragmentRenderRequest::new(selector("hero"), model()),
        )
        .unwrap();

    assert!(output.html.contains("Branded Hero"));
    let event_output = runtime
        .render_fragment(
            &[TemplateNamespace::new("events").unwrap()],
            FragmentRenderRequest::new(selector("hero"), model()),
        )
        .unwrap();
    assert!(event_output.html.contains("Book &amp; Save"));
    assert!(
        event_output
            .html
            .contains("<strong class=\"badge\">Live</strong>")
    );
}

#[test]
fn namespace_resolution_prefers_customer_app_over_module_templates() {
    let runtime = TemplateRuntime::new(base_registry());

    let output = runtime
        .render_fragment(
            &namespaces(),
            FragmentRenderRequest::new(selector("hero"), model()),
        )
        .unwrap();

    assert!(output.html.contains("hero customer"));
    assert!(!output.html.contains("Book &amp; Save"));
}

#[test]
fn fragment_includes_resolve_through_namespace_precedence() {
    let mut registry = base_registry();
    registry
        .register(TemplateDefinition::fragment(
            TemplateNamespace::new("customer-app").unwrap(),
            TemplateName::new("page").unwrap(),
            vec![Node::Include(selector("hero"))],
        ))
        .unwrap();
    registry
        .register(TemplateDefinition::fragment(
            TemplateNamespace::new("events").unwrap(),
            TemplateName::new("page").unwrap(),
            vec![Node::Include(selector("hero"))],
        ))
        .unwrap();

    let runtime = TemplateRuntime::new(registry);
    let output = runtime
        .render_fragment(
            &namespaces(),
            FragmentRenderRequest::new(selector("page"), model()),
        )
        .unwrap();

    assert!(output.html.contains("hero customer"));
    assert!(!output.html.contains("Book &amp; Save"));
}

#[test]
fn layouts_cannot_be_rendered_as_fragments() {
    let runtime = TemplateRuntime::new(base_registry());

    assert_eq!(
        runtime
            .render_fragment(
                &namespaces(),
                FragmentRenderRequest::new(selector("storefront.layout"), model()),
            )
            .unwrap_err(),
        TemplateModelError::FragmentCannotRenderLayout {
            name: TemplateName::new("storefront.layout").unwrap(),
        }
    );
}

#[test]
fn includes_cannot_pull_in_layout_templates() {
    let mut registry = TemplateRegistry::new();
    registry
        .register(TemplateDefinition::layout(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("shell").unwrap(),
            vec![Node::static_text("<!DOCTYPE html>")],
        ))
        .unwrap();
    registry
        .register(TemplateDefinition::fragment(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("page").unwrap(),
            vec![Node::Include(selector("shell"))],
        ))
        .unwrap();

    let runtime = TemplateRuntime::new(registry);

    assert_eq!(
        runtime
            .render_fragment(
                &[TemplateNamespace::new("core").unwrap()],
                FragmentRenderRequest::new(selector("page"), RenderModel::new()),
            )
            .unwrap_err(),
        TemplateModelError::LayoutCannotBeIncludedAsFragment {
            name: TemplateName::new("shell").unwrap(),
        }
    );
}

#[test]
fn missing_slot_without_fallback_is_rejected() {
    let mut registry = TemplateRegistry::new();
    registry
        .register(TemplateDefinition::layout(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("minimal.layout").unwrap(),
            vec![Node::Slot(SlotNode::new(SlotName::new("content").unwrap()))],
        ))
        .unwrap();
    let runtime = TemplateRuntime::new(registry);

    assert_eq!(
        runtime
            .render_document(
                &[TemplateNamespace::new("core").unwrap()],
                DocumentRenderRequest::new(selector("minimal.layout"), RenderModel::new()),
            )
            .unwrap_err(),
        TemplateModelError::MissingSlotFill {
            slot: SlotName::new("content").unwrap(),
        }
    );
}

#[test]
fn template_source_parser_handles_thymeleaf_style_fragments_layouts_slots_and_directives() {
    let parser = TemplateSourceParser::new();

    let navigation = parser
        .parse_fragment(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("navigation.primary").unwrap(),
            r#"
<nav coil:fragment="primary">
  <ul class="nav">
    <li><a coil:attr="href=${nav_featured_href}" coil:text="${nav_featured}">Featured</a></li>
    <li><a coil:attr="href=${nav_events_href}" coil:text="${nav_events}">Events</a></li>
  </ul>
</nav>
"#,
        )
        .unwrap();

    let hero = parser
        .parse_auto(
            TemplateNamespace::new("events").unwrap(),
            TemplateName::new("hero").unwrap(),
            r#"
<section coil:fragment="hero" class="hero" coil:with="headline='Branded Hero', show=true">
  </?coil:block coil:if="${show}">
    <h1 coil:text="${headline}">Fallback</h1>
  </?coil:block>
  <p coil:utext="${trusted_badge}"></p>
  <a coil:attr="href=${featured_href},class=${cta_class}">Shop featured</a>
</section>
"#,
        )
        .unwrap();

    let layout = parser
        .parse_layout(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("storefront.layout").unwrap(),
            r#"
<!doctype html>
<html coil:attr="lang=${locale}">
  <body class="harbor">
    <header>
      <nav coil:replace="navigation.primary"></nav>
    </header>
    <main coil:slot="content">
      <p>Fallback content</p>
    </main>
  </body>
</html>
"#,
        )
        .unwrap();

    assert_eq!(navigation.kind, TemplateKind::Fragment);
    assert_eq!(hero.kind, TemplateKind::Fragment);
    assert_eq!(layout.kind, TemplateKind::Layout);

    let mut registry = TemplateRegistry::new();
    registry.register(navigation).unwrap();
    registry.register(hero).unwrap();
    registry.register(layout).unwrap();

    let runtime = TemplateRuntime::new(registry);
    let output = runtime
        .render_document(
            &[
                TemplateNamespace::new("core").unwrap(),
                TemplateNamespace::new("events").unwrap(),
            ],
            DocumentRenderRequest::new(
                selector("storefront.layout"),
                RenderModel::new()
                    .with_value("locale", RenderValue::text("en-GB"))
                    .unwrap()
                    .with_value("nav_featured", RenderValue::text("Featured"))
                    .unwrap()
                    .with_value(
                        "nav_featured_href",
                        RenderValue::text("/collections/featured"),
                    )
                    .unwrap()
                    .with_value("nav_events", RenderValue::text("Events"))
                    .unwrap()
                    .with_value("nav_events_href", RenderValue::text("/events"))
                    .unwrap()
                    .with_value(
                        "trusted_badge",
                        RenderValue::trusted_html(
                            TrustedHtml::new("<strong>Live</strong>").unwrap(),
                        ),
                    )
                    .unwrap()
                    .with_value("featured_href", RenderValue::text("/collections/featured"))
                    .unwrap()
                    .with_value("cta_class", RenderValue::text("primary"))
                    .unwrap(),
            )
            .with_slot_fill(
                SlotName::new("content").unwrap(),
                SlotFill::Template(selector("hero")),
            ),
        )
        .unwrap();

    assert!(output.html.contains("<!DOCTYPE html>"));
    assert!(output.html.contains("<html lang=\"en-GB\""));
    assert!(output.html.contains("<header>"));
    assert!(output.html.contains("<ul class=\"nav\">"));
    assert!(output.html.contains("<h1>Branded Hero</h1>"));
    assert!(output.html.contains("<p>"));
    assert!(
        output
            .html
            .contains("<a href=\"/collections/featured\" class=\"primary\">Shop featured</a>")
    );
    assert!(!output.html.contains("coil:block"));
}

#[test]
fn template_source_parser_escapes_by_default_and_renders_each_with_nested_objects() {
    let parser = TemplateSourceParser::new();
    let template = parser
        .parse_fragment(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("collections.grid").unwrap(),
            r#"
<section coil:fragment="grid" coil:if="${featured}">
  <h2 coil:text="${title}">Fallback</h2>
  <ul>
    <li coil:each="collection : ${featured_collections}">
      <a coil:attr="href=${collection.url}" coil:text="${collection.name}">Fallback</a>
    </li>
  </ul>
</section>
"#,
        )
        .unwrap();

    let mut registry = TemplateRegistry::new();
    registry.register(template).unwrap();
    let runtime = TemplateRuntime::new(registry);

    let collections = vec![
        RenderModel::new()
            .with_value("name", RenderValue::text("Spring & Summer"))
            .unwrap()
            .with_value("url", RenderValue::text("/collections/spring"))
            .unwrap(),
        RenderModel::new()
            .with_value("name", RenderValue::text("Autumn <New>"))
            .unwrap()
            .with_value("url", RenderValue::text("/collections/autumn"))
            .unwrap(),
    ];

    let output = runtime
        .render_fragment(
            &[TemplateNamespace::new("core").unwrap()],
            FragmentRenderRequest::new(
                selector("collections.grid"),
                RenderModel::new()
                    .with_bool("featured", true)
                    .unwrap()
                    .with_value("title", RenderValue::text("Featured <Collections>"))
                    .unwrap()
                    .with_list("featured_collections", collections)
                    .unwrap(),
            ),
        )
        .unwrap();

    assert!(
        output
            .html
            .contains("<h2>Featured &lt;Collections&gt;</h2>")
    );
    assert!(output.html.contains("Spring &amp; Summer"));
    assert!(output.html.contains("Autumn &lt;New&gt;"));
    assert!(output.html.contains("href=\"/collections/spring\""));
}

#[test]
fn template_source_parser_reports_parse_errors() {
    let parser = TemplateSourceParser::new();

    let error = parser
        .parse_fragment(
            TemplateNamespace::new("core").unwrap(),
            TemplateName::new("broken").unwrap(),
            r#"<section coil:each="collection"><span></span></section>"#,
        )
        .unwrap_err();

    assert!(matches!(error, TemplateModelError::ParseError { .. }));
}
