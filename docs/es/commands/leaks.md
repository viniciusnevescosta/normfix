# `normfix leaks`

Ejecuta un programa que ya compilaste bajo un verificador de fugas e informa de
lo que observó.

```sh
normfix leaks ./libft_test
normfix leaks ./push_swap -- 3 1 2
```

Todo lo demás que hace normfix lee tu código. Este comando lo ejecuta, así que
pregunta antes:

```console
$ normfix leaks ./push_swap
normfix va a ejecutar ./push_swap bajo el verificador de fugas. Esto ejecuta tu programa. ¿Continuar? [y/N] y
Se perdieron 1024 bytes del todo, y 96 más alcanzables solo a través de ellos.
Esto es lo que observó una ejecución con los argumentos que recibió. No es una prueba de que el programa nunca tenga fugas.
```

Los argumentos después de `--` van a tu programa, no al verificador, así que
puedes ejercitar el camino que importa:

```sh
normfix leaks ./push_swap -- 5 2 9 1
```

## Lo que no hace

`normfix` nunca compila tu programa. Compilar significa ejecutar las recetas de
tu Makefile, que es una segunda categoría —mucho mayor— de ejecutar código que
escribiste; y *«tú lo compilaste, yo lo ejecuté»* es una promesa mucho menor que
*«yo lo compilé y lo ejecuté»*. Compílalo como lo haces siempre y apunta este
comando al resultado.

## Un resultado limpio no es una prueba

El verificador ve el único camino que tomó tu programa con los argumentos que le
diste. Una ejecución que no pierde nada te dice que ese camino está limpio; no
dice nada de los caminos que no tomaste. Esa línea se imprime con cada resultado
por el mismo motivo por el que el resto de la herramienta informa de lo que no
puede demostrar en lugar de afirmarlo.

La memoria todavía alcanzable al salir no cuenta como perdida. 42 evalúa la
memoria que ya nadie puede alcanzar, y una arena que tu programa retiene hasta
salir no es eso.

Si el verificador produce una salida que normfix no puede leer como un resumen
de fugas, eso es un error, no un resultado limpio. Un verificador que fue matado
y uno que no encontró nada producen el mismo silencio, y la diferencia importa
demasiado como para adivinarla.

## Códigos de salida

| Código | Significado |
|---|---:|
| `0` | No se perdió nada en el camino de esta ejecución |
| `1` | Se perdió algo |
| `2` | El verificador no está disponible, fue rechazado, o no se pudo leer |

Fuera de una terminal interactiva —en CI, o con `--format json`— la confirmación
no se puede responder, así que `--force` es obligatorio:

```sh
normfix leaks --force ./libft_test
```

## Instalar un verificador

| Sistema | Cómo |
|---|---|
| Linux, FreeBSD | Valgrind, desde el gestor de paquetes |
| macOS | [`LouisBrunner/valgrind-macos`](https://github.com/LouisBrunner/valgrind-macos), ya que Valgrind oficial no compila en macOS. Su soporte para Apple Silicon es limitado |
| Windows | Ejecuta normfix dentro de [WSL](https://learn.microsoft.com/windows/wsl/install), donde el verificador de Linux funciona con normalidad |

normfix localiza `valgrind` en el `PATH` y lo verifica con su propio
`--version`, así que cualquier compilación funcional le sirve. Cuando no
encuentra ninguno, lo dice y nombra la ruta para el sistema en el que estás.
