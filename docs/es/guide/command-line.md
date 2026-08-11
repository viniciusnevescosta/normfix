# Línea de comandos

La interfaz sin subcomando es la forma más corta de formatear un proyecto. Los
subcomandos hacen la intención más clara en scripts y en revisiones interactivas.

```sh
normfix format src includes
normfix lint
normfix check main.c
normfix budget src
normfix preflight
normfix explain TOO_MANY_LINES
normfix undo --list
normfix undo --run RUN_ID
```

## Flujos

| Comando | Escribe archivos | Qué hace |
|---|---|---|
| `format` | sí | Aplica las ediciones aceptadas |
| `lint` | no | Informa diagnósticos sobre los bytes originales; no propone formato, cabecera, Makefile ni sustitución de Markdown |
| `check` | no | Ejecuta formato y lint en un búfer sombra |
| `budget` | no | Una ejecución de lint más una fila informativa de líneas/variables/parámetros por función analizada |
| `preflight` | no | Una ejecución orientada a check con la comprobación estricta del compilador activada; no ejecuta `make` ni el programa |
| `explain` | no | Imprime la explicación incluida en inglés de un ID de regla estable, sin analizar un proyecto |
| `undo` | sí | Lista o restaura una copia de seguridad de transacción íntegra |
| `uninstall` | sí | Elimina este binario y, con `--purge`, los datos que creó |

`undo` se niega a sobrescribir bytes cambiados después de la ejecución que
restaura. Sin `--run` selecciona el punto de recuperación válido más reciente tras
una confirmación interactiva; la restauración no interactiva exige `--force`.

## Opciones

| Opción | Comportamiento |
|---|---|
| `PATH...` | Cero, uno o muchos archivos/directorios; cero significa el directorio actual |
| `--check` | Planifica e informa cambios sin escribir |
| `--diff` | Imprime diffs unificados en la salida humana sin escribir |
| `--changed` | Selecciona cambios rastreados sin indexar más archivos no rastreados y no ignorados por Git |
| `--staged` | Selecciona solo rutas registradas como cambiadas en el índice de Git |
| `--interactive` | Previsualiza, muestra el diff de cada archivo cambiado y pregunta cuáles escribir |
| `--use-gitignore` | Respeta `.gitignore` durante el descubrimiento recursivo de directorios |
| `--login LOGIN` | Proporciona o restringe el login 42 usado en la validación de identidad |
| `--email EMAIL` | Proporciona el correo verificado de estudiante 42 usado en las cabeceras oficiales |
| `--no-backup` | Desactiva las copias retenidas para escrituras de formateo seguras y corrientes |
| `--backup-dir PATH` | Usa una base externa concreta de copias de seguridad |
| `--format human\|json` | Selecciona la salida de terminal o el informe JSON versionado |
| `--lang CODE` | Idioma de la salida humana: `en`, `pt`, `es` o `fr` |
| `--no-color` | Desactiva el color ANSI |
| `-v`, `--verbose` | Lista cada corrección aceptada en la salida humana |
| `--timeout SECONDS` | Tiempo límite de Norminette por invocación; por defecto: 5 segundos |
| `--threads N` | Número de procesos paralelos; por defecto: el hardware disponible |
| `--remove-invalid-comments` | Borra solo comentarios rechazados en ubicaciones oficiales exactas |
| `--remove-unused` | Elimina solo funciones `static` inalcanzables probadas en un proyecto completo |
| `--remove-unexpected` | Mueve archivos regulares inesperados a una cuarentena externa recuperable |
| `--unsafe` | Activa el conjunto cerrado de acciones arriesgadas/destructivas |
| `--force` | Confirma las capacidades destructivas solicitadas o reconoce un alcance protegido |
| `--no-reorder-includes` | Deja los bloques contiguos de include en su orden actual |
| `--no-format-markdown` | Analiza documentos README sin reimpresión canónica en CommonMark |
| `--no-cache` | Desactiva la caché externa persistente de análisis |
| `--norminette PATH` | Usa un ejecutable exacto de Norminette |
| `--strict-norminette-version` | Rechaza una versión del verificador distinta de la probada |
| `--no-compiler-preflight` | Omite la pasada consultiva estricta del compilador C, activa por defecto |
| `--cc PATH` | Usa un compilador C exacto para el preflight y el análisis |
| `--analyzer` | Añade el analizador acotado de GCC/Clang a los flujos corrientes; el preflight lo activa automáticamente |
| `-h`, `--help` | Muestra la ayuda incorporada |
| `-V`, `--version` | Muestra la versión de la CLI nativa |

`--check` y `--diff` son mutuamente excluyentes. `--changed` y `--staged` son
mutuamente excluyentes y no pueden combinarse con argumentos de ruta explícitos.
`--force` sin `--unsafe`, `--remove-unused` o `--remove-unexpected` es un error, a
menos que el propio alcance esté protegido. Las raíces del sistema de archivos,
el directorio personal completo, raíces amplias como `/Users` y `/home` y los
árboles del sistema operativo se niegan antes del descubrimiento sin ese
reconocimiento explícito.

## Orden de los includes

Una secuencia de directivas `#include` se reordena para que las cabeceras de
sistema vayan primero, luego las del proyecto, en orden alfabético dentro de cada
categoría:

```c
# include "libft.h"          # include <limits.h>
# include "ft_printf.h"  ->  # include <stdlib.h>
# include <stdlib.h>         # include "ft_printf.h"
# include <limits.h>         # include "libft.h"
```

::: warning El bloque debe ser demostrablemente contiguo
Una secuencia se reescribe solo mientras **cada** línea de ella sea exactamente
una directiva de include. La primera línea que sea otra cosa (un comentario, una
línea en blanco, un condicional, una definición de macro o texto tras el
delimitador final) termina la secuencia, y cada lado se ordena de forma
independiente. Ninguna directiva cruza tal construcción, porque hacerlo puede
cambiar declaraciones, macros de característica o compilación condicional.
:::

Los nombres se comparan sin distinguir mayúsculas y los nombres iguales conservan
su orden relativo original. `--no-reorder-includes` deja cada bloque intacto; el
informe recurre entonces a la advertencia `INCLUDE_ORDER_REVIEW`, que
`normfix explain INCLUDE_ORDER_REVIEW` describe sin conexión.

## Alcances de Git

La selección de alcance por Git ocurre antes del descubrimiento normal:

```sh
normfix check --changed
normfix format --staged
```

`--changed` significa cambios rastreados sin indexar más archivos no rastreados
que Git no ignora; deliberadamente no incluye rutas solo indexadas. `--staged`
usa el diff del índice para seleccionar nombres, y luego analiza y formatea los
bytes actuales del árbol de trabajo. No reescribe el índice ni indexa el
resultado.

Un alcance vacío es un no-op correcto y nunca recae en un escaneo de directorio
completo. Git se invoca directamente, con rutas delimitadas por NUL, un tiempo
límite, un límite de salida y comprobaciones de confinamiento de rutas. Los
nombres absolutos o que se escapan se rechazan. Un candidato que es un enlace
simbólico o que no es un archivo regular se omite con seguridad; un fallo de
metadatos o de Git rechaza el alcance entero en lugar de escanear en silencio
otro conjunto.

::: tip Un alcance no es una prueba
El alcance de Git es una comodidad de revisión, no una prueba de proyecto
completo. Los hallazgos de todo el proyecto que necesitan una instantánea cerrada
se desactivan cuando el alcance no puede proporcionarla.
:::

## Revisión interactiva

```sh
normfix format --interactive
```

La primera pasada es de solo lectura: `normfix` imprime el informe y el diff de
cada archivo propuesto, aceptando `y`, `n`, `a` (todos) o `q` (cancelar). Luego
analiza otra vez el mismo alcance seleccionado. Cada aprobación queda ligada a
los hashes de los bytes originales y propuestos exactos mostrados en la primera
pasada, y la transacción escribe solo los archivos cuyo plan de la segunda pasada
aún coincide con esa aprobación ligada a la instantánea.

El modo interactivo requiere un terminal humano y no puede combinarse con vista
previa, JSON, lint/budget ni operaciones arriesgadas/destructivas.

## Comportamiento de ignorado

Los escaneos recursivos respetan `.normfixignore` por defecto, usando el estilo
de ignorado de Git soportado por el crate `ignore`. El nombre heredado
`.norminetteignore` sigue soportado para que los proyectos existentes no
recuperen en silencio entradas ignoradas.

`.gitignore` es deliberadamente opcional, mediante `--use-gitignore`, porque los
archivos C ignorados todavía pueden afectar a pruebas de todo el proyecto. Los
argumentos explícitos de archivo siguen siendo explícitos y no se filtran por
archivos de ignorado.

## Códigos de salida

| Código | Significado |
|---:|---|
| `0` | El modo de corrección terminó sin diagnóstico bloqueante, o la entrada ya estaba limpia |
| `1` | Quedan diagnósticos manuales, o el modo de vista previa encontró cambios propuestos/candidatos a cuarentena |
| `2` | Fallo de descubrimiento, configuración, herramienta, E/S, transacción o cuarentena |
| `130` | Se canceló una revisión interactiva por archivo |

Los avisos informativos no hacen fallar una ejecución.
