import DefaultTheme from "vitepress/theme";
import { defineAsyncComponent, h } from "vue";

import "./custom.css";
import "./playground-link.css";

// The documentation is published under `/docs/`, so VitePress rewrites every
// internal navigation link with that prefix. The playground lives at the site
// root, which means its navbar entry has to be a plain anchor rendered outside
// the theme's link normalization.
export default {
  extends: DefaultTheme,
  enhanceApp({ app }: { app: import("vue").App }) {
    // The renderer is several megabytes. Registering it asynchronously keeps it
    // out of the shared bundle, so only a page with a diagram pays for it.
    app.component(
      "Mermaid",
      defineAsyncComponent(() => import("vitepress-plugin-mermaid/Mermaid.vue")),
    );
  },
  Layout() {
    return h(DefaultTheme.Layout, null, {
      "nav-bar-content-after": () =>
        h(
          "a",
          {
            class: "playground-nav-link",
            href: "/",
            target: "_self",
            title: "Open the in-browser WebAssembly playground",
          },
          "Playground",
        ),
    });
  },
};
