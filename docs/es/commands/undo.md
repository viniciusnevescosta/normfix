# `normfix undo`

Restaura una ejecución anterior desde su copia de seguridad externa, y se niega a
sobrescribir cualquier cosa que haya cambiado desde entonces.

```sh
normfix undo --list
normfix undo
normfix undo --run run-1785950998077000000-53423
```

## Encontrar un punto de recuperación

```console
$ normfix undo --list
normfix undo: 1 recovery point(s)
  run-1785950998077000000-53423  1 file(s)
```

Cada ejecución guarda los bytes originales exactos y un `journal.json` que
prueba qué archivos escribió y qué escribió en ellos.

## Restaurar

Sin `--run`, `undo` selecciona el punto de recuperación íntegro más reciente y
pide confirmación. La restauración no interactiva exige `--force`:

```sh
normfix undo --force
```

## Cuándo se niega

`undo` falla cerrado. No restaura cuando:

- un archivo de destino ya no coincide con los bytes que escribió esa ejecución,
  porque alguien lo editó después, y restaurar descartaría ese trabajo en
  silencio;
- falta un archivo de copia de seguridad o su hash no coincide con el journal;
- cualquier ruta de la copia o del proyecto se resuelve a través de un enlace
  simbólico;
- el journal es ilegible o su esquema es desconocido.

Una negativa nombra el archivo y el motivo. Es deliberado: una herramienta de
recuperación que adivina es peor que una que se detiene.

## Qué no está cubierto

Las ejecuciones con `--no-backup` no dejan nada que restaurar, y ese es el
precio de omitir las copias. Las operaciones destructivas siempre conservan
almacenamiento de recuperación de todos modos, así que un archivo en cuarentena o
un comentario eliminado se puede recuperar incluso cuando se pasó `--no-backup`.
