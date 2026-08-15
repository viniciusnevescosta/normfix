// A plain-text copy of every documentation page, beside the page itself.
//
// An agent asked to follow one page's instructions should not have to parse a
// rendered site to find them. The site-wide `llms.txt` lists what exists; this
// gives each entry in that list a fetchable body, at the page's own route plus
// `.txt`, so `/docs/guide/getting-started` has `/docs/guide/getting-started.txt`
// with the same words and none of the markup that only matters to a browser.
//
// What is stripped is what carries no meaning outside VitePress: frontmatter,
// container fences, and the attributes it hangs on code blocks. What is kept is
// everything a reader would have read.

import { readFileSync, writeFileSync, mkdirSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const docsRoot = join(here, "..");
const outputRoot = join(docsRoot, "..", "web", "dist", "docs");

/** Directories that hold no published page. */
const SKIPPED = new Set([".vitepress", "node_modules", "public"]);

function markdownFiles(directory) {
  const found = [];
  for (const entry of readdirSync(directory)) {
    if (SKIPPED.has(entry)) continue;
    const full = join(directory, entry);
    if (statSync(full).isDirectory()) {
      found.push(...markdownFiles(full));
    } else if (entry.endsWith(".md")) {
      found.push(full);
    }
  }
  return found;
}

/**
 * The page as prose.
 *
 * The container syntax is the interesting case: `::: warning` opens a callout
 * whose body is ordinary text, so dropping the fence and keeping the body is
 * what preserves the meaning. Dropping the body with it would silently lose
 * exactly the sentences a page marks as most important.
 */
export function plainText(source) {
  const withoutFrontmatter = source.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, "");
  return `${withoutFrontmatter
    .split("\n")
    .filter((line) => !line.trim().startsWith(":::"))
    .join("\n")
    .replace(/^```[a-z]*\s*\[[^\]]*\]\s*$/gim, "```")
    .replace(/\n{3,}/g, "\n\n")
    .trim()}\n`;
}

/** The route a page is published at, without its extension. */
export function routeFor(relativePath) {
  const parts = relativePath.split(sep);
  const last = parts.pop().replace(/\.md$/, "");
  const stem = last === "index" ? "index" : last;
  return [...parts, stem].join("/");
}

function main() {
  const pages = markdownFiles(docsRoot);
  let written = 0;
  for (const page of pages) {
    const route = routeFor(relative(docsRoot, page));
    const target = join(outputRoot, `${route}.txt`);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, plainText(readFileSync(page, "utf8")));
    written += 1;
  }
  console.log(`wrote ${written} plain-text pages into web/dist/docs`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
