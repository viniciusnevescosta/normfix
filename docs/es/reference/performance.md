# Rendimiento

Cada número aquí se midió, y cada uno es reproducible con un comando que puedes
ejecutar tú. Donde un número no impresiona, el texto lo dice y dice por qué.

::: tip No existe `normfix bench`
Los benchmarks son una herramienta de desarrollo, no parte de la superficie de
comandos. Se ejecutan con `cargo bench` desde un clon del repositorio.
:::

## Cuánto cuesta realmente una ejecución

En un proyecto real, `libft` con 44 fuentes y cabeceras:

| Ejecución | Tiempo |
|---|---:|
| Caché fría, todo activado | 1,82 s |
| Caché caliente, todo activado | 0,19 s |
| Caché caliente, sin el preflight del compilador | 0,17 s |

La caché vale unas **diez veces**, lo que importa porque el caso común es
ejecutar la herramienta repetidamente sobre un proyecto en el que estás
trabajando, no una vez sobre un proyecto que nunca has visto.

### Por qué una ejecución fría cuesta lo que cuesta

Una invocación de la Norminette oficial cuesta **107 ms** en esta máquina, y eso
es un intérprete de Python arrancando, no algo que este proyecto controle. Para
44 archivos eso son unos 4,7 s de trabajo en serie, que el paralelismo reduce a
1,82 s.

Así que el resumen honesto de una ejecución fría es: está dominada por un
subproceso por archivo. Optimizar el Rust de este repositorio mueve ese número en
porcentajes de un dígito. La caché existe precisamente porque la solución al
coste dominante es no hacer el trabajo dos veces.

## Cuánto cuesta el código propio de este proyecto

Estos números excluyen toda herramienta externa, así que miden lo que un cambio
en este repositorio puede realmente empeorar:

| Caso | Tiempo |
|---|---:|
| Archivo de 50 líneas ya correcto | 0,95 ms |
| Archivo desordenado de 40 líneas, todas las acciones de disposición | 1,89 ms |
| Archivo desordenado de 800 líneas | 38,2 ms |
| Construir un analizador | 0,34 µs |

Medido en un Apple M1, 8 núcleos, macOS 26.5, con la cadena de herramientas
fijada en `rust-toolchain.toml`.

```sh
cargo bench -p normfix-c-actions
```

La CI ejecuta los mismos benchmarks en cada push como un trabajo informativo. Un
runner compartido es demasiado ruidoso para usarlo como puerta, pero un benchmark
que nunca se ejecuta es un benchmark que deja de compilar en silencio.

## Qué encontraron los benchmarks

Los benchmarks se añadieron tras semanas de cronometrar a mano, y la primera
ejecución contradijo dos suposiciones en unos minutos.

Un archivo de 50 líneas ya correcto tardaba **4,5 ms** en decidir que no hacía
falta hacer nada. La causa sospechada era la construcción del analizador; medirla
mostró **340 nanosegundos**, así que no era eso. La causa real era que la fuente
se reanalizaba una vez por fase de formateo, cuando no puede cambiar mientras el
bucle de fases corre: aceptar un lote es lo único que la reescribe, y eso sale
del bucle de inmediato.

Analizando una vez por pasada, en cambio:

| Caso | Antes | Después |
|---|---:|---:|
| Archivo de 50 líneas ya correcto | 4,49 ms | 0,95 ms |
| Archivo desordenado de 800 líneas | 108 ms | 38,2 ms |

De extremo a extremo en un proyecto real eso es una mejora del 29 por ciento en
caliente y del 5 por ciento en frío, por el motivo de arriba: una ejecución fría
está esperando a Python.

La lección vale más que los números. Dos explicaciones plausibles estaban
equivocadas, y solo la medición lo dijo.

## Qué no está optimizado

- **El subproceso por archivo.** Norminette acepta varios archivos en una
  invocación, lo que sustituiría 44 arranques de proceso por uno. Hacerlo
  significa que el pipeline ya no puede analizar un búfer sombra cada vez, que es
  como está estructurada hoy la prueba antes/después. Es la mayor ganancia
  restante y la de mayor coste arquitectónico.
- **Archivos individuales muy grandes.** Por encima de unos miles de líneas el
  coste lo domina algo distinto del índice de líneas, y eso no se ha perseguido.
  Las fuentes reales de 42 están muy por debajo.
- **Asignación de tokens.** Cada análisis copia el texto de cada token a una
  cadena propia. Tomarlo prestado de la fuente es un cambio contenido que aún no
  se ha medido.
