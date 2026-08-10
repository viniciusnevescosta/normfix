# Makefiles, documentos README y archivos de proyecto

Los Makefiles usan un formateador conservador dedicado, porque Norminette no
analiza la sintaxis de GNU Make. El formateador puede:

- eliminar un BOM UTF-8 y normalizar los finales de línea;
- insertar o actualizar la cabecera oficial 42 con estilo `#`;
- garantizar un único salto de línea final;
- empaquetar de forma codiciosa asignaciones explícitas simples de `.c` hasta 80
  columnas de visualización, conservando el orden y la semántica de la
  asignación.

Preserva deliberadamente las recetas, los proyectos con `.RECIPEPREFIX`, los
bloques `define`, las asignaciones de shell, la expansión de variables/funciones,
los patrones, los comentarios, las comillas, los separadores de comandos y otras
construcciones ambiguas de Make.

El analizador informa:

- una asignación `NAME` ausente;
- reglas `all`, `clean`, `fclean`, `re` o `$(NAME)` ausentes;
- que `all` no sea el objetivo concreto por defecto;
- descubrimiento de fuentes/objetos por comodín;
- líneas largas que no pueden empaquetarse con seguridad;
- espacios en blanco tras una barra invertida de continuación.

Para una asignación simple al estilo `SRC`/`SRCS` cuyo valor completo está hecho
de rutas `.c` relativas y literales, también comprueba si cada token existe y si
el archivo regular referenciado contiene algún token C más allá de espacios o
comentarios. Las rutas se resuelven desde el directorio que contiene ese
Makefile, también para Makefiles anidados, y cada componente debe permanecer
dentro de la raíz canónica del proyecto y evitar enlaces simbólicos. Una ruta
ausente o con solo trivialidades se informa por defecto. `--unsafe` puede
eliminar solo el token exacto probado y reempaquetar la lista restante sin
reordenarla. Las expansiones, los patrones, las comillas, los comentarios, las
recetas, los bloques `define`, `.RECIPEPREFIX`, las rutas que se escapan y los
resultados inciertos del sistema de archivos quedan sin cambios.

Todo flujo respaldado por el sistema de archivos compara prototipos no estáticos
de las cabeceras del proyecto con una instantánea completa y sin pérdidas de
fuentes C/cabecera. Las implementaciones ausentes y los cuerpos correspondientes
con solo trivialidades se informan en el nombre del prototipo. La eliminación
insegura se limita a implementaciones ausentes y exige el alcance completo del
proyecto, autorización acotada, ningún otro uso del identificador ni ambigüedad,
validación de reanálisis en la sombra y una comprobación de hash, en el momento
de la transacción, de todas las entradas de la prueba. Las definiciones
existentes con solo trivialidades nunca se eliminan: un no-op intencionado puede
ser válido.

La herramienta no añade automáticamente cada archivo `.c` encontrado en disco a
una variable de fuentes. Pertenecer a un objetivo es una decisión de diseño de la
compilación.

## Preflight del compilador y avisos de fugas

Para cada archivo `.c` seleccionado, el pipeline normal ejecuta una pasada de
solo lectura del compilador equivalente a:

```text
cc -fsyntax-only -Wall -Wextra -Werror
```

Añade rutas `-I` estables para los directorios que contienen cabeceras del
proyecto descubiertas, pero no adivina defines específicos de la asignatura,
modos de lenguaje, cabeceras generadas, flags de destino ni entradas del
enlazador. Usa `--cc PATH` para seleccionar un compilador exacto o
`--no-compiler-preflight` para omitir la pasada. Los hallazgos del compilador son
solo diagnósticos: nunca autorizan ni rechazan ediciones del formateador. Un
compilador no disponible o un contexto de compilación visiblemente incompleto
produce un aviso claro que falla abierto.

`--analyzer` además pide al compilador elegido la salida de `-fanalyzer` de GCC
en flujos corrientes. Preflight realiza esa pasada acotada del analizador
automáticamente. Puede sacar a la luz posibles fugas de asignación y rutas de
acceso inválido, pero es más lenta e intencionadamente informativa. No es una
prueba de fugas: la exploración de rutas es incompleta, se inspecciona una unidad
de traducción cada vez, y la propiedad oculta tras funciones externas o guardada
en structs puede escapar al análisis. Un compilador sin ninguna de las interfaces
de analizador soportadas se informa y se omite.

### Modo previo a la defensa

```sh
normfix preflight
```

`preflight` es la vista previa de solo lectura de formato/lint pensada para el
momento inmediatamente anterior a la evaluación. Agrega resultados oficiales de
Norminette, límites nativos de la Norma y sugerencias de extracción, cabeceras
oficiales y guardas de cabecera, política de funciones permitidas, estructura del
Makefile y referencias literales de fuente, fuentes de Makefile con solo
trivialidades, prototipos de cabecera sin definición en el proyecto, cuerpos de
implementación con solo trivialidades, archivos inesperados, hallazgos de README,
la pasada estricta del compilador y el analizador del compilador. Las pasadas del
compilador no pueden desactivarse en este flujo.

La `Pre-defense estimate` final es intencionadamente no concluyente. Los archivos
inesperados, los hallazgos de la Norminette instalada y los diagnósticos de
Makefile producen un suspenso con ubicaciones exactas de fuente. La nota de 0 a
100 y la banda de letra solo priorizan el trabajo restante; no son una nota
oficial.

La evidencia de suspenso se basa en los diagnósticos originales en disco de
Norminette y del Makefile, más cualquier hallazgo recién expuesto que permanezca
en la sombra. Una edición segura propuesta por el modo check no hace que los
bytes entregados aprueben retroactivamente.

Cuando falta `normfix.toml`, preflight emite `FUNCTION_POLICY_NOT_CONFIGURED` en
lugar de fingir que se ejecutó la comprobación de funciones autorizadas. También
emite `PREFLIGHT_MANUAL_STEPS`: el comando deliberadamente no ejecuta recetas de
Make, no enlaza ni inspecciona el binario final, no ejecuta el programa/pruebas y
no invoca herramientas de fugas en tiempo de ejecución. Ejecuta esos pasos
específicos del proyecto por separado. Informa de si `clang-tidy` está en el
`PATH` y da orientación separada de sanitizers para compilación de depuración,
pero no ejecuta ninguno de los dos. Cuando no se selecciona ni se encuentra un
Makefile regular en la raíz del proyecto, `MAKEFILE_NOT_FOUND` informa de una
comprobación incompleta sin suspender: solo una política específica de la
asignatura puede probar que todo proyecto necesita uno.

## Soporte de README y Markdown

Los archivos README se analizan con Comrak y se reimprimen canónicamente por
defecto:

```sh
normfix README.md
```

La reimpresión canónica es idempotente, pero puede crear un diff amplio en la
primera ejecución. Usa `--check` o `--diff` para previsualizarla.
`--no-format-markdown` mantiene los archivos README de solo lectura, informando
aún de saltos de nivel de encabezado, espacios al final de línea y la falta de un
salto de línea final.

Cuando preflight descubre un README, `README_42_CRITERIA_REVIEW` te recuerda
compararlo con la ficha de asignatura y evaluación actual. La ausencia de README
no emite diagnóstico y nunca suspende el preflight.

## Archivos inesperados del proyecto

El descubrimiento recursivo informa de archivos regulares distintos de `.c`,
`.h`, `Makefile`, variantes de README, `.normfixignore` y su alias heredado
`.norminetteignore`. Fuera de preflight, esa advertencia por sí sola no cambia el
estado de salida. Preflight la usa como regla explícita de suspenso, porque se
espera que el alcance de entrega evaluado contenga solo archivos de proyecto
soportados. Eso nunca implica que un archivo sea prescindible.

Usa `--remove-unexpected` solo cuando pretendas mover todos los archivos
regulares inesperados elegibles a la cuarentena externa. Los enlaces simbólicos,
los directorios, las rutas fuera del proyecto, las instantáneas cambiadas y las
rutas de recuperación solapadas se rechazan.
