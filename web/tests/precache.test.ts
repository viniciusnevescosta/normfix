import assert from "node:assert/strict";
import { test } from "vitest";

import {
  CACHE_MATCH_OPTIONS,
  EXTRA_SHELL_PATHS,
  PAGE_PATHS,
  offlineShell,
  pagePathFor,
  staticClosure,
  strategyFor,
  type Bundle,
} from "../src/offline/precache";

/** A bundle shaped like a real build: an entry, its CSS, a dynamic editor, the WASM. */
const bundle: Bundle = {
  "assets/index-aaa.js": {
    type: "chunk",
    isEntry: true,
    imports: ["assets/shared-bbb.js"],
    importedCss: ["assets/index-ccc.css"],
    moduleIds: ["/repo/web/src/main.ts"],
  },
  "assets/shared-bbb.js": { type: "chunk", imports: [], moduleIds: ["/repo/web/src/i18n.ts"] },
  "assets/index-ccc.css": { type: "asset" },
  "assets/wasm-glue-ddd.js": {
    type: "chunk",
    imports: ["assets/shared-bbb.js"],
    moduleIds: ["/repo/web/pkg/normfix_wasm.js"],
  },
  "assets/normfix_wasm_bg-eee.wasm": { type: "asset" },
  "assets/codeEditorWidget-fff.js": {
    type: "chunk",
    imports: ["assets/monaco-core-ggg.js"],
    moduleIds: ["/repo/web/node_modules/monaco-editor/editor.js"],
  },
  "assets/monaco-core-ggg.js": { type: "chunk", imports: [] },
};

test("the offline shell carries everything a first cold start needs", () => {
  const shell = offlineShell(bundle);

  for (const page of PAGE_PATHS) assert.ok(shell.includes(page), page);
  for (const extra of EXTRA_SHELL_PATHS) assert.ok(shell.includes(extra), extra);
  assert.ok(shell.includes("/assets/index-aaa.js"));
  assert.ok(shell.includes("/assets/shared-bbb.js"));
  assert.ok(shell.includes("/assets/index-ccc.css"));
});

test("the formatter is cached even though it is imported dynamically", () => {
  const shell = offlineShell(bundle);

  // A playground that opens offline but cannot format is not worth installing.
  assert.ok(shell.includes("/assets/wasm-glue-ddd.js"));
  assert.ok(shell.includes("/assets/normfix_wasm_bg-eee.wasm"));
});

test("Monaco is left out of the install", () => {
  const shell = offlineShell(bundle);

  // Roughly 2.5 MB bought on a first visit for syntax highlighting the reader
  // may never see. It is cached later, once they have actually loaded it.
  assert.ok(!shell.includes("/assets/codeEditorWidget-fff.js"));
  assert.ok(!shell.includes("/assets/monaco-core-ggg.js"));
});

test("the shell is sorted and free of duplicates", () => {
  const shell = offlineShell(bundle);

  assert.deepEqual(shell, [...shell].sort());
  assert.equal(new Set(shell).size, shell.length);
});

test("the static closure stops at dynamic boundaries", () => {
  assert.deepEqual(staticClosure(bundle, ["assets/index-aaa.js"]).sort(), [
    "assets/index-aaa.js",
    "assets/index-ccc.css",
    "assets/shared-bbb.js",
  ]);
});

test("a cycle in the import graph terminates", () => {
  const cyclic: Bundle = {
    "a.js": { type: "chunk", isEntry: true, imports: ["b.js"] },
    "b.js": { type: "chunk", imports: ["a.js"] },
  };

  assert.deepEqual(staticClosure(cyclic, ["a.js"]).sort(), ["a.js", "b.js"]);
});

test("every playground page is recognized however it is addressed", () => {
  assert.equal(pagePathFor("/"), "/");
  assert.equal(pagePathFor("/index.html"), "/");
  assert.equal(pagePathFor("/pt"), "/pt/");
  assert.equal(pagePathFor("/pt/"), "/pt/");
  assert.equal(pagePathFor("/pt/index.html"), "/pt/");
  assert.equal(pagePathFor("/de/"), null);
});

test("pages are network-first and hashed assets are cache-first", () => {
  assert.equal(strategyFor("/", true), "page");
  assert.equal(strategyFor("/fr/index.html", true), "page");
  assert.equal(strategyFor("/assets/index-aaa.js", false), "asset");
  assert.equal(strategyFor("/favicon.svg", false), "asset");
  assert.equal(strategyFor("/es/site.webmanifest", false), "asset");
});

test("the worker never answers for anything outside the playground", () => {
  // The documentation is a separate build on the same origin. Serving a stale
  // cached page there would be worse than serving none, and the installer is
  // fetched by curl, which does not consult a service worker at all.
  assert.equal(strategyFor("/docs/", true), "passthrough");
  assert.equal(strategyFor("/docs/pt/guide/getting-started.html", true), "passthrough");
  assert.equal(strategyFor("/docs/assets/style-hhh.css", false), "passthrough");
  assert.equal(strategyFor("/install.sh", false), "passthrough");
  assert.equal(strategyFor("/sitemap.xml", false), "passthrough");
  assert.equal(strategyFor("/robots.txt", false), "passthrough");
  assert.equal(strategyFor("/llms.txt", false), "passthrough");
  assert.equal(strategyFor("/og-normfix.png", false), "passthrough");
});

test("a cached asset is never missed over a header the URL already decides", () => {
  // Observed, not theoretical: assets are served with `Vary: Origin`, and the
  // page requests its own scripts with `crossorigin`, so it sends an `Origin`
  // that the precached entries — created from URL strings — do not carry. With
  // the default match the worker missed every precached script and fell
  // through to a network that was not there: install succeeded, the shell
  // loaded from cache, and the app never booted.
  assert.equal(CACHE_MATCH_OPTIONS.ignoreVary, true);
});

test("a page is still found when the URL carries a query string", () => {
  // Campaign and referral parameters must not cost a reader their offline copy.
  assert.equal(CACHE_MATCH_OPTIONS.ignoreSearch, true);
});

test("an unknown route is not turned into the playground", () => {
  // Answering every navigation with the cached shell is the usual single-page
  // trick. Here it would hide a real 404 behind a working-looking editor.
  assert.equal(strategyFor("/de/", true), "passthrough");
  assert.equal(strategyFor("/anything", true), "passthrough");
});
