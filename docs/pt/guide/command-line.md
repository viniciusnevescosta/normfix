# Linha de comando

A interface sem subcomando é o caminho mais curto para formatar um projeto. Os
subcomandos deixam a intenção mais clara em scripts e em revisões interativas.

```sh
normfix format src includes
normfix lint
normfix check main.c
normfix budget src
normfix preflight
normfix explain TOO_MANY_LINES
normfix undo --list
normfix undo --run RUN_ID
```

## Fluxos

| Comando | Grava arquivos | O que faz |
|---|---|---|
| `format` | sim | Aplica as edições aceitas |
| `lint` | não | Relata diagnósticos sobre os bytes originais; não propõe formatação, cabeçalho, Makefile nem substituição de Markdown |
| `check` | não | Roda formatação e lint em um buffer sombra |
| `budget` | não | Uma execução de lint mais uma linha informativa de linhas/variáveis/parâmetros por função analisada |
| `preflight` | não | Uma execução orientada a check com a verificação estrita do compilador ativada; não executa `make` nem o programa |
| `explain` | não | Imprime a explicação embutida em inglês de um ID de regra estável, sem varrer um projeto |
| `undo` | sim | Lista ou restaura um backup de transação íntegro |
| `uninstall` | sim | Remove este binário e, com `--purge`, os dados que ele criou |

O `undo` se recusa a sobrescrever bytes alterados depois da execução que ele
restaura. Sem `--run`, ele seleciona o ponto de recuperação válido mais recente
após confirmação interativa; a restauração não interativa exige `--force`.

## Opções

| Opção | Comportamento |
|---|---|
| `PATH...` | Zero, um ou muitos arquivos/diretórios; zero significa o diretório atual |
| `--check` | Planeja e relata mudanças sem gravar |
| `--diff` | Imprime diffs unificados na saída humana sem gravar |
| `--changed` | Seleciona mudanças rastreadas não indexadas mais arquivos não rastreados e não ignorados pelo Git |
| `--staged` | Seleciona apenas caminhos registrados como alterados no índice do Git |
| `--interactive` | Pré-visualiza, mostra o diff de cada arquivo alterado e pergunta quais gravar |
| `--use-gitignore` | Respeita o `.gitignore` durante a descoberta recursiva de diretórios |
| `--login LOGIN` | Fornece ou restringe o login 42 usado na validação de identidade |
| `--email EMAIL` | Fornece o e-mail verificado de estudante 42 usado nos cabeçalhos oficiais |
| `--no-backup` | Desativa os backups retidos para gravações comuns e seguras de formatação |
| `--backup-dir PATH` | Usa uma base externa específica de backup |
| `--format human\|json` | Seleciona a saída de terminal ou o relatório JSON versionado |
| `--lang CODE` | Idioma da saída humana: `en`, `pt`, `es` ou `fr` |
| `--no-color` | Desativa a cor ANSI |
| `-v`, `--verbose` | Lista cada correção aceita na saída humana |
| `--timeout SECONDS` | Tempo limite da Norminette por invocação; padrão: 5 segundos |
| `--threads N` | Número de processos paralelos; padrão: o hardware disponível |
| `--remove-invalid-comments` | Apaga apenas comentários rejeitados em localizações oficiais exatas |
| `--remove-unused` | Remove apenas funções `static` inalcançáveis comprovadas em um projeto completo |
| `--remove-unexpected` | Move arquivos regulares inesperados para uma quarentena externa recuperável |
| `--unsafe` | Ativa o conjunto fechado de ações arriscadas/destrutivas |
| `--force` | Confirma as capacidades destrutivas pedidas ou reconhece um escopo protegido |
| `--no-reorder-includes` | Deixa os blocos contíguos de include na ordem atual |
| `--no-format-markdown` | Analisa documentos README sem reimpressão canônica em CommonMark |
| `--no-cache` | Desativa o cache externo persistente de análise |
| `--norminette PATH` | Usa um executável exato da Norminette |
| `--strict-norminette-version` | Recusa uma versão do verificador diferente da testada |
| `--no-compiler-preflight` | Pula a passagem consultiva estrita do compilador C, ativa por padrão |
| `--cc PATH` | Usa um compilador C exato para o preflight e a análise |
| `--analyzer` | Adiciona o analisador limitado do GCC/Clang aos fluxos comuns; o preflight o ativa automaticamente |
| `-h`, `--help` | Mostra a ajuda embutida |
| `-V`, `--version` | Mostra a versão da CLI nativa |

`--check` e `--diff` são mutuamente exclusivos. `--changed` e `--staged` são
mutuamente exclusivos e não podem ser combinados com argumentos explícitos de
caminho. `--force` sem `--unsafe`, `--remove-unused` ou `--remove-unexpected` é
um erro, a menos que o próprio escopo seja protegido. Raízes do sistema de
arquivos, o diretório pessoal completo, raízes amplas como `/Users` e `/home` e
árvores do sistema operacional recusam antes da descoberta sem esse
reconhecimento explícito.

## Ordem dos includes

Uma sequência de diretivas `#include` é reordenada para que os cabeçalhos de
sistema venham primeiro, depois os do projeto, em ordem alfabética dentro de
cada categoria:

```c
# include "libft.h"          # include <limits.h>
# include "ft_printf.h"  ->  # include <stdlib.h>
# include <stdlib.h>         # include "ft_printf.h"
# include <limits.h>         # include "libft.h"
```

::: warning O bloco precisa ser comprovadamente contíguo
Uma sequência só é reescrita enquanto **cada** linha dela for exatamente uma
diretiva de include. A primeira linha que for outra coisa (um comentário, uma
linha em branco, um condicional, uma definição de macro ou texto após o
delimitador final) encerra a sequência, e cada lado é ordenado de forma
independente. Nenhuma diretiva atravessa tal construção, porque fazer isso pode
mudar declarações, macros de recurso ou compilação condicional.
:::

Os nomes são comparados sem diferenciar maiúsculas de minúsculas e nomes iguais
mantêm a ordem relativa original. `--no-reorder-includes` deixa todos os blocos
intactos; o relatório então recorre ao aviso `INCLUDE_ORDER_REVIEW`, que
`normfix explain INCLUDE_ORDER_REVIEW` descreve offline.

## Escopos do Git

A seleção de escopo pelo Git acontece antes da descoberta normal:

```sh
normfix check --changed
normfix format --staged
```

`--changed` significa mudanças rastreadas não indexadas mais arquivos não
rastreados que o Git não ignora; ele deliberadamente não inclui caminhos apenas
indexados. `--staged` usa o diff do índice para selecionar nomes e então analisa
e formata os bytes atuais da árvore de trabalho. Ele não reescreve o índice nem
indexa o resultado.

Um escopo vazio é um no-op bem-sucedido e nunca recai para uma varredura de
diretório inteiro. O Git é invocado diretamente, com caminhos delimitados por
NUL, um tempo limite, um limite de saída e verificações de confinamento de
caminho. Nomes absolutos ou que escapam são rejeitados. Um candidato que é um
link simbólico ou não é um arquivo regular é omitido com segurança; uma falha de
metadados ou do Git rejeita o escopo inteiro em vez de varrer silenciosamente
outro conjunto.

::: tip Um escopo não é uma prova
O escopo do Git é uma conveniência de revisão, não uma prova de projeto
completo. Achados abrangentes que precisam de um snapshot fechado são
desativados quando o escopo não consegue fornecer um.
:::

## Revisão interativa

```sh
normfix format --interactive
```

A primeira passagem é somente leitura: o `normfix` imprime o relatório e o diff
de cada arquivo proposto, aceitando `y`, `n`, `a` (todos) ou `q` (cancelar). Ele
então analisa de novo o mesmo escopo selecionado. Cada aprovação fica vinculada
aos hashes dos bytes originais e propostos exatos mostrados na primeira
passagem, e a transação grava apenas os arquivos cujo plano da segunda passagem
ainda corresponde àquela aprovação vinculada ao snapshot.

O modo interativo exige um terminal humano e não pode ser combinado com
pré-visualização, JSON, lint/budget ou operações arriscadas/destrutivas.

## Comportamento de ignorar

Varreduras recursivas respeitam o `.normfixignore` por padrão, usando o estilo
de ignore do Git suportado pelo crate `ignore`. O nome legado
`.norminetteignore` continua suportado para que projetos existentes não
recuperem silenciosamente entradas ignoradas.

O `.gitignore` é deliberadamente opcional, via `--use-gitignore`, porque
arquivos C ignorados ainda podem afetar provas de projeto inteiro. Argumentos
explícitos de arquivo continuam explícitos e não são filtrados por arquivos de
ignore.

## Códigos de saída

| Código | Significado |
|---:|---|
| `0` | O modo de correção terminou sem diagnóstico bloqueante, ou a entrada já estava limpa |
| `1` | Restam diagnósticos manuais, ou o modo de pré-visualização encontrou mudanças propostas/candidatos a quarentena |
| `2` | Falha de descoberta, configuração, ferramenta, E/S, transação ou quarentena |
| `130` | Uma revisão interativa por arquivo foi cancelada |

Avisos informativos não fazem uma execução falhar.
