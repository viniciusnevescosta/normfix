# `normfix check`

Executa o pipeline completo de correção em memória e relata o resultado sem
tocar em um único arquivo.

```sh
normfix check
normfix check main.c
```

`normfix --check` é a mesma coisa.

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

`WOULD FIX` e `4 proposed` são a diferença em relação ao [`lint`](/pt/commands/lint):
o `check` planeja as edições e diz quantas passaram pelas provas, apenas não as
confirma.

Os dois status respondem perguntas diferentes. `WOULD FIX` significa que tudo o
que foi encontrado naquele arquivo tem uma correção comprovada esperando —
`add.c` e `main.c` não precisam de nada de você. `REVIEW` significa que algo
sobra depois de aplicada toda correção segura, e a coluna `REMAINING` conta
isso: aqui o Makefile lista uma fonte que não existe e `demo.h` declara uma
função que ninguém implementa. Nenhum dos dois tem resposta automática segura,
então ambos são relatados em vez de adivinhados.

Lendo o resumo da esquerda para a direita: 4 arquivos foram analisados, 4 têm
mudanças propostas, nenhum foi gravado porque isto é `check`, 12 correções
individuais passaram pelas provas e 2 achados ainda precisam de uma pessoa.

## Legível por máquina

```console
$ normfix check --format json
{
  "schema_version": 2,
  "tool_version": "1.5.0",
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

Sempre ramifique pelo `schema_version` antes de ler o resto. A saída humana pode
melhorar entre versões; a estrutura do JSON, não.

## Use como portão

```sh
normfix check --format json > report.json || exit 1
```

O código de saída `1` aqui significa "há trabalho a fazer", que é exatamente o
que uma verificação de pré-merge quer.
