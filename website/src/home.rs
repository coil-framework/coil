use fission::prelude::*;

const PAPER: Color = Color {
    r: 247,
    g: 244,
    b: 237,
    a: 255,
};
const INK: Color = Color {
    r: 24,
    g: 24,
    b: 22,
    a: 255,
};
const MUTED: Color = Color {
    r: 88,
    g: 86,
    b: 80,
    a: 255,
};
const RUST: Color = Color {
    r: 190,
    g: 60,
    b: 22,
    a: 255,
};
#[derive(Clone, Copy)]
pub struct HomePage;

impl From<HomePage> for Widget {
    fn from(_: HomePage) -> Self {
        SemanticsRegion::new(
            Container::new(Column {
                gap: Some(0.0),
                children: widgets![
                    header(),
                    hero(),
                    execution_model(),
                    proof(),
                    closing(),
                    footer()
                ],
                ..Default::default()
            })
            .bg(PAPER),
        )
        .identifier("site-main")
        .into()
    }
}

fn header() -> Widget {
    region(
        "site-header",
        Container::new(Row {
            gap: Some(28.0),
            wrap: ir_op::FlexWrap::Wrap,
            align_items: ir_op::AlignItems::Center,
            children: widgets![
                Text::new("COIL")
                    .size(19.0)
                    .line_height(22.0)
                    .weight(760)
                    .color(INK),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                nav_link("Start", "/docs/getting-started/quickstart/", "start"),
                nav_link("Concepts", "/docs/core-concepts/", "concepts"),
                nav_link(
                    "Architecture",
                    "/architecture/03-product-shape-core-official-modules-customer-apps/",
                    "architecture",
                ),
                nav_link("GitHub", "https://github.com/coil-framework/coil", "github",),
            ],
            ..Default::default()
        })
        .padding([0.0, 0.0, 24.0, 24.0]),
    )
}

fn hero() -> Widget {
    region(
        "site-section:hero",
        Container::new(Column {
            gap: Some(28.0),
            children: widgets![
                Text::new("FISSION-NATIVE RUST PRODUCT FRAMEWORK")
                    .size(12.0)
                    .line_height(16.0)
                    .weight(720)
                    .color(RUST),
                Text::new("Build the product.\nKeep the platform coherent.")
                    .size(72.0)
                    .line_height(76.0)
                    .weight(560)
                    .color(INK)
                    .semantics_identifier("site-heading-1:top"),
                Container::new(
                    Text::new("Coil gives Rust teams one product shape from searchable public pages to rich operational software: Fission rendering and state, Coil domains and production services.")
                        .size(20.0)
                        .line_height(31.0)
                        .color(MUTED),
                )
                .max_width(690.0),
                Row {
                    gap: Some(28.0),
                    wrap: ir_op::FlexWrap::Wrap,
                    align_items: ir_op::AlignItems::Center,
                    children: widgets![
                        primary_link("Start with Shoppr", "/docs/getting-started/quickstart/"),
                        text_link("Read the architecture", "/architecture/03-product-shape-core-official-modules-customer-apps/"),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .padding([0.0, 0.0, 82.0, 82.0]),
    )
}

fn execution_model() -> Widget {
    let stages = [
        (
            "01",
            "SSR",
            "Complete HTML, resolved after typed jobs settle.",
        ),
        (
            "02",
            "ISLANDS",
            "Focused Rust interaction where a region earns it.",
        ),
        (
            "03",
            "WEB",
            "Full Fission applications for editor and operator work.",
        ),
        (
            "04",
            "OPERATIONS",
            "Auth, data, jobs, media, payments, and observability.",
        ),
    ];
    region(
        "site-section:execution",
        Container::new(SimpleGrid {
            min_child_width: 230.0,
            gap: Some(0.0),
            children: stages
                .into_iter()
                .map(|(number, title, body)| stage(number, title, body))
                .collect(),
        })
        .padding([0.0, 0.0, 30.0, 30.0]),
    )
}

fn stage(number: &'static str, title: &'static str, body: &'static str) -> Widget {
    region(
        &format!("site-stage:{title}"),
        Container::new(Column {
            gap: Some(13.0),
            children: widgets![
                Text::new(number).size(11.0).line_height(14.0).color(RUST),
                Text::new(title)
                    .size(25.0)
                    .line_height(28.0)
                    .weight(650)
                    .color(INK),
                Text::new(body).size(15.0).line_height(23.0).color(MUTED),
            ],
            ..Default::default()
        })
        .padding([22.0, 22.0, 0.0, 0.0]),
    )
}

fn proof() -> Widget {
    region(
        "site-section:proof",
        Container::new(SimpleGrid {
            min_child_width: 360.0,
            gap: Some(0.0),
            children: vec![
                proof_column(
                    "PUBLIC PRODUCT SURFACES",
                    "public-product-surfaces",
                    "Pages people can trust before JavaScript arrives.",
                    "Multi-site catalogues, editorial content, events, memberships, accounts and checkout begin as accessible server-rendered HTML. Fission jobs resolve authoritative data before the document is sent.",
                    "Follow the Shoppr build",
                    "/docs/use-cases/shoppr/overview/",
                ),
                proof_column(
                    "OPERATIONAL SURFACES",
                    "operational-surfaces",
                    "Rich tools without a second product architecture.",
                    "CMS editing, merchandising, fulfilment and support can graduate to focused islands or a full Fission Web app while retaining the same actions, reducers, jobs and authorization contracts.",
                    "Understand the runtime boundary",
                    "/docs/core-concepts/runtime-and-module-composition/",
                ),
            ],
        })
        .padding([0.0, 0.0, 74.0, 74.0]),
    )
}

fn proof_column(
    eyebrow: &'static str,
    heading_id: &'static str,
    title: &'static str,
    body: &'static str,
    link: &'static str,
    href: &'static str,
) -> Widget {
    region(
        &format!("site-proof:{eyebrow}"),
        Container::new(Column {
            gap: Some(18.0),
            children: widgets![
                Text::new(eyebrow)
                    .size(11.0)
                    .line_height(14.0)
                    .weight(720)
                    .color(RUST),
                Text::new(title)
                    .size(36.0)
                    .line_height(42.0)
                    .weight(560)
                    .color(INK)
                    .semantics_identifier(format!("site-heading-2:{heading_id}")),
                Text::new(body).size(17.0).line_height(27.0).color(MUTED),
                text_link(link, href),
            ],
            ..Default::default()
        })
        .padding([36.0, 36.0, 0.0, 0.0]),
    )
}

fn closing() -> Widget {
    region(
        "site-section:closing",
        Container::new(Row {
            gap: Some(32.0),
            wrap: ir_op::FlexWrap::Wrap,
            align_items: ir_op::AlignItems::Center,
            children: widgets![
                Container::new(
                    Text::new("Own the product logic. Reuse the platform discipline.")
                        .size(28.0)
                        .line_height(35.0)
                        .weight(560)
                        .color(INK),
                )
                .flex_grow(1.0),
                primary_link("Start building", "/docs/getting-started/quickstart/"),
            ],
            ..Default::default()
        })
        .padding([0.0, 0.0, 38.0, 38.0]),
    )
}

fn footer() -> Widget {
    region(
        "site-footer",
        Container::new(Row {
            gap: Some(22.0),
            wrap: ir_op::FlexWrap::Wrap,
            children: widgets![
                Text::new("COIL").size(13.0).weight(720).color(INK),
                Text::new("Fission-native Rust product framework")
                    .size(13.0)
                    .color(MUTED),
                Spacer {
                    flex_grow: 1.0,
                    ..Default::default()
                },
                text_link("Docs", "/docs/intro/"),
                text_link("GitHub", "https://github.com/coil-framework/coil"),
            ],
            ..Default::default()
        })
        .padding([0.0, 0.0, 24.0, 24.0]),
    )
}

fn primary_link(label: &'static str, href: &'static str) -> Widget {
    SemanticsRegion::new(
        Pressable::new(
            Container::new(
                Text::new(label)
                    .size(15.0)
                    .line_height(18.0)
                    .weight(680)
                    .color(PAPER),
            )
            .padding([22.0, 22.0, 14.0, 14.0])
            .bg(RUST)
            .border_radius(3.0),
        )
        .label(label)
        .href(href),
    )
    .identifier("site-primary-link")
    .into()
}

fn text_link(label: &'static str, href: &'static str) -> Widget {
    SemanticsRegion::new(Link::to(label, href))
        .identifier("site-text-link")
        .into()
}

fn nav_link(label: &'static str, href: &'static str, identity: &'static str) -> Widget {
    SemanticsRegion::new(Link::to(label, href))
        .identifier(format!("site-nav-link:{identity}"))
        .into()
}

fn region(identifier: &str, child: impl Into<Widget>) -> Widget {
    SemanticsRegion::new(child).identifier(identifier).into()
}
