# Makefiles, documentos README e arquivos de projeto

Makefiles usam um formatador conservador dedicado, porque a Norminette não
analisa a sintaxe do GNU Make. O formatador pode:

- remover um BOM UTF-8 e normalizar as quebras de linha;
- inserir ou atualizar o cabeçalho oficial 42 no estilo `#`;
- garantir uma única quebra de linha final;
- empacotar de forma gulosa atribuições explícitas simples de `.c` até 80
  colunas de exibição, mantendo a ordem e a semântica da atribuição.

Ele preserva deliberadamente receitas, projetos com `.RECIPEPREFIX`, blocos
`define`, atribuições de shell, expansão de variáveis/funções, padrões,
comentários, aspas, separadores de comando e outras construções ambíguas do
Make.

O analisador relata:

- uma atribuição `NAME` ausente;
- regras `all`, `clean`, `fclean`, `re` ou `$(NAME)` ausentes;
- `all` não sendo o alvo concreto padrão;
- descoberta de fontes/objetos por curinga;
- linhas longas que não podem ser empacotadas com segurança;
- espaço em branco após uma barra invertida de continuação;
- uma linha de receita indentada com espaços, que o Make se recusa a ler.

Para uma atribuição simples no estilo `SRC`/`SRCS` cujo valor completo é feito de
caminhos `.c` relativos e literais, ele também verifica se cada token existe e
se o arquivo regular referenciado contém algum token C além de espaços ou
comentários. Os caminhos são resolvidos a partir do diretório que contém aquele
Makefile, inclusive para Makefiles aninhados, e todo componente precisa
permanecer dentro da raiz canônica do projeto e evitar links simbólicos. Um
caminho ausente ou só com trivialidades é relatado por padrão. O `--unsafe` pode
remover apenas o token exato comprovado e reempacotar a lista restante sem
reordená-la. Expansões, padrões, aspas, comentários, receitas, blocos `define`,
`.RECIPEPREFIX`, caminhos que escapam e resultados incertos do sistema de
arquivos são deixados inalterados.

Todo fluxo apoiado no sistema de arquivos compara protótipos não estáticos dos
cabeçalhos do projeto com um snapshot completo e sem perdas de fontes
C/cabeçalho. Implementações ausentes e corpos correspondentes só com
trivialidades são relatados no nome do protótipo. A remoção insegura é limitada
a implementações ausentes e exige o escopo completo do projeto, autorização
delimitada, nenhum outro uso do identificador nem ambiguidade, validação de
uma releitura limpa do resultado e uma verificação de hash, na hora de gravar, de todas
as entradas da prova. Definições existentes só com trivialidades nunca são
removidas: um no-op intencional pode ser válido.

A ferramenta não adiciona automaticamente todo arquivo `.c` encontrado em disco a
uma variável de fontes. Pertencer a um alvo é uma decisão de design do build.

## Preflight do compilador e avisos de vazamento

Para cada arquivo `.c` selecionado, o pipeline normal roda uma passagem somente
leitura do compilador equivalente a:

```text
cc -fsyntax-only -Wall -Wextra -Werror
```

Ele adiciona caminhos `-I` estáveis para os diretórios que contêm cabeçalhos do
projeto descobertos, mas não adivinha defines específicos do assunto, modos de
linguagem, cabeçalhos gerados, flags de alvo nem entradas do linker. Use
`--cc PATH` para selecionar um compilador exato ou `--no-compiler-preflight` para
pular a passagem. Achados do compilador são apenas diagnósticos: eles nunca
autorizam nem rejeitam edições do formatador. Um compilador indisponível ou um
contexto de compilação visivelmente incompleto produz um aviso claro que falha
aberto.

O `--analyzer` adicionalmente pede ao compilador escolhido a saída do
`-fanalyzer` do GCC em fluxos comuns. O preflight faz essa passagem limitada do
analisador automaticamente. Ela pode revelar possíveis vazamentos de alocação e
caminhos de acesso inválido, mas é mais lenta e intencionalmente informativa.
Ela não é uma prova de vazamento: a exploração de caminhos é incompleta, uma
unidade de tradução é inspecionada por vez, e a posse escondida atrás de funções
externas ou guardada em structs pode escapar da análise. Um compilador sem
nenhuma das interfaces de analisador suportadas é relatado e pulado.

### Modo pré-defesa

```sh
normfix preflight
```

O `preflight` é a pré-visualização somente leitura de formatação/lint pensada
para o momento imediatamente anterior à avaliação. Ele agrega resultados
oficiais da Norminette, limites nativos da Norm e sugestões de extração,
cabeçalhos oficiais e guardas de cabeçalho, política de funções permitidas,
estrutura do Makefile e referências literais de fonte, fontes de Makefile só com
trivialidades, protótipos de cabeçalho sem definição no projeto, corpos de
implementação só com trivialidades, arquivos inesperados, achados de README, a
passagem estrita do compilador e o analisador do compilador. As passagens do
compilador não podem ser desativadas nesse fluxo.

A `Pre-defense estimate` final é intencionalmente não conclusiva. Arquivos
inesperados, achados da Norminette instalada e diagnósticos de Makefile produzem
uma reprovação com localizações exatas de fonte. A nota de 0 a 100 e a faixa de
letra apenas priorizam o trabalho restante; elas não são uma nota oficial.

A evidência de reprovação é baseada nos diagnósticos originais em disco da
Norminette e do Makefile, mais qualquer achado recém-exposto que permaneça na
as edições propostas. Uma edição segura proposta pelo modo check não faz os bytes entregues
passarem retroativamente.

Quando o `normfix.toml` está ausente, o preflight emite
`FUNCTION_POLICY_NOT_CONFIGURED` em vez de fingir que a verificação de funções
autorizadas rodou. Ele também emite `PREFLIGHT_MANUAL_STEPS`: o comando
deliberadamente não executa receitas do Make, não linka nem inspeciona o binário
final, não roda o programa/testes e não invoca ferramentas de vazamento em tempo
de execução. Rode esses passos específicos do projeto separadamente. Ele informa
se o `clang-tidy` está no `PATH` e dá orientação separada de sanitizers para
build de depuração, mas não executa nenhum dos dois. Quando nenhum Makefile
regular é selecionado ou encontrado na raiz do projeto, `MAKEFILE_NOT_FOUND`
relata uma verificação incompleta sem reprovar: só uma política específica do
assunto pode provar que todo projeto precisa de um.

## Suporte a README e Markdown

Arquivos README são analisados com o Comrak e reimpressos canonicamente por
padrão:

```sh
normfix README.md
```

A reimpressão canônica é idempotente, mas pode criar um diff amplo na primeira
execução. Use `--check` ou `--diff` para pré-visualizá-la. O
`--no-format-markdown` mantém os arquivos README somente leitura, ainda
relatando saltos de nível de título, espaços no fim da linha e a falta de uma
quebra de linha final.

Quando o preflight descobre um README, `README_42_CRITERIA_REVIEW` lembra você de
compará-lo com a ficha de assunto e avaliação atual. A ausência de README não
emite diagnóstico e nunca reprova o preflight.

## Arquivos inesperados no projeto

A descoberta recursiva relata arquivos regulares que não sejam `.c`, `.h`,
`Makefile`, variantes de README, `.normfixignore` e seu apelido legado
`.norminetteignore`. Fora do preflight, esse aviso sozinho não altera o status de
saída. O preflight o usa como regra explícita de reprovação, porque espera-se que
o escopo de entrega avaliado contenha apenas arquivos de projeto suportados.
Isso nunca implica que um arquivo seja descartável.

Use `--remove-unexpected` apenas quando pretender mover todos os arquivos
regulares inesperados elegíveis para a quarentena externa. Links simbólicos,
diretórios, caminhos fora do projeto, snapshots alterados e caminhos de
recuperação sobrepostos são rejeitados.
