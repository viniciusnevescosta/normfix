import { defineConfig } from "vitepress";
import { withMermaid } from "vitepress-plugin-mermaid";

// The playground owns the site root and the documentation is published beneath
// it, so `base` must match the deployed `/docs/` prefix and `outDir` must write
// inside the Vite bundle that Vercel publishes.
//
// `withMermaid` renders the architecture diagram that the reference documents
// already express as a fenced `mermaid` block for GitHub.
export default withMermaid(defineConfig({
  title: "normfix",
  description:
    "Safe automatic fixes and actionable diagnostics for the 42 Norm.",
  lang: "en-US",
  base: "/docs/",
  outDir: "../web/dist/docs",
  cleanUrls: true,
  lastUpdated: false,
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
      { text: "Guide", link: "/guide/getting-started" },
      { text: "Architecture", link: "/ARCHITECTURE" },
      { text: "Compatibility", link: "/COMPATIBILITY" },
    ],
    sidebar: [
      {
        text: "Guide",
        items: [
          { text: "Getting started", link: "/guide/getting-started" },
          { text: "Command line", link: "/guide/command-line" },
          { text: "Browser playground", link: "/guide/playground" },
        ],
      },
      {
        text: "Reference",
        items: [
          { text: "Architecture", link: "/ARCHITECTURE" },
          { text: "Compatibility policy", link: "/COMPATIBILITY" },
          { text: "Release process", link: "/RELEASING" },
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
  mermaid: {
    theme: "dark",
    securityLevel: "strict",
  },
}));
