# `normfix budget`

Uma execução somente leitura que adiciona uma linha informativa por função
analisada, mostrando quanto espaço resta antes dos limites da Norm: 25 linhas,
5 variáveis locais e 4 parâmetros.

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

Summary: arquivos: 1 | propostos: 0 | gravados: 0 | correções: 0 | pendentes: 14 | informativos: 2
```

As linhas de orçamento são informativas e nunca alteram o código de saída por
si só.

`budget` diagnostica os bytes já no disco e nunca planeja edições. Por isso,
flags de formatação, identidade do cabeçalho, backup, diff e remoção são
rejeitadas em vez de ignoradas silenciosamente. Use `normfix check` para prever
correções.

## Por que a folga importa

Uma função com 24 de 25 linhas está de acordo com a Norm e está a uma pergunta
do dia da defesa de deixar de estar. O `budget` existe para tornar isso visível
antes que um avaliador peça que você acrescente uma verificação.

O `normfix` informa o número; ele nunca extrai uma função por você. Escolher a
fronteira de uma função muda a estrutura do programa, e essa é uma decisão que
precisa de um nome e de um dono. Veja
[`normfix explain TOO_MANY_LINES`](/pt/commands/explain).
