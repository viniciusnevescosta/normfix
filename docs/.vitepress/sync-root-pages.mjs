// Publishes repository-root documents as pages of the site.
//
// CHANGELOG.md, CONTRIBUTING.md, and SECURITY.md belong at the repository root,
// where GitHub and every contributor expects them. Copying them in at build
// time keeps the root file the single source of truth: edit it there and the
// site follows. The copies are generated, so they are ignored by Git.

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const docs = dirname(here);
const root = dirname(docs);

/** Root document, destination page, and the title the sidebar shows. */
const PAGES = [
  { from: "CHANGELOG.md", to: "changelog.md" },
  { from: "CONTRIBUTING.md", to: "contributing.md" },
  { from: "SECURITY.md", to: "security.md" },
];

const BANNER =
  "<!-- Generated from the repository root; edit that file, not this copy. -->";

for (const page of PAGES) {
  const source = join(root, page.from);
  const target = join(docs, page.to);
  const body = await readFile(source, "utf8");

  // Repository-relative links resolve on GitHub but not on the site. Rewrite
  // the ones that point at documents the site already publishes.
  const rewritten = body
    .replace(/\]\(docs\/ARCHITECTURE\.md\)/g, "](/ARCHITECTURE)")
    .replace(/\]\(docs\/COMPATIBILITY\.md\)/g, "](/COMPATIBILITY)")
    .replace(/\]\(docs\/LOCALIZATION\.md\)/g, "](/LOCALIZATION)")
    .replace(/\]\(docs\/RELEASING\.md\)/g, "](/RELEASING)")
    .replace(/\]\(CHANGELOG\.md\)/g, "](/changelog)")
    .replace(/\]\(CONTRIBUTING\.md\)/g, "](/contributing)")
    .replace(/\]\(SECURITY\.md\)/g, "](/security)")
    .replace(/\]\(\.\.\/CHANGELOG\.md\)/g, "](/changelog)");

  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, `${BANNER}\n\n${rewritten}`, "utf8");
}

// Keep the file list visible in build output so a missing page is obvious.
console.log(
  `synced ${PAGES.length} root documents into the site: ${PAGES.map((page) => page.to).join(", ")}`,
);
