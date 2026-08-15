// Offline policy for the playground.
//
// Two decisions live here rather than inside the service worker: which built
// files have to exist before the app can run without a network, and which
// requests the worker is allowed to answer at all. Both are pure functions so
// they can be tested directly; the worker and the Vite plugin only apply them.

/** Playground pages. Every other route on the origin belongs to someone else. */
export const PAGE_PATHS = ["/", "/pt/", "/es/", "/fr/"] as const;

/**
 * Same-origin files the shell needs that no chunk graph mentions. Each page has
 * its own manifest so an installed app carries the reader's language on its
 * home screen.
 */
export const EXTRA_SHELL_PATHS: readonly string[] = [
  "/favicon.svg",
  ...PAGE_PATHS.map((page) => `${page}site.webmanifest`),
];

/**
 * How the service worker should answer a request.
 *
 * `page` is network-first: an online reader always gets the freshest HTML, and
 * an offline one gets the copy from the last install. `asset` is cache-first,
 * because every asset URL carries a content hash and therefore never changes
 * meaning. `passthrough` means the worker does not call `respondWith` at all,
 * so the browser behaves exactly as if no worker were installed.
 */
export type RequestStrategy = "page" | "asset" | "passthrough";

/**
 * Reduces a URL path to the playground page it addresses, or `null` when it
 * addresses something else. `/pt` and `/pt/index.html` both name `/pt/`.
 */
export function pagePathFor(pathname: string): string | null {
  let path = pathname.endsWith("/index.html") ? pathname.slice(0, -"index.html".length) : pathname;
  if (!path.endsWith("/")) path = `${path}/`;
  return (PAGE_PATHS as readonly string[]).includes(path) ? path : null;
}

/**
 * Decides what the worker does with a request.
 *
 * The origin also serves the documentation site, the installer script, and the
 * crawler files. Those are deliberately passed through: a stale cached copy of
 * a document is worse than a missing one, and the installer is fetched by curl,
 * which never consults a service worker anyway.
 */
export function strategyFor(pathname: string, isNavigation: boolean): RequestStrategy {
  if (isNavigation) return pagePathFor(pathname) === null ? "passthrough" : "page";
  if (pathname.startsWith("/assets/")) return "asset";
  return EXTRA_SHELL_PATHS.includes(pathname) ? "asset" : "passthrough";
}

/**
 * How the worker looks a request up in the cache.
 *
 * `ignoreVary` is not an optimization, it is required. The precache is filled
 * from URL strings, so those stored requests carry no `Origin` header, while
 * the real page requests its own scripts and styles with `crossorigin` and
 * therefore does send one. Vite serves assets with `Vary: Origin`, so the
 * default match rejects an entry that is present and correct, and the worker
 * falls through to a network that is not there — an install that reports
 * success and then a shell that never boots offline.
 *
 * Ignoring it is safe here because every asset URL carries a content hash: the
 * URL alone identifies the bytes, and no header could change the answer.
 */
export const CACHE_MATCH_OPTIONS: CacheQueryOptions = { ignoreVary: true, ignoreSearch: true };

/** The parts of a bundled file this policy needs. Deliberately not Rollup's type. */
export interface BundledFile {
  type: "chunk" | "asset";
  isEntry?: boolean;
  /** Static imports only. Dynamic ones are what keeps Monaco out of the shell. */
  imports?: readonly string[];
  importedCss?: readonly string[];
  moduleIds?: readonly string[];
}

export type Bundle = Readonly<Record<string, BundledFile>>;

/** Every file reachable from `roots` through static imports, including their CSS. */
export function staticClosure(bundle: Bundle, roots: readonly string[]): string[] {
  const reached = new Set<string>();
  const pending = [...roots];
  while (pending.length > 0) {
    const fileName = pending.pop();
    if (fileName === undefined || reached.has(fileName)) continue;
    const file = bundle[fileName];
    if (file === undefined) continue;
    reached.add(fileName);
    for (const css of file.importedCss ?? []) reached.add(css);
    for (const next of file.imports ?? []) pending.push(next);
  }
  return [...reached];
}

/**
 * The files that must be cached during install for the playground to start
 * with no network at all.
 *
 * Monaco is reached only through dynamic imports, so the static closure leaves
 * it out on purpose: precaching it would add roughly 2.5 MB to a first visit
 * to buy a nicer editor, and the textarea path formats exactly the same code.
 * Monaco is still cached opportunistically once a reader has loaded it online.
 *
 * The WebAssembly module is the opposite case. It is loaded dynamically too,
 * but without it there is no formatter, and a playground that cannot format is
 * not worth installing — so its chunk and its `.wasm` payload are pulled in
 * explicitly.
 */
export function offlineShell(bundle: Bundle): string[] {
  const roots: string[] = [];
  for (const [fileName, file] of Object.entries(bundle)) {
    if (file.type !== "chunk") continue;
    const isWasmGlue = (file.moduleIds ?? []).some((id) => id.endsWith("pkg/normfix_wasm.js"));
    if (file.isEntry === true || isWasmGlue) roots.push(fileName);
  }
  const urls = new Set<string>([...PAGE_PATHS, ...EXTRA_SHELL_PATHS]);
  for (const fileName of staticClosure(bundle, roots)) urls.add(`/${fileName}`);
  for (const [fileName, file] of Object.entries(bundle)) {
    if (file.type === "asset" && fileName.endsWith(".wasm")) urls.add(`/${fileName}`);
  }
  return [...urls].sort();
}
