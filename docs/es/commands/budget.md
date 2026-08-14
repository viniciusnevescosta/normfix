# `normfix budget`

Una ejecución de solo lectura que añade una fila informativa por función
analizada, mostrando cuánto margen queda antes de los límites de la Norma: 25
líneas, 5 variables locales y 4 parámetros.

```sh
normfix budget
normfix budget src
```

```console
$ normfix budget
info[NORM_BUDGET]: 2 occurrences in 1 file
  math_utils.c:4:1   add(): lines 1/25 (24 left), variables 0/5 (5 left),
                     parameters 2/4 (2 left).
  math_utils.c:8:1   scale(): lines 3/25 (22 left), variables 1/5 (4 left),
                     parameters 2/4 (2 left).
 = help: Keep headroom for defense-day changes; limits already exceeded are
         also reported as warnings.
 = source: Norm v4.1 native rule

Summary: archivos: 1 | propuestos: 0 | escritos: 0 | correcciones: 0 | pendientes: 14 | informativos: 2
```

Las filas de presupuesto son informativas y nunca cambian por sí solas el código
de salida.

## Por qué importa el margen

Una función con 24 de 25 líneas cumple la Norma y está a una pregunta del día de
la defensa de dejar de cumplirla. `budget` existe para hacer eso visible antes de
que un evaluador te pida añadir una comprobación.

`normfix` informa el número; nunca extrae una función por ti. Elegir la frontera
de una función cambia la estructura del programa, y esa es una decisión que
necesita un nombre y un responsable. Consulta
[`normfix explain TOO_MANY_LINES`](/es/commands/explain).
