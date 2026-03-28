import clsx from "clsx";
import Link from "@docusaurus/Link";
import Layout from "@theme/Layout";
import styles from "./index.module.css";

const features = [
  {
    title: "Build products, not plumbing",
    body: "Coil gives Rust teams a coherent product platform: HTML-first rendering, auth, storage, jobs, observability, admin, and customer-app composition in one story.",
  },
  {
    title: "Lead with ecommerce, support any web app",
    body: "Shoppr shows the opinionated ecommerce path. Gitly proves the same platform can power a completely different product shape without turning into a generic soup.",
  },
  {
    title: "Keep the extension boundary sane",
    body: "Customer-owned Rust hooks compile into the product. Third-party extensions stay bounded in WASM. That keeps customization powerful without letting the platform dissolve.",
  },
];

export default function Home(): JSX.Element {
  return (
    <Layout
      title="Coil"
      description="Coil is a highly opinionated Rust web framework for serious web products."
    >
      <main className={styles.page}>
        <section className={styles.hero}>
          <div className={styles.heroCopy}>
            <p className={styles.eyebrow}>Highly Opinionated Rust Web Framework</p>
            <h1>Build serious web products in Rust without inventing your own platform first.</h1>
            <p className={styles.lead}>
              Coil is built for teams shipping ecommerce and content-rich products that need
              scale, safety, strong extension boundaries, and a believable path from local Docker
              development to production operations.
            </p>
            <div className={styles.heroActions}>
              <Link className="button button--primary button--lg" to="/docs/getting-started/quickstart">
                Start With Shoppr
              </Link>
              <Link className="button button--secondary button--lg" to="/architecture/the-problem-we-are-solving">
                Read The Architecture
              </Link>
            </div>
          </div>
          <div className={styles.heroPanel}>
            <p className={styles.panelEyebrow}>Minimal customer app shape</p>
            <pre className={styles.codeBlock}>
{`[dependencies]
coil = "0.1.0"

fn main() -> Result<(), anyhow::Error> {
    coil::builder()
        .with_customer_plugin(shoppr_backend::plugin())
        .run_from_env()
}`}
            </pre>
            <p className={styles.panelHint}>
              Then run your customer app with Docker, local services, and a real docs-backed product
              model instead of hand-assembling every platform concern yourself.
            </p>
          </div>
        </section>

        <section className={styles.stats}>
          <div>
            <strong>HTML-first</strong>
            <span>Server-rendered pages with progressive enhancement layered on.</span>
          </div>
          <div>
            <strong>Multi-site</strong>
            <span>Market-aware routing, inventory scope, and locale handling are first-class.</span>
          </div>
          <div>
            <strong>Linked Rust</strong>
            <span>Customer-owned business logic compiles into the product through a stable SDK.</span>
          </div>
          <div>
            <strong>WASM boundary</strong>
            <span>Third-party extensions stay bounded and operationally safer.</span>
          </div>
        </section>

        <section className={styles.featureGrid}>
          {features.map((feature) => (
            <article key={feature.title} className={styles.card}>
              <h2>{feature.title}</h2>
              <p>{feature.body}</p>
            </article>
          ))}
        </section>

        <section className={styles.compare}>
          <article className={styles.compareCard}>
            <p className={styles.eyebrow}>Ecommerce path</p>
            <h2>Start with Shoppr.</h2>
            <p>
              Learn Coil through a premium, multi-market storefront: catalog, merchandising,
              account flows, checkout, customer-linked Rust, and operator surfaces.
            </p>
            <Link to="/docs/use-cases/shoppr/overview">Explore the Shoppr guide</Link>
          </article>
          <article className={styles.compareCard}>
            <p className={styles.eyebrow}>General web app path</p>
            <h2>Prove the framework with Gitly.</h2>
            <p>
              Switch lenses and see the same platform drive a developer product with mock APIs,
              themes, locales, scheduled jobs, and a non-commerce information architecture.
            </p>
            <Link to="/docs/use-cases/gitly/overview">Explore the Gitly guide</Link>
          </article>
        </section>

        <section className={clsx(styles.featureGrid, styles.bottomGrid)}>
          <article className={styles.card}>
            <p className={styles.eyebrow}>Zero to hero</p>
            <h2>Get productive in minutes.</h2>
            <p>
              The docs are written for Rust web developers who want a fast path from “what is this?”
              to “I can build with this.”
            </p>
          </article>
          <article className={styles.card}>
            <p className={styles.eyebrow}>Six months later</p>
            <h2>Still deep enough when the hard questions arrive.</h2>
            <p>
              Architecture chapters, operations guides, and module reference docs remain available
              when you are debugging production behavior or designing extensions.
            </p>
          </article>
        </section>
      </main>
    </Layout>
  );
}
