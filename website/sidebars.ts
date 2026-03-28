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
      label: "Core Concepts",
      items: [
        "core-concepts/index",
        "core-concepts/glossary-and-mental-model",
        "core-concepts/customer-root-workspace",
        "core-concepts/runtime-and-module-composition",
        "core-concepts/request-and-render-lifecycle",
        "core-concepts/sites-locales-and-markets",
        "core-concepts/customer-apps-vs-official-modules",
        "core-concepts/themes-rendering-and-assets",
        "core-concepts/internationalization-localization-and-content",
        "core-concepts/accessibility-as-a-platform-contract",
        "core-concepts/seo-and-discoverability",
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
            "use-cases/shoppr/storefront-structure",
            "use-cases/shoppr/catalog-and-merchandising",
            "use-cases/shoppr/custom-pages-and-cms",
            "use-cases/shoppr/sites-locales-and-theme-variants",
            "use-cases/shoppr/linked-rust-backend",
            "use-cases/shoppr/wasm-extensions",
            "use-cases/shoppr/checkout-and-operations",
          ],
        },
        {
          type: "category",
          label: "Gitly",
          items: [
            "use-cases/gitly/overview",
            "use-cases/gitly/product-structure",
            "use-cases/gitly/theming-localization-and-accessibility",
            "use-cases/gitly/api-and-background-work",
          ],
        },
      ],
    },
    {
      type: "category",
      label: "Operations",
      items: [
        "operations/project-organization",
        "operations/build-and-deploy",
        "operations/configuration-and-secrets",
        "operations/observability",
        "operations/jobs-and-schedulers",
        "operations/cache-tls-cutover-and-rollback",
        "operations/troubleshooting",
      ],
    },
    {
      type: "category",
      label: "Reference",
      items: [
        "reference/overview",
        "reference/app-toml",
        "reference/platform-config",
        {
          type: "category",
          label: "Auth",
          items: [
            "reference/auth-overview",
            "reference/auth-zanzibar",
            "reference/auth-schema",
            "reference/auth-packages",
            "reference/custom-auth-schema",
          ],
        },
        "reference/template-language",
        "reference/theme-structure",
        "reference/internationalization",
        "reference/accessibility",
        "reference/seo",
        "reference/modules",
        "reference/composition",
        "reference/customer-vs-wasm",
      ],
    },
    "contributing/index",
  ],
};

export default sidebars;
