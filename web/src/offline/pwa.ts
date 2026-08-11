// Page side of offline support: registers the worker, and reports the two
// things a reader actually needs to know — whether this tab still works
// without a network, and whether a newer build is waiting to be used.

/** What the page can truthfully say about offline availability right now. */
export type OfflineState =
  /** No service worker, or an insecure context. Nothing is promised. */
  | "unsupported"
  /** Registered, still filling the cache. Closing the tab now loses it. */
  | "installing"
  /** The shell is cached; this tab survives losing the network. */
  | "ready"
  /** A newer build finished installing and is waiting for a reload. */
  | "update-ready";

export interface OfflineCallbacks {
  onState(state: OfflineState): void;
  onConnectivity(online: boolean): void;
}

export interface OfflineSupport {
  /** Swaps in the waiting build and reloads. Only meaningful in `update-ready`. */
  applyUpdate(): void;
}

/**
 * Starts offline support and reports its state.
 *
 * Registration failure is not an error the reader has to act on: the
 * playground works exactly as before without a worker, so the state simply
 * stays `unsupported` and the UI says nothing more than that.
 */
export function startOfflineSupport(callbacks: OfflineCallbacks): OfflineSupport {
  const online = () => callbacks.onConnectivity(navigator.onLine);
  window.addEventListener("online", online);
  window.addEventListener("offline", online);
  online();

  if (!("serviceWorker" in navigator) || !window.isSecureContext) {
    callbacks.onState("unsupported");
    return { applyUpdate: () => undefined };
  }

  let waiting: ServiceWorker | null = null;
  let reloading = false;

  navigator.serviceWorker.addEventListener("controllerchange", () => {
    // Only reload for a swap this page asked for. An uncontrolled reload would
    // discard whatever the reader had typed.
    if (!reloading) return;
    window.location.reload();
  });

  void (async () => {
    let registration: ServiceWorkerRegistration;
    try {
      registration = await navigator.serviceWorker.register("/sw.js", { scope: "/" });
    } catch {
      callbacks.onState("unsupported");
      return;
    }

    const settle = (): void => {
      if (registration.waiting !== null && navigator.serviceWorker.controller !== null) {
        waiting = registration.waiting;
        callbacks.onState("update-ready");
        return;
      }
      callbacks.onState(registration.active !== null ? "ready" : "installing");
    };

    registration.addEventListener("updatefound", () => {
      const installing = registration.installing;
      if (installing === null) return;
      callbacks.onState(navigator.serviceWorker.controller === null ? "installing" : "ready");
      installing.addEventListener("statechange", settle);
    });
    settle();

    // A reader who leaves the tab open for a week should still be offered the
    // build that shipped meanwhile, without a background poll.
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") void registration.update();
    });
  })();

  return {
    applyUpdate: () => {
      if (waiting === null) return;
      reloading = true;
      waiting.postMessage({ type: "normfix-activate-update" });
    },
  };
}
