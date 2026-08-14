# Usar normfix desde un agente de IA

Esta página es el contrato operativo para agentes de código, agentes de editor,
bots de CI y otros llamadores no humanos. Evita que un agente convierta por
accidente una comprobación de estado en una escritura recursiva.

## La única regla que recordar

El comando pelado formatea el directorio actual de forma recursiva:

```sh
normfix
```

Por tanto, un agente debería empezar con una ruta explícita y un comando de solo
lectura:

```sh
normfix check /ruta/absoluta/al/proyecto --format json --no-color
```

Usa una ruta absoluta del proyecto. No confíes en un directorio de trabajo
heredado, sobre todo cuando el agente pueda haber arrancado en un directorio
personal, en el padre de un clon, en la raíz de un espacio de trabajo montado o
en un directorio del sistema.

## Comprobación de capacidades

Antes de la primera ejecución sobre un proyecto, registra las versiones de la
herramienta y del verificador:

```sh
normfix --version
norminette --version
normfix --help
```

`normfix` toma la huella de cada verificador. Cuando 42 publica una versión
distinta, la ejecución por defecto continúa y emite
`NORMINETTE_VERSION_UNTESTED`; un agente debe exponer esa garantía reducida. Usa
`--strict-norminette-version` solo cuando la persona usuaria o la política de CI
exija explícitamente la versión probada del verificador.

Al arrancar, el modo humano escribe un bloque de acción/configuración sin color en
`stderr`. El modo JSON escribe un evento JSON `execution_start` en `stderr` y
mantiene el informe final versionado como el único documento JSON en `stdout`.
Ninguno de los dos modos pregunta cuando stdin no es interactivo.

Lee el alcance de ese evento antes de hacer nada con el resultado. Es la
declaración de la propia ejecución sobre lo que estaba a punto de tocar, así que
un agente puede abortar una ejecución cuyo alcance no coincide con la tarea
recibida, en vez de descubrir el desajuste en el resumen.

Un alcance amplio o sensible del sistema operativo se rechaza antes de leer
ningún archivo:

```console
$ normfix check /
normfix
error: refusing to scan or modify protected scope `/` because it is a filesystem root; inspect the path and pass --force to acknowledge it explicitly
No unvalidated changes were written.
```

Eso es salida `2` sin informe JSON en `stdout`. Las raíces del sistema de
archivos, los directorios personales completos, los árboles del sistema operativo
y los directorios amplios con varios proyectos se rechazan así, y la comprobación
resuelve antes los enlaces simbólicos y `..`. No añadas `--force` para que
desaparezca el mensaje: el rechazo casi siempre significa que el alcance se
calculó mal, y `--force` es una decisión de la persona usuaria sobre una ruta que
ha inspeccionado.

El formateador corriente no necesita Rust. Se usa un compilador solo para
comprobaciones consultivas de preflight; sus hallazgos nunca autorizan una
edición.

## Flujo recomendado para agentes

1. Inspecciona el estado del repositorio y resuelve cualquier conflicto de fusión
   antes de formatear.
2. Ejecuta una vista previa legible por máquina contra un alcance explícito.
3. Lee `schema_version` antes de consumir campos del informe JSON.
4. Muestra a la persona usuaria los archivos propuestos, los diagnósticos
   restantes y cualquier fallo operativo.
5. Si las escrituras ya están autorizadas, ejecuta el mismo alcance explícito con
   `normfix format`.
6. Inspecciona el diff resultante y ejecuta la compilación/pruebas del propio
   proyecto.
7. Ejecuta `normfix check` otra vez. Una segunda pasada correcta no debería
   proponer ninguna edición.

```sh
project=/ruta/absoluta/al/proyecto
normfix check "$project" --format json --no-color > normfix-report.json
normfix format "$project" --no-color
git -C "$project" diff --check
normfix check "$project" --format json --no-color
```

No crees `normfix-report.json` dentro de un directorio de entrega de 42 salvo que
la persona usuaria lo quiera ahí: un archivo inesperado es en sí mismo un hallazgo
de evaluación. Usa un directorio de salida temporal o propio del agente.

## Leer el contrato JSON

El informe estable usa actualmente `schema_version: 2`. Campos útiles:

| Campo | Decisión del agente |
|---|---|
| `summary.changed` | Una vista previa encontró cambios de bytes que puede probar seguros |
| `summary.remaining` | Quedan hallazgos manuales/bloqueantes |
| `summary.failed` | Falló una operación de herramienta, descubrimiento, E/S o transacción |
| `summary.unexpected_files` | Se encontraron archivos fuera del conjunto aceptado de archivos de proyecto |
| `files[].failure` | Este archivo no se completó; no lo describas como corregido |
| `files[].after` | Diagnósticos contra el búfer sombra final |
| `files[].fixes` | Ediciones probadas propuestas o escritas para ese archivo |
| `identity.available` | Se puede crear o actualizar una cabecera oficial de 42 |
| `evaluation.conclusive` | Siempre `false`; nunca presentes la estimación como nota oficial |
| `evaluation.verdict` | `hard_fail` significa que se cumplió una regla objetiva de rechazo del preflight |
| `evaluation.hard_failures` | Evidencia exacta de ruta/línea/columna/regla que mostrar primero |

Los búferes de fuente y los diffs están intencionadamente ausentes del JSON. Usa
`normfix --diff /ruta/absoluta` cuando haga falta un parche legible por humanos.

El estado de salida forma parte de la API:

| Código | Significado |
|---:|---|
| `0` | Limpio, o una escritura completada sin problema bloqueante |
| `1` | Una vista previa encontró trabajo, o queda un hallazgo manual |
| `2` | La propia ejecución falló |
| `130` | Una persona canceló la revisión interactiva |

La salida `1` no es un fallo operativo. La salida `2` nunca puede esconderse tras
la afirmación de que el proyecto aprobó.

## Elegir un comando

| Objetivo | Comando |
|---|---|
| Vista previa exacta | `normfix --diff PATH` |
| Puerta de máquina | `normfix check PATH --format json --no-color` |
| Diagnosticar los bytes sin editar | `normfix lint PATH --format json --no-color` |
| Revisión previa a la defensa | `normfix preflight PATH --format json --no-color` |
| Margen de las funciones | `normfix budget PATH --format json --no-color` |
| Explicar una regla sin conexión | `normfix explain RULE` |
| Formatear un alcance autorizado | `normfix format PATH --no-color` |
| Restaurar una transacción de normfix | `normfix undo --list`, luego `normfix undo --run ID` |

`--changed` y `--staged` son cómodos para el árbol de trabajo propio de quien
desarrolla, pero seleccionan nombres a través de Git y analizan los bytes del
árbol de trabajo. Usa una ruta explícita para una evaluación completa y un
alcance de Git para una edición focalizada.

## Autoridad y flags destructivas

Estas opciones piden capacidades materialmente distintas:

- `--remove-invalid-comments` borra solo comentarios rechazados en ubicaciones
  oficiales exactas;
- `--remove-unused` elimina solo funciones `static` inalcanzables bajo una prueba
  cerrada de proyecto;
- `--remove-unexpected` mueve archivos a una cuarentena externa recuperable;
- `--unsafe` activa el conjunto cerrado y documentado de limpiezas destructivas;
- `--force` aporta confirmación no interactiva para esas capacidades.

Un agente no puede inferir permiso para ellas a partir de una petición de
comprobar, formatear, evaluar o "corregir errores de Norma". Previsualizar un
plan destructivo también exige la capacidad, porque el análisis está
intencionadamente condicionado a la autorización.

Nunca borres datos de copia de seguridad o de cuarentena para que un informe
parezca limpio. Usa `normfix undo` para recuperar, e informa de la ruta del
journal si la reversión necesita revisión manual.

## Límites de la evaluación

`preflight` combina el resultado oficial de la Norma, comprobaciones de archivos
de proyecto, diagnósticos estrictos del compilador, comprobaciones de política y
una pasada automática y acotada del analizador del compilador. Es una ayuda
fuerte de revisión, no una nota concluyente de 42. No conoce el PDF de la
asignatura, no ejecuta una lista de verificación de defensa, no prueba la
corrección algorítmica y no prueba la ausencia de fugas. No ejecuta recetas de
Make, un binario producido, `clang-tidy` ni sanitizers. Ejecuta el Makefile del
propio proyecto, las pruebas, la compilación con sanitizer y el tester específico
de la asignatura por separado, y solo cuando la persona usuaria autorice la
ejecución de ese proyecto.

No trates la presencia o ausencia de un README como una regla universal de
aprobado/suspenso. Cuando exista uno, verifícalo contra las secciones exigidas
por la asignatura actual. Del mismo modo, `MAKEFILE_NOT_FOUND` es consultivo
hasta que la política de la asignatura pruebe que se exige un Makefile. No
informes de una corrección propuesta en la sombra como un aprobado del preflight:
la evaluación suspende por los hallazgos originales en disco de Norminette y del
Makefile.

## Higiene de terminal y CI

- Prefiere `--format json --no-color` para analizadores y salida redirigida.
- Nunca analices la tabla humana decorativa cuando el JSON esté disponible.
- Define `NORMFIX_NO_UPDATE_CHECK=1` en CI hermética o sin red.
- Mantén las versiones del verificador oficial y de `normfix` en los registros de
  CI.
- No canalices un comando de escritura por un filtro que oculte su estado de
  salida.
- No ejecutes contra `/`, `/System`, `/usr`, `/etc`, un directorio personal o un
  espacio de trabajo con varios proyectos. Selecciona la raíz real de la entrega.

Para cada opción y límite de prueba, continúa en
[Todas las flags](/es/reference/flags),
[Seguridad y recuperación](/es/reference/safety),
[Informes](/es/reference/reporting) y [Arquitectura](/es/ARCHITECTURE).
