# `normfix lint`

Relata o que está errado nos bytes atualmente em disco. Não propõe nada e não
grava nada: nem formatação, nem o cabeçalho oficial, nem mudanças no Makefile
ou no README.

```sh
normfix lint
normfix lint src
```

Use quando quiser o diagnóstico sem o tratamento: na CI, em uma revisão, ou
quando pretende corrigir algo à mão e não quer que a ferramenta se mexa por
baixo de você.

## O que ele relata

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

Repare no `0 proposed`: o `lint` nunca planeja uma edição. O mesmo projeto sob
[`check`](/pt/commands/check) relata dezessete correções propostas, porque o
`check` tem permissão para planejá-las.

Os diagnósticos são agrupados por regra e cada localização é preservada. Cada
grupo nomeia sua origem (a Norminette oficial, o compilador C, o analisador
nativo ou uma regra de projeto), então você sabe com qual autoridade está
discutindo.

## Na CI

```sh
normfix lint --format json > report.json
```

O JSON mantém os achados individuais e carrega `schema_version`. O código de
saída `1` significa que restam diagnósticos, `0` significa limpo e `2` significa
que a própria execução falhou.
