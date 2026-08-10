# Seguridad, recuperación y operaciones destructivas

`normfix` solo aplica automáticamente una edición cuando completa su prueba.
Un diagnóstico o una sugerencia no equivalen a una corrección demostrada. Usa
la vista previa antes de una operación destructiva:

```sh
normfix --diff --remove-unused
normfix --check --remove-unexpected
```

## Autorizaciones destructivas

Los comentarios inválidos solo se notifican de forma predeterminada.
`--remove-invalid-comments` elimina únicamente el comentario en la posición
exacta indicada por la Norminette oficial y conserva la cabecera 42. Las
opciones `--remove-unused`, `--remove-unexpected` y `--unsafe` exigen una
confirmación `y/N` interactiva; en JSON u otra ejecución no interactiva exigen
`--force`.

La confirmación solo autoriza la capacidad solicitada. Cada candidato todavía
debe superar pruebas de parser, hash, alcance y transacción. Ambigüedad,
archivos ilegibles, macros complejas o un conjunto incompleto de fuentes hacen
que la operación falle de forma conservadora.

## Copias y undo

Los bytes originales y `journal.json` se guardan fuera del proyecto:

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
~/.local/share/normfix/backups/<run-id>/
```

Antes de escribir, el programa rechaza destinos externos, enlaces simbólicos,
archivos irregulares, duplicados o cambiados después del análisis. Un fallo de
escritura activa rollback. Las eliminaciones requieren almacenamiento de
recuperación incluso con `--no-backup`; los archivos inesperados se mueven a
cuarentena, nunca se borran permanentemente.

Usa `normfix undo` para restaurar la última transacción. Conserva la ruta del
journal mostrada si rollback no puede completarse automáticamente.

`normfix.toml` y las listas de funciones permitidas complementan, pero nunca
sustituyen, el subject ni la evaluación oficial de 42.
