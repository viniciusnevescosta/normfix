# Informes, códigos de salida y rendimiento

## Leer un diagnóstico

Cada diagnóstico se muestra contra el código al que se refiere, para que vayas
directo a la línea en vez de buscar una coordenada:

```text
error[CC_IMPLICIT_FUNCTION_DECLARATION]: 2 occurrences in 2 files
  --> srcs/sort/sort.c:30:3
   |
30 |         sort_medium(ctx);
   |         ^^^^^^^^^^^ call to undeclared function 'sort_medium'
   |
  ::: srcs/sort/sort_adaptive.c:21:3
   |
21 |         sort_medium(ctx);
   |         ^^^^^^^^^^^ call to undeclared function 'sort_medium'
   |
   = help: Fix this strict -Wall/-Wextra/-Werror compiler diagnostic, then rerun normfix.
   = source: C compiler
   = explain: normfix explain CC_IMPLICIT_FUNCTION_DECLARATION
```

Los acentos circunflejos abarcan los bytes exactos a los que se refiere la regla,
no solo su primer carácter. Las ocurrencias de una misma regla se agrupan bajo un
único encabezado, cada una etiquetada con su propio mensaje, y la ayuda, las
notas, el origen y la pista de `explain` compartidas se indican una vez para el
grupo en lugar de repetirse bajo cada ocurrencia.

La vista por defecto muestra las tres primeras ocurrencias de una regla y dice
cuántas retuvo, porque un proyecto puede llevar miles de un mismo diagnóstico.
`--verbose` las muestra todas, cada una en su propia sección con su propio
fragmento.

Algunos detalles que conviene conocer:

- Las tabulaciones se expanden, así que el acento cae bajo el carácter correcto.
- Los caracteres de control de tu fuente se muestran como figuras visibles y
  nunca llegan al terminal como controles.
- La columna de la línea `-->` se cuenta en caracteres, la convención que usa un
  compilador C. La Norminette oficial cuenta columnas de visualización, así que su
  salida puede nombrar una columna mayor para el mismo carácter en una línea
  indentada con tabulaciones. El acento es la respuesta con autoridad a *dónde*.
- Un diagnóstico de compilador que pertenece a un archivo sin una posición dentro
  de él, normalmente porque la ubicación real está en una cabecera incluida,
  nombra el archivo y la cabecera en lugar de dibujar un acento sobre código no
  relacionado.

La representación usa [`annotate-snippets`], la biblioteca con la que el propio
`rustc` representa sus diagnósticos.

[`annotate-snippets`]: https://crates.io/crates/annotate-snippets

## El resto de la salida

- una tabla de estado por archivo: `CLEAN`, `INFO`, `FIXED`, `WOULD FIX`,
  `REVIEW` o `FAILED`;
- IDs de regla estables, ayuda compartida, notas, origen del diagnóstico y una
  pista `normfix explain RULE`;
- detalles opcionales de las correcciones aceptadas con `--verbose`;
- diffs unificados con `--diff`;
- recuentos agregados y el tiempo transcurrido.

El color se activa solo para un stdout interactivo. `--no-color`, `NO_COLOR`, la
salida JSON y la salida redirigida quedan sin color. Los fragmentos se
representan contra un ancho fijo, así que un informe se lee igual en dos
máquinas.

Antes del descubrimiento, el modo humano escribe un bloque compacto
`normfix · starting` en `stderr` con la acción, el alcance efectivo, el modo de
escritura/comprobación, el origen de la identidad, la política del verificador,
los procesos, la caché, la comprobación del compilador, las copias de seguridad y
las capacidades destructivas solicitadas. Eso hace obvia una ejecución accidental
en la raíz o en el directorio personal antes de que empiece el trabajo. Esos
alcances protegidos se niegan sin `--force`.

El modo JSON escribe en su lugar un objeto de evento `execution_start` en
`stderr`. El informe final versionado sigue siendo el único documento JSON en
`stdout`, así que la automatización existente puede seguir analizándolo como un
único valor.

`--format json` emite un esquema determinista y formateado con
`schema_version: 2`. Incluye metadatos de identidad, resultados de descubrimiento
y cuarentena, campos de cambio/escritura/fallo por archivo, correcciones,
diagnósticos antes/después, recuentos de resumen, el `evaluation` opcional del
preflight y `duration_seconds`. Los búferes de fuente y los diffs unificados se
omiten intencionadamente.

`normfix preflight` añade una estimación determinista y explícitamente no
concluyente: `score`, `grade`, `verdict` y `hard_failures` con ubicación exacta.
El veredicto es `hard_fail` cuando el alcance evaluado contiene un archivo
inesperado, un hallazgo corroborado por la Norminette oficial instalada o un
diagnóstico de Makefile. Las evidencias de Norminette y de Makefile vienen de la
instantánea original en disco, más cualquier fallo adicional expuesto en la
sombra final. Por tanto, un problema corregible automáticamente sigue siendo un
suspenso del preflight hasta que los bytes propuestos se escriban realmente y se
comprueben otra vez.
La nota numérica es una heurística acotada de priorización, no una nota de 42; no
puede cubrir el comportamiento en ejecución, las pruebas específicas del
proyecto, las fugas, el juicio de los pares ni las preguntas de defensa.

Este es el objeto `evaluation` de una ejecución real, en un proyecto cuyo único
problema restante es un Makefile que lista una fuente que se borró:

```json
{
  "schema_version": 2,
  "evaluation": {
    "conclusive": false,
    "score": 59,
    "grade": "fail",
    "verdict": "hard_fail",
    "hard_failures": [
      {
        "rule_id": "MAKEFILE_SOURCE_NOT_FOUND",
        "path": "Makefile",
        "line": 14,
        "column": 20,
        "message": "The literal Makefile source `ghost.c` does not exist below the project root."
      }
    ],
    "notes": [
      "Incomplete means discovery or file analysis failed, or no processable file was covered; no grade can be inferred from that run.",
      "Hard fail: an unexpected project file, a finding corroborated by the installed official Norminette, or a Makefile finding was present.",
      "The score deducts bounded category weights for those findings, other warnings, operational failures, and pending edits; it is not a 42 grade.",
      "Runtime behavior, subject-specific tests, peer judgment, leaks, and defense questions remain outside this estimate."
    ]
  }
}
```

`conclusive` es `false` en todo informe que esta herramienta puede producir;
existe para que un consumidor nunca tenga que inferir ese límite a partir de la
prosa. `notes` forma parte del documento, no de la decoración del terminal, así
que un agente que retransmite el resultado se lleva las salvedades con él. Lee
`verdict` para la decisión y `score` solo para ordenar el trabajo: el veredicto
sigue siendo `hard_fail` mientras quede cualquier suspenso, por alta que llegue a
ser la nota.

### Códigos de salida

| Código | Significado |
|---:|---|
| `0` | El modo de corrección terminó sin diagnóstico bloqueante, o la entrada ya estaba limpia |
| `1` | Quedan diagnósticos manuales, el modo de vista previa encontró cambios propuestos/candidatos a cuarentena, o el preflight cumplió una regla de suspenso |
| `2` | Fallo de descubrimiento, configuración, herramienta, E/S, transacción o cuarentena |
| `130` | Se canceló una revisión interactiva por archivo |

Los avisos informativos no hacen fallar una ejecución.

## Caché y rendimiento

El análisis de archivos se ejecuta en paralelo con Rayon. `--threads N` crea un
pool local con un número exacto de procesos; sin él, Rayon usa el hardware
disponible. Los resultados y las confirmaciones se ordenan por ruta, así que el
orden de finalización de los procesos no cambia el orden del informe ni el orden
de escritura.

Los informes de la Norminette oficial usan tanto una caché de ejecución en
memoria como una base redb persistente fuera del proyecto. En Unix:

```text
$XDG_CACHE_HOME/normfix/<project-id>/cache-v1.redb
```

o:

```text
~/.cache/normfix/<project-id>/cache-v1.redb
```

Las claves incluyen el esquema, el espacio de nombres del análisis, la ruta
relativa al proyecto cuando la entrada está dentro de la raíz de la ejecución
(con repliegue a la ruta absoluta para una entrada externa explícita), los bytes
de la fuente, la configuración de la Norma y la huella verificada del ejecutable.
Los fallos de bloqueo, E/S, decodificación o corrupción de la caché fallan
abiertos como ausencias; nunca cambian los diagnósticos ni el estado de salida.
Una base corrupta se conserva bajo un nombre `.corrupt-N` antes de recrearla.

Usa `--no-cache` para una ejecución totalmente sin caché.
