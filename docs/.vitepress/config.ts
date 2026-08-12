import { createRequire } from "node:module";
import { defineConfig } from "vitepress";

const require = createRequire(import.meta.url);
const vueServerRenderer = require.resolve("vue/server-renderer");

const siteOrigin = "https://normfix.vercel.app";
const translatedPages = new Map<string, string>([
  ["", ""],
  ["guide/getting-started", "guide/getting-started"],
  ["guide/playground", "guide/playground"],
  ["reference/safety", "reference/safety"],
  ["COMPATIBILITY", "COMPATIBILITY"],
  ["LOCALIZATION", "LOCALIZATION"],
]);
const localePrefixes = ["pt", "es", "fr"] as const;
const localizedSearch = {
  pt: {
    provider: "local" as const,
    options: {
      translations: {
        button: { buttonText: "Buscar", buttonAriaLabel: "Buscar na documentação" },
        modal: {
          displayDetails: "Exibir detalhes",
          resetButtonTitle: "Limpar busca",
          backButtonTitle: "Fechar busca",
          noResultsText: "Nenhum resultado encontrado",
          footer: {
            selectText: "selecionar",
            selectKeyAriaLabel: "Enter",
            navigateText: "navegar",
            navigateUpKeyAriaLabel: "Seta para cima",
            navigateDownKeyAriaLabel: "Seta para baixo",
            closeText: "fechar",
            closeKeyAriaLabel: "Escape",
          },
        },
      },
    },
  },
  es: {
    provider: "local" as const,
    options: {
      translations: {
        button: { buttonText: "Buscar", buttonAriaLabel: "Buscar en la documentación" },
        modal: {
          displayDetails: "Mostrar detalles",
          resetButtonTitle: "Limpiar búsqueda",
          backButtonTitle: "Cerrar búsqueda",
          noResultsText: "No se encontraron resultados",
          footer: {
            selectText: "seleccionar",
            selectKeyAriaLabel: "Enter",
            navigateText: "navegar",
            navigateUpKeyAriaLabel: "Flecha arriba",
            navigateDownKeyAriaLabel: "Flecha abajo",
            closeText: "cerrar",
            closeKeyAriaLabel: "Escape",
          },
        },
      },
    },
  },
  fr: {
    provider: "local" as const,
    options: {
      translations: {
        button: { buttonText: "Rechercher", buttonAriaLabel: "Rechercher dans la documentation" },
        modal: {
          displayDetails: "Afficher les détails",
          resetButtonTitle: "Effacer la recherche",
          backButtonTitle: "Fermer la recherche",
          noResultsText: "Aucun résultat trouvé",
          footer: {
            selectText: "sélectionner",
            selectKeyAriaLabel: "Entrée",
            navigateText: "naviguer",
            navigateUpKeyAriaLabel: "Flèche vers le haut",
            navigateDownKeyAriaLabel: "Flèche vers le bas",
            closeText: "fermer",
            closeKeyAriaLabel: "Échap",
          },
        },
      },
    },
  },
};

function localizedRoute(locale: string, page: string): string {
  const prefix = locale === "en" ? "" : `${locale}/`;
  return `${siteOrigin}/docs/${prefix}${page}`;
}

// The playground owns the site root and the documentation is published beneath
// it, so `base` must match the deployed `/docs/` prefix and `outDir` must write
// inside the Vite bundle that Vercel publishes.
//
// Mermaid fences remain readable on GitHub and are rendered client-side by the
// custom theme without the legacy VitePress plugin dependency.
export default defineConfig({
  title: "normfix",
  description:
    "Safe automatic fixes and actionable diagnostics for the 42 Norm.",
  lang: "en-US",
  locales: {
    root: { label: "English", lang: "en-US", link: "/" },
    pt: {
      label: "Português",
      lang: "pt-BR",
      link: "/pt/",
      description: "Correções automáticas seguras e diagnósticos úteis para a Norma da 42.",
      markdown: { codeCopyButton: { tooltipText: "Copiar código", copiedText: "Copiado" } },
      themeConfig: {
        nav: [
          { text: "Por que", link: "/pt/why" },
          { text: "Guia", link: "/pt/guide/getting-started" },
          { text: "Comandos", link: "/pt/commands/" },
          { text: "Flags", link: "/pt/reference/flags" },
          { text: "Arquitetura", link: "/pt/ARCHITECTURE" },
          {
            text: "Mais",
            items: [
              { text: "Changelog", link: "/pt/changelog" },
              { text: "Compatibilidade", link: "/pt/COMPATIBILITY" },
              { text: "Agentes de IA", link: "/pt/guide/ai-agents" },
              { text: "Contribuir", link: "/pt/contributing" },
              { text: "Segurança", link: "/pt/security" },
              { text: "Roadmap", link: "/pt/ROADMAP" },
              { text: "Localização", link: "/pt/LOCALIZATION" },
            ],
          },
        ],
        sidebar: [
          {
            text: "Guia",
            items: [
              { text: "Por que o normfix", link: "/pt/why" },
              { text: "Primeiros passos", link: "/pt/guide/getting-started" },
              { text: "Linha de comando", link: "/pt/guide/command-line" },
              { text: "Playground no navegador", link: "/pt/guide/playground" },
              { text: "Agentes de IA", link: "/pt/guide/ai-agents" },
            ],
          },
          {
            text: "Comandos",
            items: [
              { text: "Visão geral", link: "/pt/commands/" },
              { text: "format", link: "/pt/commands/format" },
              { text: "lint", link: "/pt/commands/lint" },
              { text: "check", link: "/pt/commands/check" },
              { text: "budget", link: "/pt/commands/budget" },
              { text: "preflight", link: "/pt/commands/preflight" },
              { text: "leaks", link: "/pt/commands/leaks" },
              { text: "explain", link: "/pt/commands/explain" },
              { text: "undo", link: "/pt/commands/undo" },
              { text: "upgrade", link: "/pt/commands/upgrade" },
              { text: "uninstall", link: "/pt/commands/uninstall" },
            ],
          },
          {
            text: "Referência",
            items: [
              { text: "Todas as flags", link: "/pt/reference/flags" },
              { text: "O que é corrigido", link: "/pt/reference/fixes" },
              { text: "Segurança e recuperação", link: "/pt/reference/safety" },
              { text: "Cabeçalhos e identidade", link: "/pt/reference/headers" },
              { text: "Makefiles e projetos", link: "/pt/reference/projects" },
              { text: "Relatórios", link: "/pt/reference/reporting" },
              { text: "Limites conhecidos", link: "/pt/reference/boundaries" },
              { text: "Desempenho", link: "/pt/reference/performance" },
              { text: "Arquitetura", link: "/pt/ARCHITECTURE" },
              { text: "Política de compatibilidade", link: "/pt/COMPATIBILITY" },
              { text: "Processo de release", link: "/pt/RELEASING" },
            ],
          },
          {
            text: "Projeto",
            items: [
              { text: "Changelog", link: "/pt/changelog" },
              { text: "Contribuir", link: "/pt/contributing" },
              { text: "Política de segurança", link: "/pt/security" },
              { text: "Roadmap", link: "/pt/ROADMAP" },
              { text: "Guia de localização", link: "/pt/LOCALIZATION" },
            ],
          },
        ],
        editLink: { text: "Editar esta página no GitHub" },
        docFooter: { prev: "Página anterior", next: "Próxima página" },
        outline: { label: "Nesta página" },
        returnToTopLabel: "Voltar ao topo",
        sidebarMenuLabel: "Menu",
        darkModeSwitchLabel: "Tema",
        lightModeSwitchTitle: "Mudar para o tema claro",
        darkModeSwitchTitle: "Mudar para o tema escuro",
        langMenuLabel: "Mudar idioma",
        skipToContentLabel: "Ir para o conteúdo",
        search: localizedSearch.pt,
        footer: {
          message: "Publicado sob a licença MIT.",
          copyright: "Copyright © 2026 Vinicius Neves Costa",
        },
        notFound: {
          title: "PÁGINA NÃO ENCONTRADA",
          quote: "O endereço pode ter mudado ou não existir.",
          link: "/pt/",
          linkLabel: "ir para o início",
          linkText: "Voltar ao início",
        },
      },
    },
    es: {
      label: "Español",
      lang: "es-ES",
      link: "/es/",
      description: "Correcciones automáticas seguras y diagnósticos útiles para la Norma de 42.",
      markdown: { codeCopyButton: { tooltipText: "Copiar código", copiedText: "Copiado" } },
      themeConfig: {
        nav: [
          { text: "Por qué", link: "/es/why" },
          { text: "Guía", link: "/es/guide/getting-started" },
          { text: "Comandos", link: "/es/commands/" },
          { text: "Flags", link: "/es/reference/flags" },
          { text: "Arquitectura", link: "/es/ARCHITECTURE" },
          {
            text: "Más",
            items: [
              { text: "Changelog", link: "/es/changelog" },
              { text: "Compatibilidad", link: "/es/COMPATIBILITY" },
              { text: "Agentes de IA", link: "/es/guide/ai-agents" },
              { text: "Contribuir", link: "/es/contributing" },
              { text: "Seguridad", link: "/es/security" },
              { text: "Roadmap", link: "/es/ROADMAP" },
              { text: "Localización", link: "/es/LOCALIZATION" },
            ],
          },
        ],
        sidebar: [
          {
            text: "Guía",
            items: [
              { text: "Por qué normfix", link: "/es/why" },
              { text: "Primeros pasos", link: "/es/guide/getting-started" },
              { text: "Línea de comandos", link: "/es/guide/command-line" },
              { text: "Playground del navegador", link: "/es/guide/playground" },
              { text: "Agentes de IA", link: "/es/guide/ai-agents" },
            ],
          },
          {
            text: "Comandos",
            items: [
              { text: "Resumen", link: "/es/commands/" },
              { text: "format", link: "/es/commands/format" },
              { text: "lint", link: "/es/commands/lint" },
              { text: "check", link: "/es/commands/check" },
              { text: "budget", link: "/es/commands/budget" },
              { text: "preflight", link: "/es/commands/preflight" },
              { text: "leaks", link: "/es/commands/leaks" },
              { text: "explain", link: "/es/commands/explain" },
              { text: "undo", link: "/es/commands/undo" },
              { text: "upgrade", link: "/es/commands/upgrade" },
              { text: "uninstall", link: "/es/commands/uninstall" },
            ],
          },
          {
            text: "Referencia",
            items: [
              { text: "Todas las flags", link: "/es/reference/flags" },
              { text: "Qué se corrige", link: "/es/reference/fixes" },
              { text: "Seguridad y recuperación", link: "/es/reference/safety" },
              { text: "Cabeceras e identidad", link: "/es/reference/headers" },
              { text: "Makefiles y proyectos", link: "/es/reference/projects" },
              { text: "Informes", link: "/es/reference/reporting" },
              { text: "Límites conocidos", link: "/es/reference/boundaries" },
              { text: "Rendimiento", link: "/es/reference/performance" },
              { text: "Arquitectura", link: "/es/ARCHITECTURE" },
              { text: "Política de compatibilidad", link: "/es/COMPATIBILITY" },
              { text: "Proceso de publicación", link: "/es/RELEASING" },
            ],
          },
          {
            text: "Proyecto",
            items: [
              { text: "Changelog", link: "/es/changelog" },
              { text: "Contribuir", link: "/es/contributing" },
              { text: "Política de seguridad", link: "/es/security" },
              { text: "Roadmap", link: "/es/ROADMAP" },
              { text: "Guía de localización", link: "/es/LOCALIZATION" },
            ],
          },
        ],
        editLink: { text: "Editar esta página en GitHub" },
        docFooter: { prev: "Página anterior", next: "Página siguiente" },
        outline: { label: "En esta página" },
        returnToTopLabel: "Volver arriba",
        sidebarMenuLabel: "Menú",
        darkModeSwitchLabel: "Tema",
        lightModeSwitchTitle: "Cambiar al tema claro",
        darkModeSwitchTitle: "Cambiar al tema oscuro",
        langMenuLabel: "Cambiar idioma",
        skipToContentLabel: "Ir al contenido",
        search: localizedSearch.es,
        footer: {
          message: "Publicado bajo la licencia MIT.",
          copyright: "Copyright © 2026 Vinicius Neves Costa",
        },
        notFound: {
          title: "PÁGINA NO ENCONTRADA",
          quote: "La dirección puede haber cambiado o no existir.",
          link: "/es/",
          linkLabel: "ir al inicio",
          linkText: "Volver al inicio",
        },
      },
    },
    fr: {
      label: "Français",
      lang: "fr-FR",
      link: "/fr/",
      description: "Corrections automatiques sûres et diagnostics utiles pour la Norme de 42.",
      markdown: { codeCopyButton: { tooltipText: "Copier le code", copiedText: "Copié" } },
      themeConfig: {
        nav: [
          { text: "Pourquoi", link: "/fr/why" },
          { text: "Guide", link: "/fr/guide/getting-started" },
          { text: "Commandes", link: "/fr/commands/" },
          { text: "Options", link: "/fr/reference/flags" },
          { text: "Architecture", link: "/fr/ARCHITECTURE" },
          {
            text: "Plus",
            items: [
              { text: "Changelog", link: "/fr/changelog" },
              { text: "Compatibilité", link: "/fr/COMPATIBILITY" },
              { text: "Agents d’IA", link: "/fr/guide/ai-agents" },
              { text: "Contribuer", link: "/fr/contributing" },
              { text: "Sécurité", link: "/fr/security" },
              { text: "Roadmap", link: "/fr/ROADMAP" },
              { text: "Localisation", link: "/fr/LOCALIZATION" },
            ],
          },
        ],
        sidebar: [
          {
            text: "Guide",
            items: [
              { text: "Pourquoi normfix", link: "/fr/why" },
              { text: "Bien démarrer", link: "/fr/guide/getting-started" },
              { text: "Ligne de commande", link: "/fr/guide/command-line" },
              { text: "Playground navigateur", link: "/fr/guide/playground" },
              { text: "Agents d’IA", link: "/fr/guide/ai-agents" },
            ],
          },
          {
            text: "Commandes",
            items: [
              { text: "Vue d’ensemble", link: "/fr/commands/" },
              { text: "format", link: "/fr/commands/format" },
              { text: "lint", link: "/fr/commands/lint" },
              { text: "check", link: "/fr/commands/check" },
              { text: "budget", link: "/fr/commands/budget" },
              { text: "preflight", link: "/fr/commands/preflight" },
              { text: "leaks", link: "/fr/commands/leaks" },
              { text: "explain", link: "/fr/commands/explain" },
              { text: "undo", link: "/fr/commands/undo" },
              { text: "upgrade", link: "/fr/commands/upgrade" },
              { text: "uninstall", link: "/fr/commands/uninstall" },
            ],
          },
          {
            text: "Référence",
            items: [
              { text: "Toutes les options", link: "/fr/reference/flags" },
              { text: "Ce qui est corrigé", link: "/fr/reference/fixes" },
              { text: "Sécurité et récupération", link: "/fr/reference/safety" },
              { text: "En-têtes et identité", link: "/fr/reference/headers" },
              { text: "Makefiles et projets", link: "/fr/reference/projects" },
              { text: "Rapports", link: "/fr/reference/reporting" },
              { text: "Limites connues", link: "/fr/reference/boundaries" },
              { text: "Performance", link: "/fr/reference/performance" },
              { text: "Architecture", link: "/fr/ARCHITECTURE" },
              { text: "Politique de compatibilité", link: "/fr/COMPATIBILITY" },
              { text: "Processus de publication", link: "/fr/RELEASING" },
            ],
          },
          {
            text: "Projet",
            items: [
              { text: "Changelog", link: "/fr/changelog" },
              { text: "Contribuer", link: "/fr/contributing" },
              { text: "Politique de sécurité", link: "/fr/security" },
              { text: "Roadmap", link: "/fr/ROADMAP" },
              { text: "Guide de localisation", link: "/fr/LOCALIZATION" },
            ],
          },
        ],
        editLink: { text: "Modifier cette page sur GitHub" },
        docFooter: { prev: "Page précédente", next: "Page suivante" },
        outline: { label: "Sur cette page" },
        returnToTopLabel: "Retour en haut",
        sidebarMenuLabel: "Menu",
        darkModeSwitchLabel: "Thème",
        lightModeSwitchTitle: "Passer au thème clair",
        darkModeSwitchTitle: "Passer au thème sombre",
        langMenuLabel: "Changer de langue",
        skipToContentLabel: "Aller au contenu",
        search: localizedSearch.fr,
        footer: {
          message: "Publié sous licence MIT.",
          copyright: "Copyright © 2026 Vinicius Neves Costa",
        },
        notFound: {
          title: "PAGE INTROUVABLE",
          quote: "L’adresse a peut-être changé ou n’existe pas.",
          link: "/fr/",
          linkLabel: "aller à l’accueil",
          linkText: "Retour à l’accueil",
        },
      },
    },
  },
  base: "/docs/",
  outDir: "../web/dist/docs",
  cleanUrls: true,
  sitemap: { hostname: `${siteOrigin}/docs/` },
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
          { text: "AI agents", link: "/guide/ai-agents" },
          { text: "Contributing", link: "/contributing" },
          { text: "Security", link: "/security" },
          { text: "Roadmap", link: "/ROADMAP" },
          { text: "Localization", link: "/LOCALIZATION" },
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
          { text: "AI agents", link: "/guide/ai-agents" },
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
              { text: "leaks", link: "/commands/leaks" },
          { text: "explain", link: "/commands/explain" },
          { text: "undo", link: "/commands/undo" },
          { text: "upgrade", link: "/commands/upgrade" },
          { text: "uninstall", link: "/commands/uninstall" },
        ],
      },
      {
        text: "Reference",
        items: [
          { text: "Every flag", link: "/reference/flags" },
          { text: "What is fixed", link: "/reference/fixes" },
          { text: "Safety and recovery", link: "/reference/safety" },
          { text: "Headers and identity", link: "/reference/headers" },
          { text: "Makefiles and projects", link: "/reference/projects" },
          { text: "Reporting", link: "/reference/reporting" },
          { text: "Known boundaries", link: "/reference/boundaries" },
          { text: "Performance", link: "/reference/performance" },
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
          { text: "Roadmap", link: "/ROADMAP" },
          { text: "Localization guide", link: "/LOCALIZATION" },
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
      const defaultFence = md.renderer.rules.fence;
      md.renderer.rules.fence = (tokens, index, options, environment, self) => {
        const token = tokens[index];
        if (token?.info.trim() === "mermaid") {
          return `<pre class="mermaid">${md.utils.escapeHtml(token.content)}</pre>`;
        }
        return defaultFence
          ? defaultFence(tokens, index, options, environment, self)
          : self.renderToken(tokens, index, options);
      };
    },
  },
  transformHead({ page, title, description }) {
    const route = page
      .replace(/\.(?:md|html)$/, "")
      .replace(/(^|\/)index$/, "$1");
    const withoutLocale = localePrefixes.reduce(
      (value, locale) => value.replace(new RegExp(`^${locale}/`), ""),
      route,
    );
    const pageKey = translatedPages.get(withoutLocale);
    const activeLocale = localePrefixes.find((locale) => route.startsWith(`${locale}/`)) ?? "en";
    const canonical = `${siteOrigin}/docs/${route}`;
    const head: Array<[string, Record<string, string>]> = [
      ["link", { rel: "canonical", href: canonical }],
      ["meta", { property: "og:type", content: "article" }],
      ["meta", { property: "og:site_name", content: "normfix" }],
      ["meta", { property: "og:title", content: title }],
      ["meta", { property: "og:description", content: description }],
      ["meta", { property: "og:url", content: canonical }],
      ["meta", { property: "og:image", content: `${siteOrigin}/og-normfix.png` }],
      ["meta", { property: "og:image:width", content: "1731" }],
      ["meta", { property: "og:image:height", content: "909" }],
      ["meta", {
        property: "og:image:alt",
        content: "normfix — safe fixes and clear diagnostics for 42 C projects",
      }],
      ["meta", { name: "twitter:card", content: "summary_large_image" }],
      ["meta", { name: "twitter:image", content: `${siteOrigin}/og-normfix.png` }],
      ["meta", {
        name: "twitter:image:alt",
        content: "normfix — safe fixes and clear diagnostics for 42 C projects",
      }],
      ["meta", {
        property: "og:locale",
        content: activeLocale === "pt" ? "pt_BR" : activeLocale === "es" ? "es_ES" : activeLocale === "fr" ? "fr_FR" : "en_US",
      }],
    ];
    if (pageKey !== undefined) {
      head.push(
        ["link", { rel: "alternate", hreflang: "x-default", href: localizedRoute("en", pageKey) }],
        ["link", { rel: "alternate", hreflang: "en", href: localizedRoute("en", pageKey) }],
        ["link", { rel: "alternate", hreflang: "pt-BR", href: localizedRoute("pt", pageKey) }],
        ["link", { rel: "alternate", hreflang: "es", href: localizedRoute("es", pageKey) }],
        ["link", { rel: "alternate", hreflang: "fr", href: localizedRoute("fr", pageKey) }],
      );
    }
    return head;
  },
  // VitePress preloads every async chunk it knows about, so a page with no
  // diagram still told the browser to fetch several megabytes of renderer.
  // Only a page that actually renders one keeps the hint.
  transformHtml(code) {
    // The landing pages use the home layout, which wraps its content in a plain
    // div. Every other page gets a `main` element from the theme, so a screen
    // reader arriving at the landing page is the one reader with no landmark to
    // skip to. The default theme's component cannot be changed from here, but
    // the role it should carry can.
    let html = code.replace(
      /<div class="VPContent is-home"/,
      '<div class="VPContent is-home" role="main"',
    );
    // VitePress preloads every async chunk it knows about, so a page with no
    // diagram still told the browser to fetch several megabytes of renderer.
    // Only a page that actually renders one keeps the hint.
    if (!html.includes('class="mermaid')) {
      html = html.replace(
        /\s*<link rel="modulepreload" href="[^"]*(?:mermaid|cynefin)[^"]*">/g,
        "",
      );
    }
    return html;
  },
  vite: {
    resolve: {
      // VitePress 2 alpha externalizes this subpath while rendering. Resolving
      // it eagerly avoids Node treating Vue's directory as a bare ESM import
      // in npm workspaces.
      alias: { "vue/server-renderer": vueServerRenderer },
    },
    build: {
      // Mermaid is loaded only by pages that contain a diagram. Its dedicated
      // async chunk is intentionally larger than an ordinary documentation
      // page and remains cacheable across prose-only edits.
      chunkSizeWarningLimit: 700,
    },
  },
});
