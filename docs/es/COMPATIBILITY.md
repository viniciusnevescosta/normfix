# Política de compatibilidad

Este documento define qué considera `normfix` como soportado. Es
deliberadamente estrecho: las afirmaciones de compatibilidad forman parte del
modelo de seguridad y deben estar respaldadas por evidencia automatizada.

## Norminette oficial

El verificador probado es la
[Norminette oficial](https://github.com/42School/norminette) `3.3.59`.

`normfix` toma la huella de la versión del ejecutable antes del análisis. Una
versión distinta continúa, por defecto, con un aviso destacado
`NORMINETTE_VERSION_UNTESTED`; `--strict-norminette-version` la rechaza en una CI
con versión fijada. Esto no es una afirmación de compatibilidad con una versión
mínima, porque los nombres de los diagnósticos oficiales, las ubicaciones, el
comportamiento del analizador y las disposiciones aceptadas son entradas de la
capa nativa de compatibilidad. El aviso hace explícita esa garantía reducida.

La Norminette sigue siendo una dependencia externa. Los archivos de release
contienen el binario nativo de `normfix`, no Python ni el verificador oficial.

### Adoptar otra versión del verificador

Una actualización de la Norminette exige un cambio revisado que:

1. registre las notas de versión upstream y los cambios de nombres de reglas;
2. ejecute la suite nativa completa contra la versión candidata;
3. actualice las fixtures de salida oficial solo tras explicar cada diferencia;
4. verifique la idempotencia de las correcciones seguras y la ausencia de
   regresiones en proyectos representativos de 42;
5. actualice la constante exacta de versión, la instalación en CI, el README y
   este archivo;
6. se publique como una nueva versión de `normfix`.

Soportar un rango de versiones solo es apropiado después de que la CI demuestre
cada versión de ese rango y el oráculo tenga un adaptador explícito para
cualquier diferencia de protocolo.

### Cuando 42 se mueve primero

Una herramienta que rechaza todas las versiones menos una deja de funcionar para
todo el mundo el día en que la escuela actualiza. Por eso el comportamiento por
defecto continúa e informa `NORMINETTE_VERSION_UNTESTED`; una CI fijada puede
optar por el rechazo:

```sh
normfix --strict-norminette-version
```

El comportamiento por defecto es defendible, no un agujero en el argumento,
porque la propiedad que la herramienta realmente promete no depende de conocer
la versión: la prueba de regresión antes/después compara dos respuestas del
**mismo ejecutable**, así que una ejecución sigue sin poder dejar un archivo con
más diagnósticos oficiales de los que tenía al empezar. Lo que cuesta una
versión no verificada es la garantía de que las reglas nativas coinciden con
ella, que es exactamente lo que dice el aviso.

## Toolchain de Rust

- Versión mínima soportada de [Rust](https://www.rust-lang.org/tools/install)
  (MSRV): `1.85`.
- Toolchain del repositorio y de las releases: `1.97.1`, fijada en
  `rust-toolchain.toml`.

La CI comprueba la MSRV de forma independiente de la toolchain de desarrollo
fijada. Elevar la MSRV exige un cambio de release documentado, no una
actualización incidental de dependencias.

## Sistemas operativos y objetivos de release

Las releases precompiladas cubren los entornos Unix que usan los estudiantes de
42:

| Sistema operativo | Arquitectura | Archivo público de release |
|---|---|---|
| Linux | x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux | ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS | Intel | `normfix-x86_64-macos.tar.gz` |
| macOS | Apple Silicon | `normfix-aarch64-macos.tar.gz` |

Los nombres públicos de los archivos omiten deliberadamente los marcadores de
proveedor de Rust y las etiquetas de fabricante de la máquina. Los
identificadores de objetivo de la toolchain siguen siendo entradas internas de
compilación, no nombres de release ni de producto.

Windows no tiene un objetivo nativo de release. La CLI completa se soporta en
Windows a través de [WSL](https://learn.microsoft.com/windows/wsl/install), con
el archivo de Linux y una instalación de la Norminette dentro del mismo entorno
WSL. El playground en el navegador es la alternativa sin instalación para las
vistas previas del formateador. La ejecución nativa en PowerShell/CMD no está
soportada: la terminación acotada de subprocesos, el comportamiento de enlaces
simbólicos y rutas y las pruebas de transacción tienen hoy implementación y
evidencia de integración específicas de Unix, así que un binario nativo de
Windows exageraría el contrato.

## Diagnósticos de C y de compilación

La Norminette oficial es la autoridad de compatibilidad de estilo. Un compilador
C del sistema se ejecuta por defecto como un oráculo aparte, solo de
diagnósticos, para `-fsyntax-only -Wall -Wextra -Werror`. Las rutas de include
inferidas a partir de los directorios de headers no sustituyen las flags del
propio Makefile del proyecto, sus defines, entradas generadas, modo de lenguaje,
entradas del enlazador ni pruebas de ejecución.

El `-fanalyzer` de GCC es automático en `preflight` y opcional en los flujos
habituales. Sus hallazgos sobre el ciclo de vida de las asignaciones y el flujo
de control pueden sugerir una posible fuga o un acceso inválido, pero no son
prueba de que un comportamiento C arbitrario sea correcto ni de que un proyecto
esté libre de fugas.

`normfix preflight` no ejecuta recetas de Make, no enlaza un binario, no ejecuta
el programa ni las pruebas, y no invoca un verificador de fugas en tiempo de
ejecución. Informa explícitamente de esos pasos manuales pendientes.

## Compatibilidad con navegadores

El playground apunta a navegadores modernos con soporte estándar de WebAssembly
y de módulos ES. Su interfaz HTML/CSS/TypeScript deliberadamente pequeña y a la
antigua se construye como sitio estático con
[Vite 8.2.1](https://vite.dev/releases) fijado, y puede servirse localmente o
mediante Vercel. Su contrato de compatibilidad es el subconjunto nativo de
formateo y diagnóstico en memoria descrito en
[`web/README.md`](https://github.com/viniciusnevescosta/normfix/blob/main/web/README.md).
Puede construir una cabecera oficial a partir de una identidad indicada a esa
pestaña del navegador, y puede previsualizar C, headers, Makefiles y Markdown.
No incorpora ni emula la Norminette, un compilador, Git, pruebas de guardas de
header para todo el proyecto ni transacciones del sistema de archivos.

## Compatibilidad del informe

La interfaz humana agrupa los diagnósticos para facilitar la lectura y puede
mejorar entre versiones. La automatización debe usar `--format json` y comprobar
`schema_version`; el JSON conserva los hallazgos individuales. Una estructura
JSON incompatible exige incrementar la versión del esquema y añadir notas de
compatibilidad.

Conviene decir una consecuencia con claridad: la línea y la columna impresas
junto a un fragmento siguen la convención del compilador C y cuentan caracteres,
mientras que la Norminette oficial cuenta columnas de visualización. Ambas
discrepan en una línea indentada con tabulador. Ninguno de los dos números forma
parte de la superficie versionada, y lo que localiza el hallazgo es el acento
circunflejo bajo el código. Consulta
[Informes](/es/reference/reporting#leer-un-diagnóstico).

## Qué cubre el versionado

`normfix` sigue el Versionado Semántico. El número de versión describe las
siguientes superficies, y solo estas:

| Superficie | Cubierta | Qué significa un cambio incompatible |
|---|---|---|
| Flags y subcomandos de la línea de comandos | sí | Eliminar o renombrar uno, o cambiar lo que hace uno existente |
| Códigos de salida | sí | Cambiar el significado de `0`, `1`, `2` o `130` |
| Estructura del informe JSON | sí, mediante `schema_version` | Eliminar un campo o cambiar su tipo |
| Archivos de configuración (`normfix.toml`, `.normfixignore`) | sí | Cambiar cómo se interpreta una clave o un patrón existente |
| Disposición de copias de seguridad, journal y cuarentena | sí | Hacer que `undo` no pueda leer un punto de recuperación anterior |
| Qué fuentes se editan automáticamente | no | Las nuevas ediciones demostradas llegan en releases menores |
| Redacción, agrupación y texto de ayuda de los diagnósticos | no | Se mejoran continuamente |
| APIs de los crates de Rust | no | Cada crate define `publish = false` y es interno |
| La versión soportada de la Norminette | aparte | Cambiarla es un cambio de release documentado, nunca incidental |

Una nueva edición automática es una release menor, porque un formateador cuya
salida nunca cambiara no merecería la pena ejecutarse. Una ejecución que produce
un resultado oficial *peor* es un error en cualquier versión, y la prueba
diferencial existe precisamente para detectarlo.

La versión mínima soportada de Rust es una decisión de release, no un detalle de
compilación. Elevarla exige un cambio documentado; una dependencia que necesite
un compilador más nuevo se deja atrás.
