# `normfix preflight`

As verificações somente leitura que vale a pena rodar imediatamente antes de uma
avaliação da 42, com a passagem estrita do compilador ativada.

```sh
normfix preflight
```

Ele roda tudo o que o [`check`](/pt/commands/check) roda, mais
`cc -fsyntax-only -Wall -Wextra -Werror` contra as unidades de tradução reais em
disco.

```console
$ normfix preflight
error[CC_IMPLICIT_FUNCTION_DECLARATION]: 2 occurrences in 2 files
  srcs/sort/sort.c:30:5           call to undeclared function 'sort_medium'
  srcs/sort/sort_adaptive.c:21:5  call to undeclared function 'sort_medium'
    note: Compiler diagnostics inspect the original on-disk translation unit
          and never authorize or reject formatter edits.
 = help: Fix this strict compiler diagnostic, then rerun normfix.
 = source: C compiler
```

Esse exemplo é real: um cabeçalho declarava `sort_medium`, mas nenhum arquivo o
definia, então o projeto não compilava. A Norminette nunca teria contado isso.

## Uma execução completa, antes e depois

Toda saída desta página vem de uma execução real. O projeto abaixo tem quatro
arquivos: `main.c` e `add.c` indentados com espaços, um `demo.h` declarando um
`unused_api` que ninguém implementa, e um Makefile cujo `SRC` ainda lista um
`ghost.c` que foi apagado.

O preflight diz o que vai fazer antes de ler qualquer coisa:

```console
$ normfix preflight
normfix · starting
  action       preflight
  mode         read-only check
  scope        /home/student/demo (recursive)
  working dir  /home/student/demo
  identity     student@student.42.fr (user config)
  workers      auto
  checks       Norminette + strict compiler
  norminette   automatic PATH discovery
  version rule advisory (other releases continue)
  timeout      5s per file
  cache        enabled
  gitignore    not applied
  backups      automatic external backup
  destructive  none
  force        no
```

Depois ele relata a estimativa contra os bytes que estão em disco agora:

```console
Pre-defense estimate: HARD FAIL | grade FAIL | 31/100
This estimate is heuristic and never replaces the official evaluation.
Hard-fail evidence
  Makefile:1:1 [INVALID_HEADER] The official 42 Makefile header is missing or malformed
  add.c:1:1 [INVALID_HEADER] Missing or invalid 42 header
  demo.h:1:1 [INVALID_HEADER] Missing or invalid 42 header
  main.c:1:1 [INVALID_HEADER] Missing or invalid 42 header
  Makefile:2:20 [MAKEFILE_SOURCE_NOT_FOUND] The literal Makefile source `ghost.c` does not exist below the project root.
  add.c:3:4 [SPACE_BEFORE_FUNC] Found space when expecting tab before function name
  main.c:3:4 [SPACE_BEFORE_FUNC] Found space when expecting tab before function name
  main.c:5:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:5:8 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:7:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:8:5 [SPACE_REPLACE_TAB] Found space when expecting tab
  main.c:5:1 [TOO_FEW_TAB] Missing tabs for indent level
  main.c:7:1 [TOO_FEW_TAB] Missing tabs for indent level
  main.c:8:1 [TOO_FEW_TAB] Missing tabs for indent level
```

A maior parte dessa lista é exatamente o que o `normfix` conserta. Rodando a
correção padrão e perguntando de novo:

```console
$ normfix
$ normfix preflight
Pre-defense estimate: HARD FAIL | grade FAIL | 59/100
This estimate is heuristic and never replaces the official evaluation.
Hard-fail evidence
  Makefile:14:20 [MAKEFILE_SOURCE_NOT_FOUND] The literal Makefile source `ghost.c` does not exist below the project root.
```

Treze reprovações sumiram e uma permanece, e esse é o resultado útil: o
`ghost.c` apagado ainda está listado no Makefile, e nenhuma ferramenta deveria
decidir sozinha se aquele arquivo deve voltar ou se a linha deve sair. O veredito
continua `HARD FAIL` enquanto restar qualquer reprovação — a nota se move, o
veredito não amolece.

Os bytes avaliados são os bytes entregues. Na primeira execução o `normfix` já
tinha calculado as correções de todo `INVALID_HEADER` e `SPACE_REPLACE_TAB`
acima, e a estimativa mesmo assim reprovou por causa deles, porque um conserto
que você não gravou não faz parte do que um avaliador vai abrir.

Todo fluxo apoiado no sistema de arquivos, inclusive o check padrão, compara
protótipos não estáticos dos cabeçalhos do projeto com cada arquivo C/cabeçalho
do projeto analisado sem perdas. Uma implementação ausente, ou uma definição
correspondente cujo corpo é só chaves, espaços e comentários, é destacada no
nome do protótipo. Fontes geradas e bibliotecas externas continuam ambíguas. O
modo `--unsafe` explicitamente autorizado remove apenas um protótipo sem
implementação quando o conjunto completo de fontes não contém nenhuma definição,
chamada, ponteiro de função/referência, macro, string, condicional, atributo ou
colagem de tokens como evidência. Uma definição existente só com trivialidades é
apenas um aviso, porque um no-op intencional pode ser válido.

## Estimativa e regras de reprovação

O relatório termina com uma estimativa de 0 a 100, uma faixa de nota e um
veredito. Ele é sempre rotulado como **não conclusivo**. É um auxílio de
priorização, não uma nota prevista da 42.

O veredito é `HARD FAIL` quando qualquer uma destas condições objetivas está
presente:

- um arquivo inesperado no escopo avaliado;
- um achado de Norm corroborado pela Norminette oficial instalada;
- um diagnóstico estático de Makefile ou uma falha de processamento do Makefile.

Cada item de reprovação de fonte repete seu `caminho:linha:coluna` exato, o ID da
regra e a mensagem. Uma falha operacional de Makefile nomeia o arquivo sem
inventar uma coordenada de fonte.
Achados oficiais de Norm e de Makefile são avaliados contra os bytes originais em
disco; uma correção proposta somente leitura não transforma a entrega atual em
aprovação. Achados novos que permanecem na sombra final também são incluídos.
A ausência de README não é uma reprovação. Quando um README está presente, um
aviso informativo pede que você o compare com a ficha de assunto/avaliação
atual.
Se nenhum Makefile regular for selecionado ou encontrado na raiz do projeto,
`MAKEFILE_NOT_FOUND` diz que a verificação de build está incompleta. Ele
permanece um aviso porque nem todo assunto exige um Makefile e nenhuma política
de projeto foi comprovada.

## O que ele não faz

Ele não roda `make`, não linka um binário, não executa seu programa nem seus
testes, e não prova a ausência de vazamentos. Isso continua sendo seu, e o
relatório diz isso.

O preflight informa se o `clang-tidy` está disponível no `PATH` e mostra uma
receita prática de build de depuração com AddressSanitizer/UndefinedBehaviorSanitizer.
Ele não roda `clang-tidy`, nem sanitizers, nem `make` (nem mesmo `make -n`, que
pode avaliar `$(shell ...)`), nem um binário do projeto. Tal execução exige
confiança separada e explícita no comportamento de build e de execução do
projeto.

O preflight adiciona automaticamente uma passagem limitada de análise estática
profunda: `-fanalyzer` no GCC, `--analyze` no Clang. Fluxos comuns ainda exigem
`--analyzer`. O `normfix` escolhe a partir do banner de versão do compilador, o
que importa porque `/usr/bin/gcc` no macOS é o Clang usando outro nome.

Eles podem *sugerir* um vazamento ou um acesso inválido; nunca provam correção,
e nunca autorizam uma edição. Um compilador sem analisador nenhum reporta
`CC_ANALYZER_UNAVAILABLE` e a execução continua.

`preflight` se recusa a combinar com `--no-compiler-preflight`, porque a
passagem do compilador é o motivo do comando existir.
