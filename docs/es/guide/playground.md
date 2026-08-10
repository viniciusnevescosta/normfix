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
