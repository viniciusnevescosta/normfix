# `normfix explain`

Imprime la explicación incluida de una regla. No analiza ningún proyecto, no lee
ningún archivo y no usa la red.

```sh
normfix explain TOO_MANY_LINES
normfix explain INCLUDE_ORDER_REVIEW
normfix explain VLA_COMPAT_FALSE_POSITIVE
```

Cada diagnóstico de un informe normal termina con el comando exacto de su propia
regla, así que rara vez tienes que escribir el identificador de memoria:

```text
 = explain: normfix explain TOO_MANY_WS
```

## La forma de una respuesta

```console
$ normfix explain TOO_MANY_LINES
TOO_MANY_LINES: Function body exceeds 25 lines

Why
  The 42 Norm limits each function body to 25 physical lines so
  responsibilities stay small and reviewable.

Next
  Extract one coherent responsibility. Keep live inputs to four parameters or
  fewer and verify that the file still contains at most five functions.

Safety
  normfix reports this as a suggestion because choosing a function boundary
  changes program structure.
```

Siempre cuatro partes: qué es la regla, por qué existe, qué hacer a
continuación y por qué la herramienta actuó o no actuó por su cuenta.

## Familias de reglas

Los identificadores con el prefijo `CC_` vienen del compilador C y `CC_ANALYZER_`
de `-fanalyzer`; ambos se explican de forma genérica, porque el mensaje con
autoridad es el del propio compilador. Todo lo demás es un nombre de regla
oficial de Norminette o una regla nativa de `normfix`.

Un identificador desconocido sigue recibiendo una respuesta útil en lugar de un
error. El conjunto de artículos incluidos es una comodidad, no la fuente de la
verdad.
