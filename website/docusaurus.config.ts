import type { Config } from "@docusaurus/types";
import type * as Preset from "@docusaurus/preset-classic";

const config: Config = {
  title: "Davenda",
  tagline: "A highly opinionated Rust web framework for serious products",
  favicon: "img/favicon.svg",
  url: "https://davenda.dev",
  baseUrl: "/",
  organizationName: "davenda",
  projectName: "davenda",
  onBrokenLinks: "throw",
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: "throw",
    },
  },
  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },
  themes: [],
  presets: [
    [
      "classic",
      {
        docs: {
          path: "docs",
          routeBasePath: "docs",
          sidebarPath: "./sidebars.ts",
        },
        blog: false,
        theme: {
          customCss: "./src/css/custom.css",
        },
      } satisfies Preset.Options,
    ],
  ],
  plugins: [
    [
      "@docusaurus/plugin-content-docs",
      {
        id: "architecture",
        path: "../docs/design",
        routeBasePath: "architecture",
        sidebarPath: "./sidebars-architecture.ts",
      },
    ],
  ],
  themeConfig: {
    image: "img/favicon.svg",
    navbar: {
      title: "Davenda",
      logo: {
        alt: "Davenda",
        src: "img/favicon.svg",
      },
      items: [
        { to: "/docs/intro", label: "Docs", position: "left" },
        { to: "/docs/use-cases/shoppr/overview", label: "Shoppr", position: "left" },
        { to: "/docs/use-cases/gitly/overview", label: "Gitly", position: "left" },
        { to: "/architecture/the-problem-we-are-solving", label: "Architecture", position: "left" },
        { href: "https://github.com/davenda/davenda", label: "GitHub", position: "right" },
      ],
    },
    footer: {
      style: "dark",
      links: [
        {
          title: "Docs",
          items: [
            { label: "Get Started", to: "/docs/getting-started/quickstart" },
            { label: "Operations", to: "/docs/operations/build-and-deploy" },
            { label: "Reference", to: "/docs/reference/overview" },
          ],
        },
        {
          title: "Use Cases",
          items: [
            { label: "Shoppr", to: "/docs/use-cases/shoppr/overview" },
            { label: "Gitly", to: "/docs/use-cases/gitly/overview" },
            { label: "Architecture", to: "/architecture/the-problem-we-are-solving" },
          ],
        },
        {
          title: "Community",
          items: [
            { label: "Contributing", to: "/docs/contributing" },
            { label: "Code of Conduct", href: "https://github.com/davenda/davenda/blob/main/CODE_OF_CONDUCT.md" },
            { label: "Security", href: "https://github.com/davenda/davenda/blob/main/SECURITY.md" },
            { label: "Support", href: "https://github.com/davenda/davenda/blob/main/SUPPORT.md" },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Davenda contributors.`,
    },
    colorMode: {
      defaultMode: "light",
      disableSwitch: true,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
