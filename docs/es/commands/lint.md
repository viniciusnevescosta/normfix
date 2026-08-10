# `normfix lint`

Informa qué está mal en los bytes que hay ahora mismo en disco. No propone nada
y no escribe nada: ni formato, ni la cabecera oficial, ni cambios en el Makefile
o el README.

```sh
normfix lint
normfix lint src
```

Úsalo cuando quieras el diagnóstico sin el tratamiento: en CI, en una revisión, o
cuando piensas arreglar algo a mano y no quieres que la herramienta se mueva
debajo de ti.

## Qué informa

```console
$ normfix lint
warning[TOO_MANY_WS]: 2 occurrences in 1 file
  math_utils.c:1:1                     Extra whitespaces for indent level
  math_utils.c:2:1                     Extra whitespaces for indent level
 = help: Review this location and apply the named Norm rule manually; no
         semantics-preserving automatic edit was proven.
 = source: official Norminette 3.3.59 compatibility
 = explain: normfix explain TOO_MANY_WS

Summary: 1 files | 0 proposed | 0 written | 0 fixes | 14 remaining | 0 info
```

Fíjate en `0 proposed`: `lint` nunca planifica una edición. El mismo proyecto bajo
[`check`](/es/commands/check) informa diecisiete correcciones propuestas, porque
`check` sí tiene permiso para planificarlas.

Los diagnósticos se agrupan por regla y se conserva cada ubicación. Cada grupo
nombra su origen (la Norminette oficial, el compilador C, el analizador nativo o
una regla de proyecto), así sabes con qué autoridad estás discutiendo.

## En CI

```sh
normfix lint --format json > report.json
```

El JSON conserva los hallazgos individuales y lleva `schema_version`. El código
de salida `1` significa que quedan diagnósticos, `0` significa limpio y `2`
significa que la propia ejecución falló.
