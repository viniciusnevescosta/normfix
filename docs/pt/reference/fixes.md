# O que é corrigido, e o que não é

O formatador nativo de C atualmente trata casos comprovados nestas áreas:

- remoção de BOM UTF-8, normalização de CRLF, espaços no fim da linha,
  sequências de linhas em branco, espaços no início do arquivo e uma única
  quebra de linha final;
- indentação e espaçamento de pré-processador, exceto formas multilinha
  sensíveis;
- ordem do bloco de includes: cabeçalhos de sistema antes dos do projeto, em
  ordem alfabética dentro de cada categoria;
- linhas em branco obrigatórias e proibidas em torno de declarações,
  pré-processadores e funções;
- chaves e corpos de controle que precisam da própria linha física;
- layout de controle Allman, remoção conservadora de blocos redundantes de uma
  única instrução e uma limpeza estreita de `else` redundante quando ambos os
  ramos retornam;
- indentação com paradas de tabulação de quatro colunas e diagnósticos comuns
  de espaço/tabulação;
- indentação e a linha em branco obrigatória seguinte para grupos simples de
  declarações locais iniciais;
- espaçamento em torno de operadores, ponteiros, parênteses, palavras-chave e
  declaradores de função;
- alinhamento de grupo para variáveis simples de uma linha e protótipos de
  função, incluindo declaradores de ponteiro quando o grupo é inequívoco;
- `return value;` para `return (value);`;
- listas de parâmetros vazias em definições de função para `(void)`;
- `return (0);` de retorno-ponteiro para `return (NULL);` quando o tipo de
  retorno e um provedor visível de `NULL` estão ambos comprovados;
- quebra de linha em operadores ou vírgulas comprovados;
- rejunção gulosa de linhas de continuação enquanto o resultado permanecer
  dentro de 80 colunas de exibição.

O empacotamento de linhas longas não atravessa comentários, diretivas de
pré-processamento, emendas de linha nem instruções não relacionadas. Strings e
comentários não são divididos. Linhas de pré-processador não são reescritas
apenas para satisfazer a largura.

### Ordem dos includes

Uma sequência de diretivas `#include` só é reordenada enquanto **cada** linha
dela for exatamente uma diretiva de include. A primeira linha que for outra
coisa (um comentário, uma linha em branco, um condicional, uma definição de
macro ou texto após o delimitador final) encerra a sequência, e as diretivas de
cada lado são ordenadas de forma independente. Nenhuma diretiva é movida através
dessa construção, porque atravessá-la pode mudar declarações, macros de recurso
ou compilação condicional.

```c
# include "libft.h"          # include <limits.h>
# include "ft_printf.h"  ->  # include <stdlib.h>
# include <stdlib.h>         # include "ft_printf.h"
# include <limits.h>         # include "libft.h"
```

A ordenação é primeiro por categoria (`<sistema>` antes de `"projeto"`), depois
pelo nome do cabeçalho, comparado sem diferenciar maiúsculas de minúsculas.
Nomes iguais mantêm a ordem relativa original. Use `--no-reorder-includes` para
deixar todos os blocos intactos; o relatório então recorre ao aviso
`INCLUDE_ORDER_REVIEW`.

O formatador mede células de exibição do terminal: tabulações usam paradas de
quatro colunas, marcas combinantes usam zero células e caracteres Unicode largos
usam duas.

### Provas obrigatórias

A formatação acontece primeiro apenas em memória. Para cada ação de layout:

- a fonte precisa ser analisada sem regiões `ERROR`, `MISSING` ou de fita
  desconhecida;
- a fita de tokens precisa cobrir e reconstruir a entrada completa;
- a impressão digital ordenada de tokens e comentários precisa permanecer
  idêntica;
- o candidato precisa ser reanalisado sem recuperação;
- os intervalos de edição precisam ser válidos e não sobrepostos.

Depois que o candidato completo é produzido, a Norminette roda de novo. Se
qualquer contagem de regra subir em relação ao que foi medido antes, o lote
de formatação nativa é revertido para aquele arquivo. Falhas operacionais nunca
autorizam uma gravação parcial.

Ações estreitas que mudam tokens, como `return (...)` e `(void)`, são ações
semânticas separadas com regras próprias de construção; elas não são tratadas
como edições genéricas de espaço em branco.

## Diagnósticos que continuam manuais

O relatório do terminal explica a regra, o trecho exato de fonte, a origem e um
próximo passo concreto para trabalhos como:

- funções com mais de 25 linhas de corpo;
- mais de 4 parâmetros, 5 variáveis locais ou 5 funções por arquivo `.c`;
- linhas com mais de 80 colunas sem uma quebra segura em operador/vírgula;
- estruturas de controle proibidas, ternários, `goto`, rótulos e atribuições em
  condições;
- separação entre declaração e atribuição, e declarações após instruções;
- identificadores públicos ou globais que precisam de renomeação no projeto
  inteiro;
- movimentação de tipos/includes e mudanças de estrutura do projeto;
- declarações ambíguas, ponteiros de função, atributos, campos de bits e
  declaradores multilinha;
- C malformado ou recuperado pelo analisador;
- guardas de cabeçalho que não passam na prova fechada da árvore de trabalho.

A camada semântica avalia um subconjunto conservador de expressões constantes
inteiras de C, incluindo constantes de enum. Isso permite que um limite de enum
conhecido, como `count[op_total]`, seja relatado como um falso positivo
informativo de compatibilidade com a Norminette, em vez de um array de
comprimento variável de verdade. Expressões não suportadas permanecem
desconhecidas; elas nunca são adivinhadas.

Para uma função longa, o diagnóstico sugere extrair uma região coesa e informa o
orçamento aplicável. Ele nunca move instruções, inventa parâmetros nem cria uma
função auxiliar automaticamente: fluxo de dados, nomes, visibilidade e a
intenção do projeto não podem ser provados apenas a partir de fatos de
formatação.
