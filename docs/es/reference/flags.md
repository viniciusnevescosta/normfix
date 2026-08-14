# Todas las flags

Cada entrada dice qué hace la flag, cuándo recurrirías a ella, y la muestra en
uso. Las flags son globales: funcionan con el comando pelado y con cada
subcomando.

Ejecuta `normfix --help` para la misma lista sin la prosa.

## Seleccionar qué procesar

### `PATH...`

Cero, uno o muchos archivos y directorios. Cero significa el directorio actual,
escaneado de forma recursiva sin seguir enlaces simbólicos.

```sh
normfix                                   # the whole project
normfix main.c                            # one file
normfix src includes                      # two directories
normfix src/parser.c includes/shell.h     # a mixture
```

Un argumento explícito de archivo siempre se procesa, aunque un archivo de
ignorado lo hubiera excluido.

### `--changed`

Procesa cambios rastreados sin indexar más archivos no rastreados que Git no
ignora.

```sh
normfix --changed
```

Úsalo mientras trabajas: formatea lo que acabas de tocar en vez de reescribir el
proyecto entero. Excluye deliberadamente las rutas solo indexadas.

### `--staged`

Procesa solo las rutas registradas como cambiadas en el índice de Git.

```sh
normfix check --staged
```

Lee el índice para seleccionar *nombres*, y luego analiza los bytes actuales del
árbol de trabajo. No reescribe el índice ni indexa el resultado, así que
`git diff --staged` no se ve afectado.

No puede combinarse con `--changed` ni con rutas explícitas. Un alcance vacío es
un no-op correcto, y nunca recae en escanearlo todo.

### `--use-gitignore`

También respeta `.gitignore` durante el descubrimiento recursivo.

```sh
normfix --use-gitignore
```

Desactivado por defecto, deliberadamente: un archivo C que le dijiste a Git que
ignorara sigue participando en pruebas de todo el proyecto, como la comprobación
de funciones permitidas. `.normfixignore` se respeta siempre.

## Previsualizar en vez de escribir

### `--check`

Planifica todo, no escribe nada.

```sh
normfix --check
normfix --check --format json > report.json
```

El código de salida `1` significa que hay trabajo por hacer, lo que lo convierte
en una puerta de CI de una línea.

### `--diff`

Imprime un diff unificado de cada cambio propuesto, y no escribe nada.

```sh
normfix --diff
normfix --diff src/parser.c
```

Las tabulaciones se muestran como `\t` para que los cambios de indentación sigan
siendo visibles. Mutuamente excluyente con `--check`.

### `--interactive`

Previsualiza cada archivo cambiado y elige cuáles se escriben.

```sh
normfix format --interactive
```

Responde `y`, `n`, `a` (todos) o `q` (cancelar). La aprobación queda ligada a los
bytes exactos que se te mostraron; si un archivo cambia debajo de ti, se omite en
vez de escribirse. Requiere un terminal, y se niega a combinarse con `--check`,
`--diff`, salida JSON o flags destructivas.

## Identidad para las cabeceras oficiales

### `--login LOGIN`

Proporciona o restringe el login 42 usado en la cabecera oficial.

```sh
normfix --login vneves-c
```

### `--email EMAIL`

Proporciona el correo verificado de estudiante 42. El correo es la fuente de la
verdad; el login se valida contra él.

```sh
normfix --email vneves-c@student.42.fr
```

Sin ninguna de las dos flags, `normfix` resuelve la identidad desde tu entorno y
la configuración de Git, y pregunta de forma interactiva cuando no puede y la
ejecución necesita una. Una identidad válida proporcionada explícitamente, o una
respuesta válida a ese aviso, se guarda atómicamente en la configuración privada
por usuario de la plataforma, para que las ejecuciones posteriores no vuelvan a
preguntar. Consulta [cabeceras oficiales](/es/reference/headers) para rutas y
permisos.

## Copias de seguridad y recuperación

### `--no-backup`

Omite las copias retenidas para escrituras corrientes de formateo.

```sh
normfix --no-backup
```

**No** omite la recuperación de una eliminación destructiva. Esas siempre exigen
almacenamiento externo y fallan cerradas sin él. Omitir las copias significa que
[`undo`](/es/commands/undo) no tiene nada que restaurar de esa ejecución.

### `--backup-dir PATH`

Usa una base externa concreta de copias en lugar de la predeterminada bajo
`$XDG_DATA_HOME`.

```sh
normfix --backup-dir ~/normfix-backups
```

El directorio no debe solaparse con el proyecto. Una ruta dentro de él, o por
encima de él, se rechaza, antes y después de resolver los enlaces simbólicos.

## Salida

### `--format human|json`

Salida de terminal, o el informe JSON versionado.

```sh
normfix --check --format json | jq '.summary'
```

Ramifica siempre por `schema_version` antes de leer el JSON. La disposición
humana puede mejorar entre versiones; la estructura del JSON no.

### `--lang`

Elige el idioma de la salida humana: `en`, `pt`, `es` o `fr`.

```sh
normfix check --lang es
```

```console
$ normfix check --lang es
normfix · iniciando
  acción            check
  modo              solo lectura
  alcance           /home/student/demo (recursivo)
...
Resumen: archivos: 1 | propuestos: 1 | escritos: 0 | correcciones: 1 | pendientes: 0 | informativos: 0 | con fallo: 0 | inesperados: 0 | 0 en cuarentena
Completado en 219 ms.
```

Sin la flag se usa la configuración regional del proceso — `NORMFIX_LANG`, luego
`LC_ALL`, `LC_MESSAGES` y `LANG` — con repliegue al inglés. Solo importa el
subtag primario, así que `es_ES.UTF-8` selecciona el español. Un valor de
`--lang` no publicado continúa en inglés con un aviso en lugar de fallar.

Esto cambia solo las explicaciones. Los nombres de comandos, las grafías de las
flags, los IDs de regla, los códigos de salida y todos los valores de
`--format json` permanecen idénticos en los cuatro idiomas, así que un script
nunca tiene que seleccionar un idioma para seguir funcionando.

Los mensajes de regla de los analizadores siguen en inglés. Una ejecución no
inglesa lo dice en una línea en vez de presentar un informe parcialmente
traducido como si estuviera completo.

### `--no-color`

Desactiva los colores ANSI incluso en un terminal.

```sh
normfix --no-color
```

Los colores ya están desactivados cuando la salida no es un terminal, o cuando
`NO_COLOR` está definido.

### `-v`, `--verbose`

Lista cada corrección aceptada en vez de solo el recuento.

```sh
normfix --check -v
```

Útil cuando quieres saber exactamente qué diecisiete correcciones recibió un
archivo.

## Ejecución

### `--threads N`

Fija el número de procesos paralelos. Por defecto, el hardware disponible.

```sh
normfix --threads 1
```

Usa `1` para que el orden de la salida sea trivialmente reproducible mientras
depuras. Los resultados y las escrituras se ordenan por ruta de todos modos, así
que el número de procesos nunca cambia el informe ni el orden en que se escriben
los archivos.

### `--timeout SECONDS`

Tiempo límite de Norminette por archivo. Por defecto `5`.

```sh
normfix --timeout 15
```

Súbelo en una máquina lenta o en un archivo muy grande. Un tiempo agotado es un
fallo operativo de ese archivo, no un diagnóstico.

### `--no-cache`

Desactiva la caché externa de análisis.

```sh
normfix --no-cache
```

La caché guarda resultados del verificador oficial fuera del proyecto, indexados
por los bytes de la fuente y la huella verificada del verificador. Desactívala
para forzar una reejecución completa; un fallo de caché ya falla abierto como una
ausencia.

### `--norminette PATH`

Usa un ejecutable exacto de Norminette en lugar de buscar en el `PATH`.

```sh
normfix --norminette ~/.local/pipx/venvs/norminette/bin/norminette
```

Se toma la huella de la versión. La versión `3.3.59` es la probada; otra versión
analizable continúa con un aviso destacado `NORMINETTE_VERSION_UNTESTED`.

## Comprobaciones del compilador

### `--strict-norminette-version`

Rechaza una versión de Norminette contra la que esta versión no se ha
verificado.

```sh
normfix --strict-norminette-version
```

El comportamiento por defecto sigue funcionando cuando un campus instala una
versión oficial más nueva, nombrando aun así la brecha de compatibilidad. El modo
estricto es útil para una CI reproducible que fija deliberadamente la `3.3.59`.
La grafía anterior `--allow-untested-norminette` permanece como un no-op oculto
durante la transición de las versiones candidatas.

### `--no-compiler-preflight`

Omite la pasada estricta `cc -fsyntax-only -Wall -Wextra -Werror`.

```sh
normfix --no-compiler-preflight
```

La pasada está activa por defecto y es solo de diagnóstico: nunca autoriza ni
rechaza una edición del formateador. Omítela cuando tu proyecto necesita flags de
compilación que el contexto inferido no puede aportar, y el ruido no es útil.

### `--cc PATH`

Usa un compilador exacto para la pasada estricta de sintaxis y para el analizador
profundo. El analizador es automático en `preflight`; los flujos corrientes
requieren `--analyzer`.

```sh
normfix --cc /usr/bin/gcc-14
```

El compilador se identifica por su banner de versión, así que un comando llamado
`gcc` que en realidad es Clang se trata como Clang.

### `--analyzer`

Ejecuta además el analizador estático profundo que trae tu compilador durante un
flujo corriente. `preflight` ya activa esta pasada acotada automáticamente.

```sh
normfix --analyzer
```

`normfix` elige las flags a partir del propio banner de versión del compilador,
no del nombre del comando:

| Compilador | Qué se ejecuta |
|---|---|
| GCC | `-fanalyzer` |
| Clang | `--analyze -Xclang -analyzer-output=text` |
| Cualquier otro | Nada; la ejecución informa `CC_ANALYZER_UNAVAILABLE` y continúa |

::: warning `/usr/bin/gcc` en macOS es Clang
Apple distribuye un comando `gcc` que responde `Apple clang version ...`.
Elegirlo con `--cc` no te da `-fanalyzer`. `normfix` lo detecta y usa el
analizador de Clang, así que la flag hace lo que querías decir de todas formas.
:::

Ambos analizadores son más lentos e informativos. Son automáticos en `preflight`
y opcionales en el resto. Pueden sugerir una fuga o un acceso inválido a lo largo
de una ruta; ninguno es prueba de ninguna de las dos cosas, y ninguno es jamás
prueba de su ausencia. Un analizador ausente nunca cambia el estado de salida.

Para un GCC de verdad en macOS, instala uno y apunta a él explícitamente:

```sh
brew install gcc
normfix preflight --cc "$(brew --prefix)/bin/gcc-14"
```

## Contenido que se reescribe

### `--no-reorder-includes`

Deja los bloques contiguos de `#include` en su orden actual.

```sh
normfix --no-reorder-includes
```

Por defecto, una secuencia de directivas de include se ordena con las cabeceras
de sistema primero, luego las del proyecto, en orden alfabético dentro de cada
una. Un bloque solo se reescribe mientras cada línea de él sea exactamente una
directiva de include, así que un comentario o un condicional termina la secuencia
y nada la cruza.

### `--no-format-markdown`

Deja los documentos README sin cambios.

```sh
normfix --no-format-markdown
```

Los archivos README se reimprimen como CommonMark canónico por defecto. Eso puede
producir un diff grande en la primera ejecución, que es el motivo habitual para
desactivarlo.

El documento se lee en el dialecto en que fue escrito, así que las listas de
tareas, las notas al pie, las tablas y el texto tachado vuelven como ellos
mismos. Leídos como CommonMark puro serían texto corriente, y la reimpresión
escaparía sus corchetes: `- [x] hecho` volvería como `- \[x\] hecho` literal.

## Operaciones destructivas

Cada una de estas borra o mueve algo. Todas conservan almacenamiento externo
recuperable, y todas exigen confirmación.

### `--remove-invalid-comments`

Borra solo los comentarios que el verificador oficial rechazó en ubicaciones
exactas.

```sh
normfix --remove-invalid-comments
```

No se toca nada más: un comentario que el verificador acepta nunca se elimina.

### `--remove-unused`

Elimina funciones `static` probadamente inalcanzables en el proyecto completo.

```sh
normfix --remove-unused
```

La prueba necesita que toda fuente del proyecto sea legible e inequívoca. Un solo
archivo ilegible desactiva el análisis entero en vez de producir una respuesta
parcial.

### `--remove-unexpected`

Mueve archivos regulares inesperados a la cuarentena externa.

```sh
normfix --remove-unexpected
```

No se borra nada: los archivos se mueven al almacenamiento de recuperación con su
ruta relativa preservada, y un destino existente nunca se sobrescribe.

### `--unsafe`

Activa el conjunto cerrado de arriba, más la compactación de comparaciones con
NULL, la eliminación de fuentes obsoletas del Makefile y el borrado de una
variable local que nada lee.

Esa última se rechaza siempre que la declaración guarda algo que se ejecuta.
`int n = g();` es una llamada, y un `malloc` ahí vería su fuga reparada por
accidente, convirtiéndose en un programa que tú no escribiste. Esos casos se
informan.

```sh
normfix --unsafe
```

Es un conjunto con nombre, no un interruptor abierto. No puede activar una
operación que no exista ya como su propia flag.

### `--force`

Confirma operaciones destructivas sin un aviso, o reconoce explícitamente un
alcance protegido del sistema/amplio.

```sh
normfix --unsafe --force
```

Para CI y scripts. `--force` por sí solo, sin ninguna flag destructiva, es un
error salvo que el alcance seleccionado esté protegido. Reconocer un alcance
protegido no crea ninguna capacidad destructiva; esas siguen exigiendo sus
propias flags.

## Entorno

### `NORMFIX_NO_UPDATE_CHECK`

Desactiva el aviso diario de versión.

```sh
export NORMFIX_NO_UPDATE_CHECK=1
```

El aviso solo aparece para salida humana interactiva y es silencioso ante un
fallo. Consulta [`upgrade`](/es/commands/upgrade) para saber exactamente qué
envía.

## Información

### `-h`, `--help`

```sh
normfix --help
normfix undo --help
```

### `-V`, `--version`

```sh
normfix --version
```
