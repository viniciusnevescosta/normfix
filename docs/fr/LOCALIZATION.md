# Guide de localisation

Les interfaces publiées sont l’anglais (`en`), le portugais (`pt`), l’espagnol
(`es`) et le français (`fr`). Une traduction doit permettre d’installer
l’outil, de comprendre sa limite de sécurité et d’utiliser le playground sans
texte d’interface anglais résiduel.

## Ce qui ne se traduit pas

Gardez `normfix`, les sous-commandes, flags, IDs comme `TOO_MANY_LINES`, clés
JSON, `schema_version`, codes de sortie, noms de configuration, chemins et
exemples de code inchangés. Conservez les noms officiels Norminette, Rust, WSL,
Clang, Vite, Monaco, Git, GitHub et Vercel, avec leurs liens officiels.

## Playground

Ajoutez la langue à `SUPPORTED_LOCALES` dans `web/i18n.ts` et traduisez chaque
`MessageKey`, y compris validations, confidentialité, titres et libellés
accessibles. Le texte statique utilise `data-i18n`, `data-i18n-title`,
`data-i18n-placeholder` ou `data-i18n-aria` ; le texte dynamique utilise
`translate()`. Préservez les placeholders comme `{path}` et `{count}` et
n’injectez jamais une traduction avec `innerHTML`.

Les diagnostics Rust natifs restent en anglais jusqu’à la localisation de la
CLI 1.1. L’interface doit expliquer cette limite clairement.

## Documentation et validation

Avant d’annoncer une langue, traduisez landing, bien démarrer, playground,
sécurité/récupération, compatibilité et ce guide. Ajoutez navigation, sitemap,
canonical et `hreflang` uniquement pour des routes réelles.

```sh
npm ci
npm run typecheck
npm audit --audit-level=moderate
npm run build
```

Vérifiez desktop et mobile, clavier, focus, overflow, liens, métadonnées et
sitemap. Une personne compétente dans la langue doit approuver sens et ton ; le
build ne prouve que la forme du catalogue.
