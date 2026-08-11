# Playground en el navegador

El <a href="/es/" target="_self">playground</a> ejecuta el núcleo de `normfix` en WebAssembly. Monaco
ofrece números de línea, búsqueda, múltiples cursores, pares de corchetes y
resaltado para C, headers, Markdown y Makefiles. En móviles se usa un editor
ligero porque Monaco no admite oficialmente navegadores móviles.

## Cabecera de 42

Introduce un correo de estudiante válido en **Identidad 42**. La opción de
recordarlo está desmarcada de forma predeterminada. Si la activas, queda solo en
el almacenamiento local de ese navegador y puedes borrarlo con **Olvidar**. Se
entrega al WebAssembly de la pestaña para generar la cabecera oficial; nunca se
envía al servidor.

## Privacidad y límites

El código y la identidad permanecen en la pestaña. La única consulta externa es
la cantidad pública de estrellas del repositorio en GitHub; si falla se muestra
un valor incluido. No hay subida de código, cuenta, analytics ni backend.

El navegador no ejecuta la [Norminette oficial](https://github.com/42school/norminette),
el compilador, Git ni Make. Usa la CLI para la comprobación oficial, copias,
transacciones y undo.

## Uso sin conexión

El playground se instala la primera vez que lo abres. A partir de ahí, la
página, el formateador en WebAssembly y la interfaz no necesitan red alguna:
abre la misma dirección en un avión, con el wifi de la escuela en su peor
momento, o incluso con el sitio caído, y el formateo se ejecuta igual que
antes. Nunca se envió nada a ningún servidor, así que trabajar sin conexión
cambia cómo llegas a la herramienta, no lo que hace.

El navegador también puede instalarlo como aplicación desde la barra de
direcciones o el menú. Entonces se abre en su propia ventana, con el nombre en
el idioma que elegiste.

Conviene saber dos cosas:

- El editor de escritorio no forma parte de la instalación. Monaco es una
  descarga grande que aporta resaltado de sintaxis y búsqueda, así que solo se
  descarga cuando hay conexión, y se conserva en cuanto la haya. Abrir el
  playground sin conexión antes de eso te da el área de texto simple, que
  formatea de forma idéntica.
- Solo se guarda el playground. La documentación que estás leyendo es otro
  sitio y sigue necesitando red.

Una versión nueva nunca reemplaza la página mientras trabajas en ella. Se
descarga en segundo plano y la cabecera ofrece **Nueva versión lista** con un
botón **Recargar**. Hasta que lo pulses, conservas la versión con la que
empezaste.
