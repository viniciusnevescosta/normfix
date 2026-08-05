import { defineConfig } from "vitepress";
import { MermaidMarkdown, MermaidPlugin } from "vitepress-plugin-mermaid";

// The playground owns the site root and the documentation is published beneath
// it, so `base` must match the deployed `/docs/` prefix and `outDir` must write
// inside the Vite bundle that Vercel publishes.
//
// `withMermaid` renders the architecture diagram that the reference documents
// already express as a fenced `mermaid` block for GitHub.
export default defineConfig({
  title: "normfix",
  description:
    "Safe automatic fixes and actionable diagnostics for the 42 Norm.",
  lang: "en-US",
  base: "/docs/",
  outDir: "../web/dist/docs",
  cleanUrls: true,
  lastUpdated: false,
  // README.md documents this directory for people reading the repository; it is
  // not a page of the published site.
  srcExclude: ["README.md"],
  // The reference documents are also read on GitHub, where a repository-relative
  // link to the playground README resolves correctly. Only that link is exempt.
  ignoreDeadLinks: [/\/web\/README$/],
  // The playground is dark-only, so the documentation opens dark to match and
  // still offers the toggle for readers who prefer light.
  appearance: "dark",
  head: [
    ["meta", { name: "color-scheme", content: "dark light" }],
    ["link", { rel: "icon", href: "/favicon.svg", type: "image/svg+xml" }],
  ],
  themeConfig: {
    siteTitle: "normfix",
    nav: [
      { text: "Why", link: "/why" },
      { text: "Guide", link: "/guide/getting-started" },
      { text: "Commands", link: "/commands/" },
      { text: "Flags", link: "/reference/flags" },
      { text: "Architecture", link: "/ARCHITECTURE" },
      {
        text: "More",
        items: [
          { text: "Changelog", link: "/changelog" },
          { text: "Compatibility", link: "/COMPATIBILITY" },
          { text: "Contributing", link: "/contributing" },
          { text: "Security", link: "/security" },
        ],
      },
    ],
    sidebar: [
      {
        text: "Guide",
        items: [
          { text: "Why normfix", link: "/why" },
          { text: "Getting started", link: "/guide/getting-started" },
          { text: "Command line", link: "/guide/command-line" },
          { text: "Browser playground", link: "/guide/playground" },
        ],
      },
      {
        text: "Commands",
        items: [
          { text: "Overview", link: "/commands/" },
          { text: "format", link: "/commands/format" },
          { text: "lint", link: "/commands/lint" },
          { text: "check", link: "/commands/check" },
          { text: "budget", link: "/commands/budget" },
          { text: "preflight", link: "/commands/preflight" },
          { text: "explain", link: "/commands/explain" },
          { text: "undo", link: "/commands/undo" },
        ],
      },
      {
        text: "Reference",
        items: [
          { text: "Every flag", link: "/reference/flags" },
          { text: "Architecture", link: "/ARCHITECTURE" },
          { text: "Compatibility policy", link: "/COMPATIBILITY" },
          { text: "Release process", link: "/RELEASING" },
        ],
      },
      {
        text: "Project",
        items: [
          { text: "Changelog", link: "/changelog" },
          { text: "Contributing", link: "/contributing" },
          { text: "Security policy", link: "/security" },
        ],
      },
    ],
    socialLinks: [
      {
        icon: "github",
        link: "https://github.com/viniciusnevescosta/normfix",
      },
    ],
    outline: [2, 3],
    search: { provider: "local" },
    editLink: {
      pattern:
        "https://github.com/viniciusnevescosta/normfix/edit/main/docs/:path",
      text: "Edit this page on GitHub",
    },
    footer: {
      message: "Released under the MIT License.",
      copyright: "Copyright © 2026 Vinicius Neves Costa",
    },
  },
  markdown: {
    config: (md) => {
      md.use(MermaidMarkdown);
    },
  },
  // VitePress preloads every async chunk it knows about, so a page with no
  // diagram still told the browser to fetch several megabytes of renderer.
  // Only a page that actually renders one keeps the hint.
  transformHtml(code) {
    if (code.includes('class="mermaid')) {
      return code;
    }
    return code.replace(
      /\s*<link rel="modulepreload" href="[^"]*(?:mermaid|cynefin)[^"]*">/g,
      "",
    );
  },
  vite: {
    plugins: [MermaidPlugin()],
    build: {
      // The mermaid renderer is large and the plugin imports it statically into
      // the theme, so it cannot be made lazy from here. Isolating it in its own
      // chunk at least keeps its hash stable: editing a page no longer
      // invalidates the renderer in a reader's cache.
      chunkSizeWarningLimit: 700,
      rollupOptions: {
        output: {
          manualChunks(id: string) {
            if (
              /node_modules\/(mermaid|cytoscape|dagre|@?d3|katex|elkjs|khroma|roughjs)/.test(
                id,
              )
            ) {
              return "mermaid";
            }
            return undefined;
          },
        },
      },
    },
  },
});
