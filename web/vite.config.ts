import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { defineConfig, type Plugin } from "vite";

const outputDirectory = resolve(fileURLToPath(new URL(".", import.meta.url)), "dist");

const localizedPages = {
  pt: {
    lang: "pt-BR",
    title: "normfix playground · formatador C para a 42",
    description:
      "Formate C, headers, Makefiles e Markdown da 42 com privacidade no navegador usando WebAssembly.",
    ogLocale: "pt_BR",
  },
  es: {
    lang: "es-ES",
    title: "normfix playground · formateador C para 42",
    description:
      "Formatea C, headers, Makefiles y Markdown de 42 de forma privada en el navegador con WebAssembly.",
    ogLocale: "es_ES",
  },
  fr: {
    lang: "fr-FR",
    title: "normfix playground · formateur C pour 42",
    description:
      "Formatez C, headers, Makefiles et Markdown de 42 en privé dans le navigateur avec WebAssembly.",
    ogLocale: "fr_FR",
  },
} as const;

function replaceMeta(html: string, id: string, attribute: string, value: string): string {
  const pattern = new RegExp(`(<[^>]+id=["']${id}["'][^>]+${attribute}=["'])[^"']*(["'])`);
  return html.replace(pattern, `$1${value}$2`);
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
        const localeDirectory = resolve(outputDirectory, locale);
        await mkdir(localeDirectory, { recursive: true });
        await writeFile(resolve(localeDirectory, "index.html"), localized, "utf8");
      }
    },
  };
}

export default defineConfig({
  base: "/",
  plugins: [localizedPlaygroundPages()],
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
