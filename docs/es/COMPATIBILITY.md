# Política de compatibilidad

## Norminette oficial

`normfix` se prueba con la
[Norminette oficial](https://github.com/42school/norminette) `3.3.59`. Otra
versión continúa con el aviso `NORMINETTE_VERSION_UNTESTED`; usa
`--strict-norminette-version` para rechazarla en CI. El release incluye el
binario de `normfix`, no Python ni Norminette.

## Plataformas

Los binarios publicados cubren Linux x86-64/ARM64 y macOS Intel/Apple Silicon.
Windows no tiene binario nativo: usa
[WSL](https://learn.microsoft.com/windows/wsl/install) para la CLI completa.
El playground funciona directamente en navegadores modernos con WebAssembly y
módulos ES.

[Rust](https://www.rust-lang.org/tools/install) solo es necesario para compilar
las fuentes. El MSRV es `1.85`; el repositorio fija `1.97.1`.

## Límites de la promesa

Norminette es la autoridad de estilo. El compilador solo aporta diagnósticos de
sintaxis y warnings; `normfix preflight` no ejecuta recetas Make, no enlaza, no
ejecuta pruebas y no demuestra ausencia de leaks. El playground tampoco
ejecuta Norminette, compilador, Git ni transacciones de archivos.

Para automatización, usa `--format json` y valida `schema_version`. Comandos,
flags, IDs de reglas, códigos de salida y claves JSON se mantienen en inglés y
forman parte de la interfaz estable.
