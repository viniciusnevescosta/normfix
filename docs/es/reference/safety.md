# Seguridad, recuperación y operaciones destructivas

## Cada ejecución dice lo que va a hacer

Antes de leer un solo archivo, `normfix` imprime la acción, el ámbito resuelto y
la configuración de seguridad que está realmente en vigor:

```console
$ normfix --unsafe --force
normfix · starting
  action       format
  mode         write
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
  backups      automatic external backup
  destructive  invalid comments, NULL-check compaction, missing or trivia-only Makefile entries, orphan header prototypes, unreachable static functions, unexpected-file quarantine
  force        acknowledged
```

La línea `destructive` nombra cada capacidad que la ejecución realmente tiene,
así que `--unsafe` nunca se amplía en silencio.

La línea `scope` es la que hay que leer. Un comando escrito en el directorio
equivocado se ve mal aquí, antes de tocar nada, en lugar de aparecer en el
resumen posterior. Con `--format json`, esa misma información es el primer
evento en la salida estándar, así que un agente puede rechazar una ejecución
cuyo ámbito no pretendía.

## Ámbitos protegidos

Las raíces del sistema de archivos, los directorios personales completos, los
árboles del sistema operativo y los directorios amplios con varios proyectos se
rechazan de entrada:

```console
$ normfix check /
normfix
error: refusing to scan or modify protected scope `/` because it is a filesystem root; inspect the path and pass --force to acknowledge it explicitly
No unvalidated changes were written.

$ normfix check ~
normfix
error: refusing to scan or modify protected scope `/home/student` because it is the complete user home directory; inspect the path and pass --force to acknowledge it explicitly
No unvalidated changes were written.
```

Ambos terminan con estado `2` y no leen nada. La comprobación resuelve primero
los enlaces simbólicos y colapsa los `..`, así que una ruta como `/work/../etc` o
un enlace que apunte a `/etc` se rechaza por el mismo motivo que un `/etc`
literal. Una ejecución con ámbito de Git se juzga por la raíz del repositorio, no
por los archivos que selecciona, así que `--git-changed` desde un directorio
personal se rechaza en lugar de recorrer en silencio todos los proyectos que
haya dentro.

`--force` reconoce un ámbito protegido y nada más. No concede por sí mismo una
capacidad destructiva, y una capacidad destructiva sigue necesitando su propia
opción:

```console
$ normfix --force
normfix
error: --force requires --unsafe, --remove-unused, --remove-unexpected, or a protected system scope
```

## Listas de funciones permitidas

Los proyectos con una lista de funciones permitidas propia del subject pueden
añadir un `normfix.toml` en la raíz del proyecto:

```toml
[project]
name = "get_next_line"
allowed = ["read", "malloc", "free"]
```

El analizador acotado interpreta deliberadamente solo el `name` entre comillas y
el array `allowed` de identificadores entre comillas. Cuando se selecciona un
ámbito de C/headers, `normfix` descubre por su cuenta el conjunto completo de
archivos C/header del proyecto desde su raíz, considerando archivos regulares sin
seguir enlaces simbólicos y con los filtros de `.gitignore`, `.normfixignore` y
`.norminetteignore` desactivados. Cada archivo descubierto debe ser UTF-8
legible y analizarse sin pérdidas. Las definiciones no `static` de esa instantánea
cerrada autorizan llamadas entre unidades de traducción; las definiciones en el
mismo archivo se tratan localmente, mientras que una definición `static` en otro
archivo nunca autoriza la llamada.

Las llamadas candidatas se recalculan contra el código sombra final, para que los
rangos informados sigan siendo correctos tras la inserción de la cabecera y el
formateo. Los parámetros, las llamadas por puntero a función, la ambigüedad de
macro o preprocesador y los identificadores en mayúsculas con aspecto de macro
fallan en cerrado en lugar de producir una conjetura. Si el descubrimiento, la
lectura, el análisis, la ausencia de pérdidas o la revalidación de la instantánea
queda incompleta, se desactivan todos los hallazgos de la lista permitida y
`FUNCTION_POLICY_PROOF_INCOMPLETE` explica por qué. El propio `normfix.toml` debe
ser un archivo regular acotado y no un enlace simbólico. La política sigue sin
sustituir al subject del proyecto ni al evaluador.

## Comentarios y capacidades destructivas

Los comentarios rechazados como `WRONG_SCOPE_COMMENT` o `COMMENT_ON_INSTR` solo
se informan por defecto. `--remove-invalid-comments` borra únicamente un
comentario encontrado exactamente en la línea y la columna de visualización que
informa el verificador oficial. Nunca elimina la cabecera oficial, y la huella de
los tokens de código restantes debe permanecer inalterada.

`--unsafe` también borra una variable local que nada lee, y la demostración
deliberadamente no es la del compilador. `-Wunused-variable` salta con
`int n = g();` igual que con `int n;`, y borrar la primera borra una llamada —
una declaración con un `malloc` vería su fuga reparada por accidente,
convirtiéndose en un programa que tú no escribiste. Esas se conservan y se
informan. Un nombre califica cuando aparece exactamente una vez en todo el
archivo, contado en el texto crudo, porque un cuerpo de macro que lo menciona es
texto que ningún árbol de análisis muestra.

`--remove-unused` y `--remove-unexpected` piden capacidades destructivas más
fuertes:

- la eliminación de funciones no usadas considera solo definiciones `static`;
- exige que las entradas seleccionadas sean iguales al conjunto completo de
  `.c`/`.h` del proyecto;
- la recuperación del analizador, los bytes desconocidos, la ambigüedad del
  preprocesador, el pegado de tokens, los atributos, las referencias basadas en
  cadenas, las definiciones duplicadas o un grafo de referencias incierto
  conservan la función;
- la eliminación de archivos inesperados es una operación de cuarentena
  recuperable, nunca un borrado permanente basado en la extensión.

En una ejecución humana e interactiva, estas capacidades requieren una
confirmación `y/N` antes del análisis. El aviso concede solo la capacidad
solicitada; cada candidato debe seguir superando sus pruebas de analizador, hash,
ámbito y transacción. Responder que sí no debilita ninguna prueba.

Las ejecuciones en JSON y otras no interactivas requieren `--force`:

```sh
normfix --remove-unused --force
normfix --remove-unexpected --force
normfix --unsafe --force
```

`--unsafe` es un atajo cerrado para seis operaciones implementadas:

- eliminación de comentarios inválidos en una ubicación exacta;
- compactación de comparaciones simples con `NULL` solo cuando la forma dedicada
  en C está demostrada;
- eliminación de tokens demostradamente ausentes o formados solo por trivia en
  listas literales simples de fuentes del Makefile;
- eliminación de prototipos de headers locales del proyecto solo cuando una
  prueba completa y sin pérdidas del código no encuentra ni implementación ni
  uso o ambigüedad alguna;
- eliminación de `static` inalcanzable bajo una prueba de código cerrado;
- cuarentena de archivos inesperados.

Los avisos sobre la implementación de prototipos ya están activos en las
ejecuciones normales. El modo inseguro puede eliminar una declaración ausente y
sin uso tras la prueba completa; nunca elimina una definición existente formada
solo por trivia ni su prototipo, porque un cuerpo vacío puede ser intencionado.

No habilita ediciones arbitrarias. La eliminación de comentarios también puede
pedirse por separado con `--remove-invalid-comments`; los demás planes
destructivos siguen requiriendo autorización de capacidad.

Usa el modo de vista previa antes de una ejecución destructiva:

```sh
normfix --diff --remove-unused
normfix --check --remove-unexpected
```

Los modos de vista previa requieren la misma autorización interactiva, porque los
propios planificadores de mundo cerrado están protegidos por capacidad, pero no
escriben, no borran ni mueven archivos del proyecto.

## Copias de seguridad, transacciones y recuperación

Las copias de seguridad del código son, por defecto, externas al proyecto
analizado:

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
```

En Unix sin `XDG_DATA_HOME`, la ruta alternativa es:

```text
~/.local/share/normfix/backups/<run-id>/
```

Cada transacción con copia de seguridad incluye los bytes originales exactos y un
`journal.json`. Antes de que cambie el primer destino, el escritor:

- canonicaliza el límite del proyecto;
- rechaza destinos duplicados, externos, enlaces simbólicos y no regulares;
- confirma que cada archivo actual sigue coincidiendo con los bytes analizados;
- escribe las copias de seguridad externas;
- prepara y sincroniza cada sustitución.

Los destinos se confirman en orden de ruta. Un error a mitad de la confirmación
dispara un rollback de mejor esfuerzo a partir de los bytes originales
capturados; un rollback incompleto se informa junto con la ruta del journal de
recuperación.

`--no-backup` se aplica solo al formateo seguro corriente. Un borrado de código
planificado por la eliminación de comentarios inválidos, la reconciliación de
fuentes del Makefile, la eliminación de prototipos huérfanos o la eliminación de
`static` inalcanzable requiere almacenamiento externo de recuperación y falla en
cerrado si no está disponible.

La cuarentena siempre conserva una copia externa recuperable, incluso cuando se
indicó `--no-backup`:

```text
<backup-base>/quarantine/<run-id>/<original-relative-path>
```

El tipo de archivo, la longitud en bytes y el hash BLAKE3 se vuelven a comprobar
justo antes de mover. Los destinos de recuperación existentes nunca se
sobrescriben. Un fallo parcial de cuarentena intenta restaurar los archivos que ya
se movieron.
