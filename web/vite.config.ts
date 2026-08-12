import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { build, defineConfig, type Plugin } from "vite";

import { offlineShell, type Bundle } from "./src/offline/precache";
import { version } from "./package.json" with { type: "json" };

const projectDirectory = resolve(fileURLToPath(new URL(".", import.meta.url)));
const outputDirectory = resolve(projectDirectory, "dist");

const localizedPages = {
  pt: {
    lang: "pt-BR",
    title: "normfix playground · formatador C para a 42",
    description:
      "Formate C, headers, Makefiles e Markdown da 42 com privacidade no navegador usando WebAssembly.",
    ogLocale: "pt_BR",
    appName: "normfix playground no navegador",
  },
  es: {
    lang: "es-ES",
    title: "normfix playground · formateador C para 42",
    description:
      "Formatea C, headers, Makefiles y Markdown de 42 de forma privada en el navegador con WebAssembly.",
    ogLocale: "es_ES",
    appName: "normfix playground en el navegador",
  },
  fr: {
    lang: "fr-FR",
    title: "normfix playground · formateur C pour 42",
    description:
      "Formatez C, headers, Makefiles et Markdown de 42 en privé dans le navigateur avec WebAssembly.",
    ogLocale: "fr_FR",
    appName: "normfix playground dans le navigateur",
  },
} as const;

function replaceMeta(html: string, id: string, attribute: string, value: string): string {
  const pattern = new RegExp(`(<[^>]+id=["']${id}["'][^>]+${attribute}=["'])[^"']*(["'])`);
  return html.replace(pattern, `$1${value}$2`);
}

/**
 * Shortens what a first visit has to discover before it can work.
 *
 * Two costs were measured rather than guessed. The stylesheet blocked the
 * first render for its own round trip, and the WebAssembly module — the thing
 * the page exists to run — was three hops down the chain: the browser could
 * not know it existed until the entry script had loaded and had in turn loaded
 * the wasm-bindgen glue. Both are fixed here rather than in the source, because
 * both need the content-hashed names this build has just produced.
 */
/**
 * The entry stylesheet, once `criticalPathHints` has inlined it into the HTML.
 *
 * Shared because the service worker must not install a file that nothing will
 * ever request: after inlining, the emitted `.css` is dead weight in the cache.
 */
const inlinedStylesheet = { fileName: "" };

function criticalPathHints(): Plugin {
  let wasm = "";
  let glue = "";
  return {
    name: "normfix-critical-path-hints",
    generateBundle(_options, bundle) {
      for (const [fileName, output] of Object.entries(bundle)) {
        if (output.type === "asset" && fileName.endsWith(".wasm")) wasm = fileName;
        if (output.type !== "chunk") continue;
        // The entry's own stylesheet, not Monaco's: those arrive with the
        // dynamic import that loads them and never block the first render.
        if (output.isEntry) {
          inlinedStylesheet.fileName = [
            ...((output as { viteMetadata?: { importedCss?: Set<string> } })
              .viteMetadata?.importedCss ?? []),
          ][0] ?? "";
        }
        if (Object.keys(output.modules).some((id) => id.endsWith("pkg/normfix_wasm.js"))) {
          glue = fileName;
        }
      }
    },
    async closeBundle() {
      const indexPath = resolve(outputDirectory, "index.html");
      let html = await readFile(indexPath, "utf8");

      const stylesheet = inlinedStylesheet.fileName;
      if (stylesheet) {
        const css = await readFile(resolve(outputDirectory, stylesheet), "utf8");
        const link = new RegExp(`<link[^>]+href="/${stylesheet}"[^>]*>`);
        if (!link.test(html)) throw new Error(`stylesheet link for ${stylesheet} not found`);
        html = html.replace(link, `<style>${css}</style>`);
      }

      // No `crossorigin`: wasm-bindgen fetches the module same-origin with
      // default credentials, and a preload whose mode does not match is not
      // reused — it downloads the file a second time instead.
      const hints = [
        glue && `<link rel="modulepreload" href="/${glue}">`,
        wasm && `<link rel="preload" href="/${wasm}" as="fetch" type="application/wasm">`,
      ].filter(Boolean).join("");
      html = html.replace("</head>", `${hints}</head>`);

      await writeFile(indexPath, html, "utf8");
    },
  };
}

function localizedPlaygroundPages(): Plugin {
  return {
    name: "normfix-localized-playground-pages",
    async closeBundle() {
      // Rolldown finalizes HTML after generateBundle. Copy the finalized page so
      // every localized entry keeps the exact hashed asset URLs from index.html.
      const index = await readFile(resolve(outputDirectory, "index.html"), "utf8");
      for (const [locale, metadata] of Object.entries(localizedPages)) {
        const canonical = `https://normfix.vercel.app/${locale}/`;
        let localized = index
          .replace('<html lang="en">', `<html lang="${metadata.lang}">`)
          .replace(/<title>[^<]*<\/title>/, `<title>${metadata.title}</title>`);
        localized = replaceMeta(localized, "meta-description", "content", metadata.description);
        localized = replaceMeta(localized, "og-title", "content", metadata.title);
        localized = replaceMeta(localized, "og-description", "content", metadata.description);
        localized = replaceMeta(localized, "og-url", "content", canonical);
        localized = replaceMeta(localized, "og-locale", "content", metadata.ogLocale);
        const alternateLocales = ["en_US", "pt_BR", "es_ES", "fr_FR"]
          .filter((candidate) => candidate !== metadata.ogLocale);
        localized = replaceMeta(localized, "og-alternate-one", "content", alternateLocales[0] ?? "en_US");
        localized = replaceMeta(localized, "og-alternate-two", "content", alternateLocales[1] ?? "en_US");
        localized = replaceMeta(localized, "og-alternate-three", "content", alternateLocales[2] ?? "en_US");
        localized = replaceMeta(localized, "twitter-title", "content", metadata.title);
        localized = replaceMeta(localized, "twitter-description", "content", metadata.description);
        localized = replaceMeta(localized, "canonical-url", "href", canonical);
        localized = replaceMeta(localized, "manifest-link", "href", `/${locale}/site.webmanifest`);
        const localeDirectory = resolve(outputDirectory, locale);
        await mkdir(localeDirectory, { recursive: true });
        await writeFile(resolve(localeDirectory, "index.html"), localized, "utf8");
        // An installed app shows its manifest name under the icon, so a reader
        // who installed the Portuguese playground should not find an English
        // label on their home screen.
        const manifest = JSON.parse(
          await readFile(resolve(outputDirectory, "site.webmanifest"), "utf8"),
        ) as Record<string, unknown>;
        await writeFile(
          resolve(localeDirectory, "site.webmanifest"),
          `${JSON.stringify({
            ...manifest,
            id: `/${locale}/`,
            name: metadata.appName,
            description: metadata.description,
            lang: metadata.lang,
            start_url: `/${locale}/`,
          }, null, 2)}\n`,
          "utf8",
        );
      }
    },
  };
}

/**
 * Builds /sw.js from the real output of this build.
 *
 * The precache list is derived from the finished bundle rather than written by
 * hand, because every asset URL carries a content hash: a hand-maintained list
 * would be wrong the first time a chunk changed, and wrong in the worst way —
 * an install that succeeds and caches the previous build.
 */
function offlineServiceWorker(): Plugin {
  let shell: string[] = [];
  return {
    name: "normfix-offline-service-worker",
    generateBundle(_options, bundle) {
      const summary: Record<string, Bundle[string]> = {};
      for (const [fileName, output] of Object.entries(bundle)) {
        summary[fileName] = output.type === "chunk"
          ? {
            type: "chunk",
            isEntry: output.isEntry,
            imports: output.imports,
            moduleIds: Object.keys(output.modules),
            importedCss: [
              ...((output as { viteMetadata?: { importedCss?: Set<string> } })
                .viteMetadata?.importedCss ?? []),
            ],
          }
          : { type: "asset" };
      }
      shell = offlineShell(summary).filter((url) => url !== `/${inlinedStylesheet.fileName}`);
    },
    async closeBundle() {
      // Naming the cache after the shell makes eviction automatic: a build that
      // changes nothing reuses the cache, and any change to a cached file
      // produces a new name that the activate step then cleans up after.
      const { createHash } = await import("node:crypto");
      const digest = createHash("sha256").update(shell.join("\n")).digest("hex").slice(0, 16);
      await build({
        configFile: false,
        logLevel: "warn",
        define: {
          __NORMFIX_CACHE__: JSON.stringify(`normfix-playground-${version}-${digest}`),
          __NORMFIX_PRECACHE__: JSON.stringify(shell),
        },
        build: {
          outDir: outputDirectory,
          emptyOutDir: false,
          target: "baseline-widely-available",
          copyPublicDir: false,
          // A service worker has no import map and no module graph of its own.
          // IIFE output cannot accidentally emit syntax that a classic worker
          // registration would fail to parse.
          lib: {
            entry: resolve(projectDirectory, "src/offline/service-worker.ts"),
            name: "normfixServiceWorker",
            formats: ["iife"],
            fileName: () => "sw.js",
          },
        },
      });
    },
  };
}

export default defineConfig({
  base: "/",
  plugins: [criticalPathHints(), localizedPlaygroundPages(), offlineServiceWorker()],
  build: {
    assetsInlineLimit: 0,
    // Monaco is a desktop-only dynamic import; its two core chunks are large
    // but never downloaded by the mobile textarea path.
    chunkSizeWarningLimit: 750,
    target: "baseline-widely-available",
  },
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
  },
  preview: {
    host: "127.0.0.1",
    port: 4173,
    strictPort: true,
  },
});
