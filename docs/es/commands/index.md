# Comandos

La interfaz sin subcomando es la forma más corta de formatear un proyecto, y es
la que usan la mayoría de las ejecuciones:

```sh
cd ruta/a/un/proyecto-42
normfix
```

Los subcomandos hacen explícita la intención, lo que importa en scripts, en CI y
durante una revisión.

| Comando | Escribe | Úsalo cuando |
|---|---|---|
| [`format`](/es/commands/format) | sí | Quieres aplicar las ediciones aceptadas |
| [`lint`](/es/commands/lint) | no | Quieres diagnósticos sobre los bytes en disco, sin proponer nada |
| [`check`](/es/commands/check) | no | Quieres ver lo que *haría* una ejecución de corrección |
| [`budget`](/es/commands/budget) | no | Quieres el margen de líneas/variables/parámetros por función |
| [`preflight`](/es/commands/preflight) | no | Vas a defender y quieres las comprobaciones de solo lectura |
| [`leaks`](/es/commands/leaks) | no | Quieres comprobar fugas en un programa que compilaste |
| [`explain`](/es/commands/explain) | no | Quieres una regla explicada sin analizar nada |
| [`undo`](/es/commands/undo) | sí | Quieres restaurar una ejecución anterior |
| [`upgrade`](/es/commands/upgrade) | sí | Quieres la versión más reciente, verificada |
| [`uninstall`](/es/commands/uninstall) | sí | Quieres eliminar normfix de esta máquina |

## Cada ejemplo de estas páginas es real

La salida mostrada fue producida por `normfix 1.5.0` sobre este archivo:

```c
# include "libft.h"
# include <stdlib.h>

int add(int a,int b){
return a+b;
}

int	scale(int value, int factor)
{
	int result;
	result = value * factor;
	return result;
}
```

Está desordenado a propósito de maneras corrientes: includes sin ordenar, una
definición de función colapsada, espacios que faltan, una declaración no separada
de las instrucciones y valores de `return` sin paréntesis.

## Códigos de salida

Todos los comandos los comparten:

| Código | Significado |
|---:|---|
| `0` | Nada bloqueante: la ejecución quedó limpia, o el modo de corrección terminó |
| `1` | Quedan diagnósticos manuales, o una vista previa encontró cambios propuestos |
| `2` | Fallo de descubrimiento, configuración, herramienta, E/S, transacción o cuarentena |
| `130` | Se canceló una revisión interactiva |

Los avisos informativos nunca cambian el código de salida. Eso hace que los
códigos sean usables directamente en CI:

```sh
normfix --check || echo "este proyecto aún no está limpio según la Norma"
```

## Flags que acepta cada comando

`--format json` y `--no-color` cambian la salida; `--threads`, `--timeout`,
`--no-cache` y `--norminette PATH` cambian cómo se ejecuta. La tabla completa
está en [línea de comandos](/es/guide/command-line).
