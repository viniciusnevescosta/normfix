# Guía de localización

Las interfaces publicadas son inglés (`en`), portugués (`pt`), español (`es`) y
francés (`fr`). Una traducción debe permitir instalar la herramienta, entender
su límite de seguridad y usar el playground sin texto residual de interfaz en
inglés.

## Qué no se traduce

Mantén `normfix`, subcomandos, flags, IDs como `TOO_MANY_LINES`, claves JSON,
`schema_version`, códigos de salida, nombres de configuración, rutas y ejemplos
de código sin cambios. Conserva los nombres oficiales Norminette, Rust, WSL,
Clang, Vite, Monaco, Git, GitHub y Vercel, con sus enlaces oficiales.

## Playground

Añade el idioma a `SUPPORTED_LOCALES` en `web/i18n.ts` y traduce cada
`MessageKey`, incluidas validaciones, privacidad, títulos y etiquetas
accesibles. El texto estático debe usar `data-i18n`, `data-i18n-title`,
`data-i18n-placeholder` o `data-i18n-aria`; el dinámico debe usar `translate()`.
Conserva placeholders como `{path}` y `{count}` y no inyectes traducciones con
`innerHTML`.

Los diagnósticos nativos de Rust siguen en inglés hasta la localización de la
CLI 1.1. La interfaz debe explicar esta limitación.

## Documentación y validación

Antes de anunciar un idioma, traduce landing, primeros pasos, playground,
seguridad/recuperación, compatibilidad y esta guía. Añade navegación, sitemap,
canonical y `hreflang` solo para rutas reales.

```sh
npm ci
npm run typecheck
npm audit --audit-level=moderate
npm run build
```

Revisa desktop y móvil, teclado, foco, overflow, enlaces, metadatos y sitemap.
Una persona fluida debe aprobar significado y tono; el build solo demuestra la
forma del catálogo.
