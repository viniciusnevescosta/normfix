import DefaultTheme from "vitepress/theme";
import { defineComponent, h, nextTick, onMounted, watch } from "vue";
import { useRoute } from "vitepress";

import "./custom.css";
import "./playground-link.css";

// The documentation is published under `/docs/`, so VitePress rewrites every
// internal navigation link with that prefix. The playground lives at the site
// root, which means its navbar entry has to be a plain anchor rendered outside
// the theme's link normalization.
const NormfixLayout = defineComponent({
  name: "NormfixLayout",
  setup() {
    const route = useRoute();
    const locale = () =>
      route.path
        .split("/")
        .find((segment) => ["pt", "es", "fr"].includes(segment)) ?? "en";
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
