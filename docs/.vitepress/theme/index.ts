import DefaultTheme from "vitepress/theme";
import { defineComponent, h, nextTick, onMounted, watch } from "vue";
import { useRoute, useRouter } from "vitepress";

import "./custom.css";
import "./playground-link.css";

const LOCALES = ["pt", "es", "fr"] as const;

// The playground writes the reader's language choice here, so following a link
// from one to the other keeps the language they picked.
const LOCALE_STORAGE_KEY = "normfix.locale.v1";

type StoredLocale = "en" | (typeof LOCALES)[number];

function isSupported(value: string | null): value is StoredLocale {
  return value === "en" || LOCALES.includes(value as (typeof LOCALES)[number]);
}

function readStoredLocale(): StoredLocale | null {
  try {
    const stored = localStorage.getItem(LOCALE_STORAGE_KEY);
    return isSupported(stored) ? stored : null;
  } catch {
    // A blocked storage API is not a reason to fail to render a page.
    return null;
  }
}

function rememberLocale(locale: StoredLocale): void {
  try {
    localStorage.setItem(LOCALE_STORAGE_KEY, locale);
  } catch {
    // Nothing to do: the reader simply gets the default next time.
  }
}

/// Returns the first browser language this site publishes.
function preferredLocale(): StoredLocale {
  for (const tag of navigator.languages ?? [navigator.language]) {
    const primary = tag.split(/[-_]/)[0]?.toLowerCase();
    if (isSupported(primary ?? null)) return primary as StoredLocale;
  }
  return "en";
}

// The documentation is published under `/docs/`, so VitePress rewrites every
// internal navigation link with that prefix. The playground lives at the site
// root, which means its navbar entry has to be a plain anchor rendered outside
// the theme's link normalization.
const NormfixLayout = defineComponent({
  name: "NormfixLayout",
  setup() {
    const route = useRoute();
    const router = useRouter();
    const locale = (): StoredLocale =>
      (route.path
        .split("/")
        .find((segment) =>
          LOCALES.includes(segment as (typeof LOCALES)[number]),
        ) as StoredLocale) ?? "en";
    const playgroundHref = () => {
      const activeLocale = locale();
      return ["pt", "es", "fr"].includes(activeLocale) ? `/${activeLocale}/` : "/";
    };
    const playgroundTitle = () => ({
      pt: "Abrir o playground WebAssembly no navegador",
      es: "Abrir el playground WebAssembly en el navegador",
      fr: "Ouvrir le playground WebAssembly dans le navigateur",
    })[locale()] ?? "Open the in-browser WebAssembly playground";
    const renderDiagrams = async () => {
      await nextTick();
      const nodes = [...document.querySelectorAll<HTMLElement>("pre.mermaid:not([data-processed])")];
      if (nodes.length > 0) {
        const { default: mermaid } = await import("mermaid");
        await mermaid.run({ nodes });
      }
    };
    onMounted(() => void renderDiagrams());
    watch(() => route.path, () => void renderDiagrams(), { flush: "post" });

    // A URL the reader typed or was sent is a decision: it is recorded, never
    // overridden. Only the language-neutral landing route is redirected, and
    // only once, so a shared link always opens the page it names.
    onMounted(() => {
      const current = locale();
      // `route.path` may or may not carry the `/docs/` base depending on how
      // VitePress resolved the request, so the check tolerates both rather than
      // silently never firing.
      const withoutBase = route.path
        .replace(/^\/docs/, "")
        .replace(/index\.html$/, "");
      const landing = withoutBase === "/" || withoutBase === "";
      if (!landing) {
        rememberLocale(current);
        return;
      }
      const chosen = readStoredLocale() ?? preferredLocale();
      if (chosen === "en") {
        rememberLocale("en");
        return;
      }
      router.go(`/docs/${chosen}/`);
    });
    watch(
      () => locale(),
      (current) => rememberLocale(current),
    );
    return () =>
      h(DefaultTheme.Layout, null, {
        "nav-bar-content-after": () =>
          h(
            "a",
            {
              class: "playground-nav-link",
              href: playgroundHref(),
              target: "_self",
              title: playgroundTitle(),
            },
            "Playground",
          ),
      });
  },
});

export default {
  extends: DefaultTheme,
  Layout: NormfixLayout,
};
