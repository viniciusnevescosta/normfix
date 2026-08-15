/// <reference lib="webworker" />

// The playground's service worker.
//
// It is bundled to /sw.js by the Vite plugin in vite.config.ts, which fills in
// the two constants below from the real build output. Nothing here decides
// policy: which files are cached and which requests are answered both come
// from precache.ts, so they can be tested without a browser.

import { CACHE_MATCH_OPTIONS, pagePathFor, strategyFor } from "./precache";

/** Cache name for this exact build. Injected by the build; see vite.config.ts. */
declare const __NORMFIX_CACHE__: string;
/** URLs that must be present before this worker reports itself installed. */
declare const __NORMFIX_PRECACHE__: readonly string[];

const worker = self as unknown as ServiceWorkerGlobalScope;

/** Every cache this project has ever owned starts with this. Nothing else is touched. */
const CACHE_PREFIX = "normfix-playground-";

worker.addEventListener("install", (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(__NORMFIX_CACHE__);
      // addAll is atomic: one failed request leaves the whole worker
      // uninstalled, which is the honest outcome. A partially cached shell
      // would claim offline support it cannot deliver.
      await cache.addAll([...__NORMFIX_PRECACHE__]);
    })(),
  );
});

worker.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      for (const name of await caches.keys()) {
        if (name.startsWith(CACHE_PREFIX) && name !== __NORMFIX_CACHE__) {
          await caches.delete(name);
        }
      }
      await worker.clients.claim();
    })(),
  );
});

// The page asks for the swap explicitly, after telling the reader an update is
// ready. Doing it automatically would replace an editing session's assets
// underneath it.
worker.addEventListener("message", (event: ExtendableMessageEvent) => {
  if ((event.data as { type?: string } | null)?.type === "normfix-activate-update") {
    void worker.skipWaiting();
  }
});

worker.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== worker.location.origin) return;

  const strategy = strategyFor(url.pathname, request.mode === "navigate");
  if (strategy === "passthrough") return;
  if (strategy === "page") {
    event.respondWith(freshPageOrCached(request, url));
    return;
  }
  event.respondWith(cachedAssetOrNetwork(request));
});

/**
 * Pages come from the network whenever there is one, so an online reader is
 * never served yesterday's HTML, and fall back to the installed copy of that
 * same page otherwise.
 */
async function freshPageOrCached(request: Request, url: URL): Promise<Response> {
  const cache = await caches.open(__NORMFIX_CACHE__);
  try {
    const response = await fetch(request);
    if (response.ok) await cache.put(pagePathFor(url.pathname) ?? url.pathname, response.clone());
    return response;
  } catch (error) {
    const cached = await cache.match(
      pagePathFor(url.pathname) ?? url.pathname,
      CACHE_MATCH_OPTIONS,
    );
    if (cached) return cached;
    throw error;
  }
}

/**
 * Asset URLs carry a content hash, so a cached copy can never be the wrong
 * answer. A miss is fetched and kept, which is how Monaco becomes available
 * offline after the first time a reader loads it online.
 */
async function cachedAssetOrNetwork(request: Request): Promise<Response> {
  const cache = await caches.open(__NORMFIX_CACHE__);
  const cached = await cache.match(request, CACHE_MATCH_OPTIONS);
  if (cached) return cached;
  const response = await fetch(request);
  if (response.ok) await cache.put(request, response.clone());
  return response;
}
