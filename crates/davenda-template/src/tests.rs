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
