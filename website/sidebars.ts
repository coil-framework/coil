import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    "intro",
    {
      type: "category",
      label: "Getting Started",
      items: [
        "getting-started/quickstart",
        "getting-started/customer-project-layout",
        "getting-started/linked-rust-backends",
      ],
    },
    {
      type: "category",
      label: "Use Cases",
      items: [
        {
          type: "category",
          label: "Shoppr",
          items: [
            "use-cases/shoppr/overview",
            "use-cases/shoppr/catalog-and-merchandising",
            "use-cases/shoppr/checkout-and-operations",
          ],
        },
        {
          type: "category",
          label: "Gitly",
          items: [
            "use-cases/gitly/overview",
            "use-cases/gitly/non-commerce-product-shape",
          ],
        },
      ],
    },
    {
      type: "category",
      label: "Operations",
      items: [
        "operations/build-and-deploy",
        "operations/configuration-and-secrets",
        "operations/observability",
      ],
    },
    {
      type: "category",
      label: "Reference",
      items: [
        "reference/overview",
        "reference/modules",
        "reference/composition",
        "reference/customer-vs-wasm",
      ],
    },
    "contributing/index",
  ],
};

export default sidebars;
