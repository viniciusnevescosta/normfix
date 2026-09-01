# Playground en el navegador

El <a href="/es/" target="_self">playground</a> es normfix funcionando dentro de
la pestaña de tu navegador. Pega o arrastra un proyecto, pulsa Ejecutar, y
obtienes el código formateado, los hallazgos que pudo demostrar y el diff — sin
que nada salga de tu máquina.

Es el mismo código que ejecuta la línea de comandos, así que lo que arregla aquí
lo arregla allí. Lo que no puede hacer aquí es comprobar tu trabajo contra la
Norminette oficial o un compilador, porque ninguno de los dos existe en un
navegador. Cada resultado lo dice.

En navegadores de escritorio el editor es Monaco, con números de línea,
búsqueda, múltiples cursores, pares de corchetes y resaltado para todos los
tipos de archivo admitidos. Los móviles y los dispositivos de puntero grueso
usan un área de texto ligera, porque Monaco no admite oficialmente navegadores
móviles.

## Añadir tu proyecto

Arrastra archivos a la página, o arrastra la carpeta del proyecto entera. Una
carpeta soltada conserva su estructura, así que `libft/src/ft_strlen.c` llega
con esa ruta y no aplanado en un montón de nombres.

Un directorio de proyecto real contiene más que código. Los archivos objeto, el
binario compilado, `.git` y la configuración del editor se omiten en lugar de
convertirse en un error, y siempre se muestra cuántos se omitieron: la
importación nunca descarta nada en silencio, ni rechaza todo el drop porque un
archivo no sea algo que normfix formatea. El botón **+** crea un archivo. Elige el tipo — `.c`, `.h`, `Makefile` o `.md` —
en lugar de escribir la extensión y descubrir después que no era una de las
cuatro. Una ruta como `src/utils.c` crea la carpeta con él, y las carpetas se
anidan cuanto necesites. **Descargar todo (.zip)** mantiene esa estructura.

El botón de carpeta crea un directorio por sí solo; no te obliga a crear un
archivo a continuación. Las carpetas vacías siguen visibles, sobreviven a una
recarga y se conservan en el ZIP. Mientras haya alguna carpeta vacía, el panel
del proyecto muestra su nombre explícitamente para que no confundas una carpeta
de ejercicio vacía con un árbol de código importado.

**Elegir archivos** hace lo mismo con
un selector.

## Hallazgos subrayados donde están

Los errores y avisos aparecen subrayados en el editor igual que los subraya tu
propio editor, para que dejes de cruzar un número de línea de una lista con una
línea de tu archivo. Al pasar el ratón aparecen la regla y la explicación.

Un hallazgo sin posición — una cabecera 42 inválida pertenece al archivo, no a
una línea — queda fuera de los subrayados en lugar de dibujarse en cualquier
sitio. Lo sigues encontrando en el panel de diagnósticos.

## Apariencia

**Sistema**, **Claro** u **Oscuro**, junto al selector de idioma. Sigue a tu
sistema operativo salvo que indiques otra cosa, y la elección se recuerda en
este dispositivo hasta que la cambies: como el idioma, cambia el aspecto de la
página y nada más, sin ejecuciones, peticiones ni recargas.

## Cabecera oficial de 42

Escribe un correo de estudiante válido en el panel **Identidad 42**. La opción
**Recordar en este dispositivo** empieza desactivada. Cuando la activas
explícitamente, la dirección se guarda solo en el almacenamiento local del mismo
origen de este navegador y puede borrarse en cualquier momento con **Olvidar**.
En caso contrario, vale solo para la pestaña actual.

La dirección se pasa a WebAssembly dentro de la pestaña para generar la cabecera
oficial de 42. Nunca se envía a un servidor de formateo. Sin una identidad
válida, el código se queda sin cabecera generada y el resultado incluye un
diagnóstico que lo indica.

## Aprovechar el resultado

Una ejecución siempre abarca el proyecto entero, porque una cabecera y el
archivo que la incluye solo se evalúan bien juntos. La elección es qué hacer con
la respuesta: aplicar de una vez todo lo que quedó demostrado, o solo lo que
tienes delante. En ambos casos, una corrección deja de ser aplicable si el
archivo se editó después de la ejecución, ya que se demostró contra el código
que normfix leyó, no contra lo que hay ahora en el editor.

- **Corregir todos los archivos** escribe de una vez, en el proyecto, cada
  resultado demostrado.
- **Corregir este archivo** hace lo mismo con el archivo que estás viendo.
- **Copiar archivo** copia el resultado estable seleccionado. Si se deniega el
  acceso al portapapeles, el navegador selecciona el texto para que lo copies
  con el teclado.
- **Descargar archivo** guarda el resultado seleccionado.
- **Descargar todo (.zip)** guarda todos los resultados estables en un único
  archivo que cualquier sistema de escritorio abre sin instalar nada.
- **Usar como nueva entrada** devuelve un resultado al editor para otra
  ejecución.

## Privacidad y comportamiento de red

El código y la identidad permanecen en la pestaña. No hay subida de código,
cuenta, dependencia de analítica ni backend de formateo. Abrir el playground no
hace peticiones a terceros; GitHub y las herramientas oficiales son enlaces
normales que solo se visitan cuando tú eliges.

## Uso sin conexión

El playground se instala la primera vez que lo abres. A partir de ahí, la
página, el formateador en WebAssembly y la interfaz no necesitan red alguna:
abre la misma dirección en un avión, con el wifi de la escuela en su peor
momento, o incluso con el sitio caído, y el formateo se ejecuta igual que antes.
Nunca se envió nada a ningún servidor, así que trabajar sin conexión cambia cómo
llegas a la herramienta, no lo que hace.

El navegador también puede instalarlo como aplicación desde la barra de
direcciones o el menú. Entonces se abre en su propia ventana, con el nombre en
el idioma que elegiste.

Conviene saber dos cosas:

- El editor de escritorio no forma parte de la instalación. Monaco es una
  descarga grande que aporta resaltado de sintaxis y búsqueda, así que solo se
  descarga cuando hay conexión, y se conserva en cuanto la haya. Abrir el
  playground sin conexión antes de eso te da el área de texto simple, que
  formatea de forma idéntica.
- Solo se guarda el playground. La documentación que estás leyendo es otro sitio
  y sigue necesitando red.

Una versión nueva nunca reemplaza la página mientras trabajas en ella. Se
descarga en segundo plano y la cabecera ofrece **Nueva versión lista** con un
botón **Recargar**. Hasta que lo pulses, conservas la versión con la que
empezaste.

## Límites entre la CLI y el playground

| Capacidad | CLI | Playground |
|---|---:|---:|
| Formateo seguro de C y headers | sí | sí |
| Formateo seguro de Makefile y Markdown | sí | sí |
| Cabecera oficial de 42 a partir de una identidad indicada | sí | sí |
| Diagnósticos estructurales y presupuestos de función | sí | sí |
| Diffs unificados | sí | sí |
| Verificación con la Norminette oficial | sí | no |
| Preflight estricto del compilador y analizador | sí | no |
| Descubrimiento automático de identidad | sí | no |
| Ámbitos de Git | sí | no |
| Copias de seguridad, transacciones y undo | sí | no |

El sandbox del navegador no ejecuta el binario de la
[Norminette oficial](https://github.com/42school/norminette), un compilador, Git
ni Make. Usa la [línea de comandos](/es/guide/command-line) para la verificación oficial y para el flujo completo de
preparación de la defensa.

## Límites y portabilidad

El playground acepta como máximo 128 archivos, 256 carpetas explícitas, 1 MiB
por archivo y 4 MiB en total. Las rutas deben ser relativas, portables y normalizadas en NFC, con un
máximo de 240 bytes UTF-8. Rechaza duplicados que colisionan en sistemas que no
distinguen mayúsculas, nombres reservados de plataforma, UTF-8 inválido y rutas
inseguras para un archivo comprimido antes siquiera de ejecutar el formateador.
Un BOM UTF-8 inicial se consume de forma consistente. Cualquier resultado del
formateador que no alcance un punto fijo se descarta, en lugar de exponerse como
una edición parcial aparentemente utilizable.

## Ejecutar localmente

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
npm ci
npm run dev
```

La compilación también exige una instalación de Clang con el destino WebAssembly
funcionando. En macOS, la construcción explora las rutas del LLVM de Homebrew y
explica cómo instalar LLVM cuando el compilador del sistema no puede generar
código para `wasm32`.
