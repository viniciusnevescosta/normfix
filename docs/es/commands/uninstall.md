# `normfix uninstall`

Elimina este binario y — solo cuando se pide por su nombre — los datos que creó.

```sh
normfix uninstall --dry-run   # muestra el plan, no elimina nada
normfix uninstall             # elimina el binario, conserva tus datos
normfix uninstall --purge     # elimina también configuración, caché y copias
```

## Muestra el plan primero

No se elimina nada antes de que hayas visto exactamente qué se eliminaría:

```console
$ normfix uninstall --dry-run
normfix uninstall
  remove  /usr/local/bin/normfix
  keep    /home/student/.config/normfix (configuration)
  keep    /home/student/.cache/normfix (cache)
  keep    /home/student/.local/share/normfix (backups and quarantine)
Pass --purge to remove the kept directories as well.
```

Por defecto conserva tus datos. Es deliberado: el directorio de copias guarda la
única copia de todo lo que una ejecución anterior sustituyó o movió, y
desinstalar un formateador no es una declaración de que quieres perder el trabajo
que te guardó.

## `--purge`

```console
$ normfix uninstall --purge --dry-run
normfix uninstall
  remove  /usr/local/bin/normfix
  remove  /home/student/.config/normfix (configuration)
  remove  /home/student/.cache/normfix (cache)
  remove  /home/student/.local/share/normfix (backups and quarantine)
This also deletes backups and quarantined files, which is the only copy of anything a previous run replaced or moved.
```

La configuración y la caché son reproducibles: la primera es tu identidad 42, que
puedes volver a proporcionar, y la segunda es una caché. Las copias de seguridad
y los archivos en cuarentena no lo son. Ejecuta
[`normfix undo --list`](/es/commands/undo) antes si no estás seguro de que algo
siga siendo recuperable.

## Confirmación

Una ejecución interactiva pregunta antes de eliminar nada:

```console
¿Eliminar los archivos listados arriba? [y/N]
```

`y` es la respuesta aceptada en todos los idiomas. Una ejecución no interactiva
—un script, la CI o `--format json`— se niega en vez de suponer, y exige
`--force`:

```sh
normfix uninstall --force
normfix uninstall --purge --force
```

## Cuándo se niega

| Situación | Qué dice |
|---|---|
| Instalado con Homebrew | Te remite a `brew uninstall viniciusnevescosta/normfix/normfix` |
| Sin permiso de escritura | Nombra la ruta y dice que revises la propiedad; nunca pide `sudo` |
| Un directorio de datos no se puede eliminar | Nombra ese directorio y se detiene, con el binario aún instalado |

Homebrew se rechaza en lugar de sortearse: eliminar un archivo que la fórmula
sigue describiendo deja a `brew` como lo único capaz de devolver la máquina a un
estado consistente.

Los directorios de datos se eliminan antes que el binario. Si uno falla, la
herramienta que informó del fallo sigue en disco para reintentarlo.

## Eliminar un binario en ejecución

En Unix, desenlazar el ejecutable en ejecución es seguro: el núcleo mantiene el
archivo vivo hasta que el proceso termina, así que el comando concluye e imprime
su resultado con normalidad. Lo que se elimina es el nombre en el sistema de
archivos.
