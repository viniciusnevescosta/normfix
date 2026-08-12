# Guía de localización

Este es el contrato para quien contribuye traduciendo cada superficie de normfix
que lee una persona. Los primeros idiomas publicados son inglés (`en`),
portugués (`pt`), español (`es`) y francés (`fr`). Un idioma solo está completo
cuando alguien puede instalar la herramienta, entender su límite de seguridad,
usar el playground del navegador y seguir la documentación central sin caer en
el inglés.

La localización no puede cambiar la interfaz de máquina. Traduce explicaciones,
no identificadores.

## Qué permanece en inglés

Estos valores son tokens estables de API o de código fuente y deben quedar
inalterados en todos los idiomas:

- el comando `normfix` y los nombres de los subcomandos;
- flags como `--check`, `--changed` y `--format json`;
- IDs de regla como `TOO_MANY_LINES` y `MAKEFILE_SOURCE_NOT_FOUND`;
- claves JSON, valores de enum, `schema_version` y códigos de salida;
- claves de configuración, nombres de variables de entorno y nombres de archivo;
- identificadores C, comandos de shell, rutas, nombres de archivos comprimidos y
  ejemplos de código;
- mensajes de commit de Git y comentarios de código en Rust/TypeScript.

Mantén sin cambios los nombres oficiales de producto: Norminette, Rust, WSL,
Clang, Vite, Monaco, Git, GitHub y Vercel. Traduce la frase que los rodea y
conserva el enlace oficial.

## Superficies actuales

| Superficie | Origen del texto traducido | Comportamiento publicado |
|---|---|---|
| Playground del navegador | `web/src/i18n.ts` y atributos `data-i18n*` en `web/index.html` | Interfaz completa en `en`, `pt`, `es` y `fr`; elegir un idioma solo cambia el idioma, y la elección se recuerda hasta que se cambia |
| Playground instalado | Un web app manifest por idioma, generado en `web/vite.config.ts` | Cada idioma se instala con su propio nombre, identidad y URL de inicio, así que un playground instalado abre en el idioma que eligió su lector |
| Documentación | Árboles de idioma bajo `docs/`, más la navegación por idioma en `docs/.vitepress/config.ts` | Landing, instalación, playground, seguridad, compatibilidad y rutas de contribución localizadas, con el inglés como fallback explícito para páginas aún no publicadas |
| SEO | Head/config de VitePress, `web/index.html`, sitemaps y `robots.txt` | URLs canónicas y `hreflang` solo para páginas que existen de verdad |
| CLI nativa | Catálogo en `crates/normfix-i18n`, seleccionado con `--lang` o por el locale del proceso | Anuncio, texto del informe, avisos de seguridad, artículos de `explain` y los diagnósticos de este proyecto en `en`, `pt`, `es` y `fr`; los hallazgos retransmitidos del verificador oficial o del compilador C quedan tal como los produjeron esas herramientas; comandos, flags, JSON, IDs de regla y códigos de salida siguen siendo neutros de idioma |

## Traducir el playground

1. Añade el código del idioma a `SUPPORTED_LOCALES` en `web/src/i18n.ts`.
2. Rellena cada `MessageKey`. No publiques un idioma que herede en silencio un
   botón, un error de validación, una declaración de privacidad o una etiqueta de
   accesibilidad en inglés.
3. Pon el texto estático del HTML detrás de `data-i18n`, `data-i18n-title`,
   `data-i18n-placeholder` o `data-i18n-aria`. Pon el texto dinámico detrás de
   `translate()`; no dejes literales de texto orientado al usuario en
   `web/src/main.ts` ni en `web/src/editor.ts`.
4. Usa placeholders con nombre como `{path}` y `{count}`. Cada traducción debe
   conservar exactamente el mismo conjunto de placeholders que el inglés.
5. Cuando un mensaje contiene un recuento, no escribas una sola frase con un
   placeholder dentro. Añade una entrada por categoría plural del CLDR —
   `importedOne`, `importedOther` — y renderízala con `translatePlural`, para que
   el sustantivo concuerde con su número en lugar de leerse “1 archivos
   añadidos”. Un mensaje pluralizado debe depender de exactamente un recuento;
   una frase con dos no puede concordar en todos los idiomas, así que escribe dos
   frases.
6. Formatea números y fechas con el idioma seleccionado. No localices la marca de
   tiempo fija de la cabecera de 42 ni otro texto de protocolo.
7. Define el valor `lang` del documento y ofrece un selector de idioma visible.
8. Nunca inyectes una traducción con `innerHTML`. Sigue usando `textContent` y
   nodos del DOM, para que el texto traducido o versionado no pueda convertirse
   en markup.
9. Prueba el fallback de textarea en pantalla estrecha, además de Monaco. Monaco
   en sí no define la completitud de la localización del producto. La ruta sin
   conexión usa ese mismo fallback, así que es también lo que ve un lector al
   abrir el playground instalado sin red.
10. Traduce el nombre de la aplicación en `localizedPages`, en
    `web/vite.config.ts`. Es la etiqueta bajo el icono de quien instale el
    playground, así que debe ser corta y leerse como un nombre, no como un título
    de página.

Los diagnósticos nativos de Rust devueltos por WebAssembly siguen en inglés. La
interfaz debe decirlo con claridad, en lugar de presentar un diagnóstico
parcialmente traducido como una localización completa.

## Traducir la documentación

Usa la página en inglés como fuente de verdad. Conserva los títulos que son
destino de enlace, salvo que la configuración del idioma proporcione también una
redirección probada. Mantén los ejemplos de comandos válidos byte a byte; traduce
solo la prosa que los rodea y la salida humana esperada.

Para un idioma nuevo:

1. crea su directorio y traduce la landing page;
2. traduce primeros pasos, la guía del playground del navegador,
   seguridad/recuperación, compatibilidad y esta guía de localización antes de
   anunciar el idioma;
3. añade etiquetas, navegación, sidebar, etiquetas de búsqueda, pie de página y
   texto del enlace de edición localizados en VitePress;
4. enlaza las páginas oficiales de Norminette, Rust, WSL y Clang allí donde esas
   herramientas se nombren como dependencias;
5. añade metadatos canónicos y de idioma alternativo solo entre páginas
   traducidas equivalentes;
6. incluye cada URL localizada publicada en el sitemap generado;
7. verifica cada enlace interno y cada bloque de código en la compilación de
   producción.

No crees una página fina cuyo único contenido sea una redirección automática al
inglés y lo llames traducción. Un enlace explícito “Esta página está disponible
en inglés” es un fallback temporal aceptable cuando la ruta localizada no se
anuncia como completa.

## Traducir la CLI nativa

El crate `crates/normfix-i18n` es dueño de la selección de idioma y del catálogo.
El texto traducido vive ahí, nunca dentro del código que decide qué decir.

La completitud la garantiza el compilador, no la revisión. Cada idioma es un
único literal de struct `Messages`, así que una entrada nueva que algún idioma no
traduzca es un error de compilación. Dos pruebas cubren lo que el sistema de
tipos no alcanza: ninguna entrada puede estar vacía, y cada traducción debe
llevar el mismo conjunto de `{placeholder}` que su original en inglés. Los
placeholders tienen nombre, no posición, así que una traducción puede
reordenarlos.

Para añadir una entrada:

1. añade el campo a `Messages` con un comentario de documentación que nombre sus
   placeholders;
2. complétalo en los cuatro literales de idioma en el mismo cambio;
3. renderízalo mediante `messages.<campo>` y `normfix_i18n::fill`, nunca como un
   literal en el punto de llamada.

La selección de idioma sigue `--lang`, luego `NORMFIX_LANG`, `LC_ALL`,
`LC_MESSAGES` y `LANG`, y después el inglés. Solo importa el subtag primario, así
que `pt_BR.UTF-8` selecciona portugués. Un valor de `--lang` no publicado recurre
al inglés con un aviso conciso; un locale de proceso no publicado recurre en
silencio, porque una pista no es una decisión. Ninguno de los dos casos es fatal:
el idioma de la salida no puede ser motivo para negarse a analizar un proyecto.

El JSON nunca se localiza. El evento `execution_start` y el informe final
conservan valores en inglés en todos los idiomas, así que un script nunca tiene
que elegir un idioma para seguir siendo fiable.

### Qué se traduce y qué nunca se traducirá

Traducido: el anuncio de la ejecución, la prosa del propio informe, todos los
avisos críticos de seguridad, los artículos de `explain` y los diagnósticos que
escribe este proyecto.

Nunca traducido: un hallazgo retransmitido de la Norminette oficial o del
compilador C. Ese texto es la salida de esas herramientas. Reescribirlo haría que
el informe contradijera lo que imprime ejecutar `norminette` directamente, lo
cual es peor que leer una frase en inglés. Una ejecución en otro idioma lo dice
en una línea: como un hecho sobre de dónde vienen esas palabras, no como una
disculpa por una traducción que falta.

Los tokens de estado de la tabla de archivos (`CLEAN`, `WOULD FIX`, `REVIEW`,
`FAILED`) y las palabras de severidad siguen en inglés, junto a los IDs de regla
con los que aparecen.

Para traducir un diagnóstico nuevo, añade un `DiagnosticKey`, complétalo en los
cuatro `match` de idioma y constrúyelo con `localized_text`. El inglés se produce
siempre, porque es lo que llega al JSON y lo que usan la igualdad y la
ordenación.

## Terminología y tono

- Usa el vocabulario que los estudiantes ya ven en 42.
- Prefiere frases cortas y directas en avisos y botones.
- Mantén precisa la distinción entre **advertencia**, **fallo**, **inseguro**,
  **recuperable**, **informativo** y **concluyente**.
- No traduzcas “safe” como “garantizadamente correcto”. Significa que la prueba
  documentada de esa edición pasó.
- No traduzcas la estimación previa a la defensa como una nota oficial.
- Conserva la afirmación de que la identidad en el navegador es configuración
  local del dispositivo, no un secreto cifrado.

Cuando un término sea discutible, actualiza un pequeño glosario en las notas de
contribución de ese idioma y usa una grafía consistente entre el playground y la
documentación.

## Validación

Ejecuta las comprobaciones completas del sitio tras cualquier cambio de
localización:

```sh
npm ci
npm run typecheck
npm audit --audit-level=moderate
npm run build
```

Después revisa cada idioma en ancho de escritorio y en pantallas estrechas.
Comprueba el acceso por teclado, las etiquetas de foco, el desbordamiento de
texto, la redacción de plurales y recuentos, el comportamiento del botón de
copiar código, los enlaces rotos, las URLs canónicas, `hreflang` y el sitemap.
Alguien con fluidez en el idioma de destino debe aprobar el significado y el
tono; una compilación de TypeScript que pasa solo demuestra la forma del
catálogo.

Para un cambio en el catálogo de la CLI, ejecuta también las pruebas del
workspace de Rust, Clippy con advertencias denegadas, rustdoc con advertencias
denegadas y las fixtures del esquema JSON.

## Lista de comprobación del pull request

- [ ] Cada texto nuevo orientado a personas está en el catálogo correcto.
- [ ] Comandos, flags, IDs de regla, claves JSON y ejemplos de código quedan sin
      cambios.
- [ ] Los nombres de los placeholders y el significado de seguridad coinciden con
      el inglés.
- [ ] Navegación, etiquetas de accesibilidad, metadatos y rutas de error están
      traducidos.
- [ ] Las entradas canónicas, `hreflang` y del sitemap apuntan solo a páginas
      reales.
- [ ] Se conservan los enlaces a las dependencias oficiales.
- [ ] Pasan los controles del sitio y de Rust relevantes para el cambio.
- [ ] Alguien con fluidez revisó el resultado renderizado, no solo el diff.
