# Relatórios, códigos de saída e desempenho

## Lendo um diagnóstico

Todo diagnóstico é mostrado contra o código a que se refere, para você ir direto
à linha em vez de procurar uma coordenada:

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

Os acentos circunflexos cobrem os bytes exatos a que a regra se refere, não
apenas o primeiro caractere. Ocorrências de uma mesma regra são agrupadas sob um
único título, cada uma rotulada com sua própria mensagem, e a ajuda, as notas, a
origem e a dica de `explain` compartilhadas são declaradas uma vez para o grupo,
em vez de repetidas sob cada ocorrência.

A visão padrão mostra as três primeiras ocorrências de uma regra e diz quantas
segurou, porque um projeto pode carregar milhares de um mesmo diagnóstico. O
`--verbose` mostra todas, cada uma em sua própria seção com seu próprio trecho.

Alguns detalhes que vale conhecer:

- Tabulações são expandidas, então o acento cai sob o caractere certo.
- Caracteres de controle na sua fonte são mostrados como figuras visíveis e
  nunca chegam ao terminal como controles.
- A coluna na linha `-->` é contada em caracteres, a convenção que um compilador
  C usa. A Norminette oficial conta colunas de exibição, então a saída dela pode
  nomear uma coluna maior para o mesmo caractere em uma linha indentada com
  tabulações. O acento é a resposta autoritativa para *onde*.
- Um diagnóstico de compilador que pertence a um arquivo sem uma posição dentro
  dele, normalmente porque a localização real está em um cabeçalho incluído,
  nomeia o arquivo e o cabeçalho em vez de desenhar um acento sobre código não
  relacionado.

A renderização usa [`annotate-snippets`], a biblioteca com que o próprio `rustc`
renderiza seus diagnósticos.

[`annotate-snippets`]: https://crates.io/crates/annotate-snippets

## O resto da saída

- uma tabela de status por arquivo: `CLEAN`, `INFO`, `FIXED`, `WOULD FIX`,
  `REVIEW` ou `FAILED`;
- IDs de regra estáveis, ajuda compartilhada, notas, origem do diagnóstico e uma
  dica `normfix explain RULE`;
- detalhes opcionais das correções aceitas com `--verbose`;
- diffs unificados com `--diff`;
- contagens agregadas e o tempo decorrido.

A cor é habilitada apenas para um stdout interativo. `--no-color`, `NO_COLOR`,
saída JSON e saída redirecionada ficam sem cor. Os trechos são renderizados
contra uma largura fixa, então um relatório é lido do mesmo jeito em duas
máquinas.

Antes da descoberta, o modo humano escreve um bloco compacto
`normfix · starting` no `stderr` com a ação, o escopo efetivo, o modo de
gravação/verificação, a origem da identidade, a política do verificador, os
processos, o cache, a verificação do compilador, os backups e as capacidades
destrutivas pedidas. Isso torna óbvia uma execução acidental na raiz ou no
diretório pessoal antes que o trabalho comece. Esses escopos protegidos recusam
sem `--force`.

O modo JSON escreve, no lugar, um objeto de evento `execution_start` no
`stderr`. O relatório final versionado continua sendo o único documento JSON no
`stdout`, então a automação existente pode continuar a analisá-lo como um único
valor.

O `--format json` emite um schema determinístico e formatado com
`schema_version: 2`. Ele inclui metadados de identidade, resultados de descoberta
e quarentena, campos de mudança/gravação/falha por arquivo, correções,
diagnósticos antes/depois, contagens de resumo, o `evaluation` opcional do
preflight e `duration_seconds`. Buffers de fonte e diffs unificados são
intencionalmente omitidos.

O `normfix preflight` adiciona uma estimativa determinística e explicitamente não
conclusiva: `score`, `grade`, `verdict` e `hard_failures` com localização exata.
O veredito é `hard_fail` quando o escopo avaliado contém um arquivo inesperado,
um achado corroborado pela Norminette oficial instalada ou um diagnóstico de
Makefile. As evidências de Norminette e de Makefile vêm do snapshot original em
disco, mais qualquer falha adicional exposta na sombra final. Portanto, um
problema corrigível automaticamente continua sendo uma reprovação do preflight
até que os bytes propostos sejam de fato gravados e verificados de novo.
A nota numérica é uma heurística limitada de priorização, não uma nota da 42; ela
não cobre comportamento em execução, testes específicos do projeto, vazamentos,
julgamento dos pares nem perguntas de defesa.

Este é o objeto `evaluation` de uma execução real, em um projeto cujo único
problema restante é um Makefile listando uma fonte que foi apagada:

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

`conclusive` é `false` em todo relatório que esta ferramenta consegue produzir;
ele existe para que um consumidor nunca precise inferir esse limite a partir da
prosa. `notes` faz parte do documento, e não da decoração do terminal, então um
agente que repassa o resultado leva as ressalvas junto. Leia `verdict` para a
decisão e `score` apenas para ordenar o trabalho: o veredito continua
`hard_fail` enquanto restar qualquer reprovação, por mais alta que a nota fique.

### Códigos de saída

| Código | Significado |
|---:|---|
| `0` | O modo de correção terminou sem diagnóstico bloqueante, ou a entrada já estava limpa |
| `1` | Restam diagnósticos manuais, o modo de pré-visualização encontrou mudanças propostas/candidatos a quarentena, ou o preflight bateu em uma regra de reprovação |
| `2` | Falha de descoberta, configuração, ferramenta, E/S, transação ou quarentena |
| `130` | Uma revisão interativa por arquivo foi cancelada |

Avisos informativos não fazem uma execução falhar.

## Cache e desempenho

A análise de arquivos roda em paralelo com o Rayon. O `--threads N` cria um pool
local com uma contagem exata de processos; sem ele, o Rayon usa o hardware
disponível. Resultados e confirmações são ordenados por caminho, então a ordem
de conclusão dos processos não muda a ordem do relatório nem a ordem de
gravação.

Relatórios da Norminette oficial usam tanto um cache de execução em memória
quanto um banco redb persistente fora do projeto. No Unix:

```text
$XDG_CACHE_HOME/normfix/<project-id>/cache-v1.redb
```

ou:

```text
~/.cache/normfix/<project-id>/cache-v1.redb
```

As chaves incluem o schema, o espaço de nomes da análise, o caminho relativo ao
projeto quando a entrada está dentro da raiz da execução (com recurso ao caminho
absoluto para uma entrada externa explícita), os bytes da fonte, a configuração
da Norm e a impressão digital verificada do executável. Falhas de trava, E/S,
decodificação ou corrupção do cache falham abertas como perdas; elas nunca mudam
diagnósticos nem o status de saída. Um banco corrompido é preservado sob um nome
`.corrupt-N` antes de ser recriado.

Use `--no-cache` para uma execução totalmente sem cache.
