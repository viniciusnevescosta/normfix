# `normfix preflight`

Las comprobaciones de solo lectura que conviene ejecutar justo antes de una
evaluación de 42, con la pasada estricta del compilador activada.

```sh
normfix preflight
```

Ejecuta todo lo que ejecuta [`check`](/es/commands/check), más
`cc -fsyntax-only -Wall -Wextra -Werror` contra las unidades de traducción reales
en disco.

```console
$ normfix preflight
error[CC_IMPLICIT_FUNCTION_DECLARATION]: 2 occurrences in 2 files
  srcs/sort/sort.c:30:5           call to undeclared function 'sort_medium'
  srcs/sort/sort_adaptive.c:21:5  call to undeclared function 'sort_medium'
    note: Compiler diagnostics inspect the original on-disk translation unit
          and never authorize or reject formatter edits.
 = help: Fix this strict compiler diagnostic, then rerun normfix.
 = source: C compiler
```

Ese ejemplo es real: una cabecera declaraba `sort_medium` pero ningún archivo lo
definía, así que el proyecto no compilaba. Norminette nunca te lo habría dicho.

## Una ejecución completa, antes y después

Toda la salida de esta página viene de una ejecución real. El proyecto de abajo
tiene cuatro archivos: `main.c` y `add.c` indentados con espacios, un `demo.h`
que declara un `unused_api` que nadie implementa, y un Makefile cuyo `SRC` aún
lista un `ghost.c` que se borró.

Preflight dice qué va a hacer antes de leer nada:

```console
$ normfix preflight
normfix · starting
  action       preflight
  mode         read-only check
  scope        /home/student/demo (recursive)
  working dir  /home/student/demo
  identity     student@student.42.fr (user config)
  workers      auto
  checks       Norminette + strict compiler
  norminette   automatic PATH discovery
  version rule advisory (other releases continue)
  timeout      5s per file
  cache        enabled
  gitignore    not applied
  backups      not used (read-only run)
  destructive  none
  force        no
```

Después informa la estimación contra los bytes que hay ahora en disco:

```console
Pre-defense estimate: HARD FAIL | grade FAIL | 31/100
This estimate is heuristic and never replaces the official evaluation.
Hard-fail evidence
  Makefile:1:1 [INVALID_HEADER] The official 42 Makefile header is missing or malformed
  add.c:1:1 [INVALID_HEADER] Missing or invalid 42 header
  demo.h:1:1 [INVALID_HEADER] Missing or invalid 42 header
  main.c:1:1 [INVALID_HEADER] Missing or invalid 42 header
  Makefile:2:20 [MAKEFILE_SOURCE_NOT_FOUND] The literal Makefile source `ghost.c` does not exist below the project root.
  add.c:3:4 [SPACE_BEFORE_FUNC] Found space when expecting tab before function name
  main.c:3:4 [SPACE_BEFORE_FUNC] Found space when expecting tab before function name
  main.c:5:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:5:8 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:7:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:8:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:5:1 [TOO_FEW_TAB] Missing tabs for indent level
  main.c:7:1 [TOO_FEW_TAB] Missing tabs for indent level
  main.c:8:1 [TOO_FEW_TAB] Missing tabs for indent level
```

La mayor parte de esa lista es exactamente lo que `normfix` repara. Ejecutando la
corrección por defecto y preguntando otra vez:

```console
$ normfix
$ normfix preflight
Pre-defense estimate: HARD FAIL | grade FAIL | 59/100
This estimate is heuristic and never replaces the official evaluation.
Hard-fail evidence
  Makefile:14:20 [MAKEFILE_SOURCE_NOT_FOUND] The literal Makefile source `ghost.c` does not exist below the project root.
```

Trece suspensos desaparecieron y queda uno, y ese es el resultado útil: el
`ghost.c` borrado sigue listado en el Makefile, y ninguna herramienta debería
decidir por su cuenta si ese archivo debe volver o si la línea debe irse. El
veredicto sigue siendo `HARD FAIL` mientras quede cualquier suspenso — la nota se
mueve, el veredicto no se ablanda.

Los bytes evaluados son los bytes entregados. En la primera ejecución `normfix`
ya había calculado las correcciones de todos los `INVALID_HEADER` y
`SPACE_REPLACE_TAB` de arriba, y la estimación aun así suspendió por ellos,
porque una reparación que no has escrito no forma parte de lo que abrirá un
evaluador.

Todo flujo respaldado por el sistema de archivos, incluida la comprobación por
defecto, compara prototipos no estáticos de las cabeceras del proyecto con cada
archivo C o cabecera del proyecto que se pudo leer sin error. Una implementación
ausente, o una definición correspondiente cuyo cuerpo son solo llaves, espacios y
comentarios, se resalta en el nombre del prototipo. Las fuentes generadas y las
bibliotecas externas siguen siendo ambiguas. El modo `--unsafe` explícitamente
autorizado elimina solo un prototipo sin implementación cuando el conjunto
completo de fuentes no contiene ninguna definición, llamada, puntero a
función/referencia, macro, cadena, condicional, atributo o pegado de tokens como
evidencia. Una definición existente con solo trivialidades es únicamente una
advertencia, porque un no-op intencionado puede ser válido.

## Estimación y reglas de suspenso

El informe termina con una estimación de 0 a 100, una banda de nota y un
veredicto. Siempre está etiquetado como **no concluyente**. Es una ayuda de
priorización, no una nota de 42 predicha.

El veredicto es `HARD FAIL` cuando está presente cualquiera de estas condiciones
objetivas:

- un archivo inesperado en el alcance evaluado;
- un hallazgo de Norma corroborado por la Norminette oficial instalada;
- un diagnóstico estático de Makefile o un fallo de procesamiento del Makefile.

Cada elemento de suspenso de fuente repite su `ruta:línea:columna` exacta, el ID
de la regla y el mensaje. Un fallo operativo de Makefile nombra el archivo sin
inventar una coordenada de fuente.
Los hallazgos oficiales de Norma y de Makefile se evalúan contra los bytes
originales en disco; una corrección propuesta de solo lectura no convierte la
entrega actual en un aprobado. Los hallazgos nuevos que permanecen en la sombra
final también se incluyen.
La ausencia de README no es un suspenso. Cuando hay un README presente, un aviso
informativo te pide compararlo con la ficha de asignatura/evaluación actual.
Si no se selecciona ni se encuentra un Makefile regular en la raíz del proyecto,
`MAKEFILE_NOT_FOUND` informa de que las comprobaciones de objetivos de
compilación y de lista de fuentes no se ejecutaron. Es un aviso y no cuesta
nota: un ejercicio de piscina espera contener solo archivos `.c`, así que el
Makefile y las cabeceras del proyecto son ambos opcionales allí. Solo la
asignatura puede decir si se exige un Makefile, y normfix no lee asignaturas.

## Qué no hace

No ejecuta `make`, no enlaza un binario, no ejecuta tu programa ni tus pruebas, y
no prueba la ausencia de fugas. Eso sigue siendo tuyo, y el informe lo dice.

No ejecuta sanitizers, ni `make` (ni siquiera `make -n`, que puede evaluar
`$(shell ...)`), ni un binario del proyecto. Eso pediría una confianza aparte y
explícita en lo que hacen la compilación y la ejecución del proyecto. Preflight
sí muestra una receta práctica de compilación de depuración con AddressSanitizer
y UndefinedBehaviorSanitizer.

## La lente de clang-tidy

Si la máquina tiene `clang-tidy`, preflight lee cada fuente C a través de él y
muestra lo que vio como entradas `TIDY_`. Es una lente, nunca una dependencia:
una máquina sin él funciona exactamente igual que antes, y `--clang-tidy <RUTA>`
señala un ejecutable exacto cuando hay varios instalados o ninguno en el `PATH`.

Pide las dos familias de comprobaciones que dicen algo sobre el programa: el
analizador estático, donde viven las fugas y el uso después de liberar, y las
comprobaciones de código propenso a error. El resto de lo que trae `clang-tidy`
es estilo de casa para C++, que discutiría con la Norm, y aquí manda la Norm.

Sus hallazgos son informativos y siguen siéndolo: nunca autorizan una edición,
nunca cuentan para lo que queda por corregir y nunca rechazan un resultado de
formateo.

Preflight añade automáticamente una pasada acotada de análisis estático profundo:
`-fanalyzer` en GCC, `--analyze` en Clang. Los flujos corrientes siguen
requiriendo `--analyzer`. `normfix` elige a partir del banner de versión del
compilador, lo que importa porque `/usr/bin/gcc` en macOS es Clang con otro
nombre.

Pueden *sugerir* una fuga o un acceso inválido; nunca prueban la corrección, y
nunca autorizan una edición. Un compilador sin ningún analizador informa
`CC_ANALYZER_UNAVAILABLE` y la ejecución continúa.

## Las pasadas estáticas y una ejecución con leaks responden cosas distintas

Las pasadas de arriba leen todos los caminos que el código podría tomar, así que
pueden señalar una fuga en un camino que ninguna ejecución tuya recorre. Un
verificador de fugas ejecuta el programa una vez y cuenta lo que esa ejecución
hizo de verdad: certeza sobre esa ejecución, y silencio sobre todo camino al que
no llegó.

Valen los dos, y preflight a propósito ejecuta solo el primer tipo. Ejecutar el
binario de un proyecto es ejecutar lo que sea que haga, y por eso
[`normfix leaks`](/es/commands/leaks) es un comando aparte que pregunta antes.
Preflight dice qué añadiría una ejecución con leaks, en vez de decidirlo por ti.

`preflight` se niega a combinarse con `--no-compiler-preflight`, porque la pasada
del compilador es la razón de ser del comando.
