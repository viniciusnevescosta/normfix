# `normfix explain`

Imprime a explicação embutida de uma regra. Não varre nenhum projeto, não lê
nenhum arquivo e não usa a rede.

```sh
normfix explain TOO_MANY_LINES
normfix explain INCLUDE_ORDER_REVIEW
normfix explain VLA_COMPAT_FALSE_POSITIVE
```

Todo diagnóstico de um relatório normal termina com o comando exato da própria
regra, então raramente é preciso digitar o identificador de memória:

```text
 = explain: normfix explain TOO_MANY_WS
```

## O formato de uma resposta

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

São sempre quatro partes: o que a regra é, por que ela existe, o que fazer em
seguida e por que a ferramenta agiu ou não agiu sozinha.

## Famílias de regras

Identificadores com o prefixo `CC_` vêm do compilador C e `CC_ANALYZER_` vêm do
`-fanalyzer`; ambos são explicados de forma genérica, porque a mensagem
autoritativa é a do próprio compilador. Todo o resto é um nome de regra oficial
da Norminette ou uma regra nativa do `normfix`.

Um identificador desconhecido ainda recebe uma resposta útil, em vez de um
erro. O conjunto de artigos embutidos é uma conveniência, não a fonte da
verdade.
