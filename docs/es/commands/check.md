# `normfix check`

Ejecuta el pipeline completo de corrección en memoria e informa el resultado sin
tocar un solo archivo.

```sh
normfix check
normfix check main.c
```

`normfix --check` es lo mismo.

```console
$ normfix check
Files
STATUS      FIXES  REMAINING  INFO  FILE
REVIEW        1          1     0  Makefile
WOULD FIX     2          0     0  add.c
REVIEW        3          1     0  demo.h
WOULD FIX     6          0     0  main.c

Summary: 4 files | 4 proposed | 0 written | 12 fixes | 2 remaining | 0 info | 0 failed | 0 unexpected | 0 quarantined
Completed in 578 ms.
```

`WOULD FIX` y `4 proposed` son la diferencia con [`lint`](/es/commands/lint):
`check` planifica las ediciones y dice cuántas superaron las pruebas, solo que no
las confirma.

Los dos estados responden preguntas distintas. `WOULD FIX` significa que todo lo
encontrado en ese archivo tiene una corrección probada esperando — `add.c` y
`main.c` no necesitan nada de ti. `REVIEW` significa que queda algo después de
aplicar toda corrección segura, y la columna `REMAINING` lo cuenta: aquí el
Makefile lista una fuente que no existe y `demo.h` declara una función que nadie
implementa. Ninguna de las dos tiene una respuesta automática segura, así que
ambas se informan en vez de adivinarse.

Leyendo el resumen de izquierda a derecha: se analizaron 4 archivos, 4 tienen
cambios propuestos, ninguno se escribió porque esto es `check`, 12 correcciones
individuales superaron sus pruebas y 2 hallazgos aún necesitan a una persona.

## Legible por máquina

```console
$ normfix check --format json
{
  "schema_version": 2,
  "tool_version": "1.3.2",
  "mode": "check",
  "summary": {
    "files": 4,
    "changed": 4,
    "written": 0,
    "fixes": 12,
    "remaining": 2,
    "advisories": 0,
    "failed": 0,
    "unexpected_files": 0,
    "quarantine_candidates": 0,
    "quarantined": 0
  },
  "evaluation": null
}
```

Ramifica siempre por `schema_version` antes de leer el resto. La salida humana
puede mejorar entre versiones; la estructura del JSON no.

## Úsalo como puerta

```sh
normfix check --format json > report.json || exit 1
```

El código de salida `1` aquí significa "hay trabajo por hacer", que es
exactamente lo que quiere una comprobación previa a un merge.
