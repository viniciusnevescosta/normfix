// Publishes the English-only documents inside every locale tree.
//
// These are contributor and process documents: the architecture record, the
// release process, the roadmap, the changelog, and the two files GitHub expects
// at the repository root. They are maintained in English, which is a reasonable
// decision. Sending a Portuguese reader to an English page and flipping the
// entire site's language with it is not. These generated copies
// keep the reader inside their own locale — same navigation, same language
// selector, same URL prefix — and state in their language that the body below
// is English.
//
// The copies are generated, so they are ignored by Git.

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const docs = dirname(here);

const LOCALES = ["pt", "es", "fr"];

/** Source page, and the localized route it is mirrored to. */
const PAGES = [
  "ARCHITECTURE.md",
  "RELEASING.md",
  "ROADMAP.md",
  "changelog.md",
  "contributing.md",
  "security.md",
];

const NOTICE = {
  pt: "Este documento é mantido em inglês. Ele é técnico e muda a cada versão, e uma tradução desatualizada seria pior do que o original. Você continua na versão em português do site: a navegação e o seletor de idioma acima não mudaram.",
  es: "Este documento se mantiene en inglés. Es técnico y cambia con cada versión, y una traducción desactualizada sería peor que el original. Sigues en la versión en español del sitio: la navegación y el selector de idioma de arriba no han cambiado.",
  fr: "Ce document est maintenu en anglais. Il est technique et change à chaque version, et une traduction périmée serait pire que l'original. Vous restez sur la version française du site : la navigation et le sélecteur de langue ci-dessus n'ont pas changé.",
};

const BANNER = "<!-- Generated mirror; edit the English page, not this copy. -->";

for (const page of PAGES) {
  const body = await readFile(join(docs, page), "utf8");
  for (const locale of LOCALES) {
    const target = join(docs, locale, page);
    // Internal links in the mirrored body point at English routes on purpose:
    // they lead to pages that only exist in English.
    const content = `${BANNER}\n\n::: warning\n${NOTICE[locale]}\n:::\n\n${body}`;
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, content, "utf8");
  }
}
