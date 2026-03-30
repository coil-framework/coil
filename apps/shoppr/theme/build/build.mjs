import { mkdir, readFile, writeFile } from "node:fs/promises";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import esbuild from "esbuild";
import postcss from "postcss";
import postcssImport from "postcss-import";
import postcssNesting from "postcss-nesting";
import autoprefixer from "autoprefixer";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const appRoot = path.resolve(__dirname, "..", "..");
const frontendRoot = path.join(appRoot, "theme", "frontend");
const assetRoot = path.join(appRoot, "theme", "assets");
const watchMode = process.argv.includes("--watch");

const jsEntries = {
  site: path.join(frontendRoot, "site.ts"),
  admin: path.join(frontendRoot, "admin.ts"),
  "cms-editor": path.join(frontendRoot, "cms-editor.ts"),
};

const cssEntries = {
  site: path.join(frontendRoot, "site.css"),
  admin: path.join(frontendRoot, "admin.css"),
  "cms-editor": path.join(frontendRoot, "cms-editor.css"),
};

async function buildCss() {
  const processor = postcss([postcssImport(), postcssNesting(), autoprefixer()]);
  await mkdir(assetRoot, { recursive: true });

  for (const [name, sourcePath] of Object.entries(cssEntries)) {
    const source = await readFile(sourcePath, "utf8");
    const result = await processor.process(source, {
      from: sourcePath,
      to: path.join(assetRoot, `${name}.css`),
    });
    await writeFile(path.join(assetRoot, `${name}.css`), result.css, "utf8");
  }
}

async function buildJs() {
  await mkdir(assetRoot, { recursive: true });
  return esbuild.build({
    entryPoints: jsEntries,
    outdir: assetRoot,
    bundle: true,
    format: "iife",
    target: "es2020",
    platform: "browser",
    sourcemap: false,
    logLevel: "info",
  });
}

async function buildAll() {
  await Promise.all([buildCss(), buildJs()]);
}

async function watch() {
  await buildCss();

  const context = await esbuild.context({
    entryPoints: jsEntries,
    outdir: assetRoot,
    bundle: true,
    format: "iife",
    target: "es2020",
    platform: "browser",
    sourcemap: "inline",
    logLevel: "info",
  });

  await context.watch();

  let cssTimer = null;
  fs.watch(frontendRoot, { recursive: true }, (eventType, filename) => {
    if (!filename || (!filename.endsWith(".css") && !filename.endsWith(".ts"))) {
      return;
    }
    if (cssTimer) {
      clearTimeout(cssTimer);
    }
    cssTimer = setTimeout(async () => {
      try {
        await buildCss();
        console.log("[shoppr-frontend] CSS rebuilt");
      } catch (error) {
        console.error("[shoppr-frontend] CSS build failed");
        console.error(error);
      }
    }, 75);
  });

  console.log("[shoppr-frontend] watching theme/frontend");
}

if (watchMode) {
  await watch();
} else {
  await buildAll();
}
