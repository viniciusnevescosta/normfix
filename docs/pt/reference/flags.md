# Todas as flags

Cada entrada diz o que a flag faz, quando você recorreria a ela, e mostra a flag
em uso. As flags são globais: funcionam com o comando puro e com todos os
subcomandos.

Rode `normfix --help` para a mesma lista sem a prosa.

## Selecionando o que processar

### `PATH...`

Zero, um ou muitos arquivos e diretórios. Zero significa o diretório atual,
varrido recursivamente sem seguir links simbólicos.

```sh
normfix                                   # the whole project
normfix main.c                            # one file
normfix src includes                      # two directories
normfix src/parser.c includes/shell.h     # a mixture
```

Um argumento explícito de arquivo é sempre processado, mesmo que um arquivo de
ignore o tivesse excluído.

### `--changed`

Processa mudanças rastreadas não indexadas mais arquivos não rastreados que o
Git não ignora.

```sh
normfix --changed
```

Use enquanto trabalha: ele formata o que você acabou de tocar em vez de
reescrever o projeto inteiro. Ele exclui deliberadamente caminhos apenas
indexados.

### `--staged`

Processa apenas os caminhos registrados como alterados no índice do Git.

```sh
normfix check --staged
```

Ele lê o índice para selecionar *nomes* e depois analisa os bytes atuais da
árvore de trabalho. Ele não reescreve o índice nem indexa o resultado, então o
`git diff --staged` não é afetado.

Não pode ser combinado com `--changed` nem com caminhos explícitos. Um escopo
vazio é um no-op bem-sucedido, e ele nunca recai para varrer tudo.

### `--use-gitignore`

Também respeita o `.gitignore` durante a descoberta recursiva.

```sh
normfix --use-gitignore
```

Desligado por padrão, deliberadamente: um arquivo C que você mandou o Git
ignorar ainda participa de provas de projeto inteiro, como a verificação de
funções permitidas. O `.normfixignore` é sempre respeitado.

## Pré-visualizar em vez de gravar

### `--check`

Planeja tudo, não grava nada.

```sh
normfix --check
normfix --check --format json > report.json
```

O código de saída `1` significa que há trabalho a fazer, o que o torna um portão
de CI de uma linha.

### `--diff`

Imprime um diff unificado de cada mudança proposta e não grava nada.

```sh
normfix --diff
normfix --diff src/parser.c
```

Tabulações são exibidas como `\t` para que mudanças de indentação continuem
visíveis. Mutuamente exclusivo com `--check`.

### `--interactive`

Pré-visualiza cada arquivo alterado e escolhe quais serão gravados.

```sh
normfix format --interactive
```

Responda `y`, `n`, `a` (todos) ou `q` (cancelar). A aprovação fica vinculada aos
bytes exatos que você viu; se um arquivo mudar por baixo de você, ele é pulado em
vez de gravado. Exige um terminal e se recusa a combinar com `--check`,
`--diff`, saída JSON ou flags destrutivas.

## Identidade para os cabeçalhos oficiais

### `--login LOGIN`

Fornece ou restringe o login 42 usado no cabeçalho oficial.

```sh
normfix --login vneves-c
```

### `--email EMAIL`

Fornece o e-mail verificado de estudante 42. O e-mail é a fonte da verdade; o
login é validado contra ele.

```sh
normfix --email vneves-c@student.42.fr
```

Sem nenhuma das duas flags, o `normfix` resolve a identidade a partir do seu
ambiente e da configuração do Git, e pergunta interativamente quando não
consegue e a execução precisa de uma. Uma identidade válida fornecida
explicitamente, ou uma resposta válida a esse prompt, é salva atomicamente na
configuração privada por usuário da plataforma, para que execuções posteriores
não perguntem de novo. Veja [cabeçalhos oficiais](/pt/reference/headers) para
caminhos e permissões.

## Backups e recuperação

### `--no-backup`

Pula os backups retidos para gravações comuns de formatação.

```sh
normfix --no-backup
```

Flags de backup se aplicam apenas a uma execução que grava. `check`, `lint`,
`budget`, `preflight`, `--check` e `--diff` as rejeitam porque essas execuções
não podem criar um backup.

Ele **não** pula a recuperação de uma remoção destrutiva. Essas sempre exigem
armazenamento externo e falham fechadas sem ele. Pular os backups significa que
o [`undo`](/pt/commands/undo) não tem nada a restaurar daquela execução.

### `--backup-dir PATH`

Usa uma base externa específica de backup em vez do padrão sob
`$XDG_DATA_HOME`.

```sh
normfix --backup-dir ~/normfix-backups
```

O diretório não pode se sobrepor ao projeto. Um caminho dentro dele, ou acima
dele, é recusado, antes e depois de resolver links simbólicos.

## Saída

### `--format human|json`

Saída de terminal, ou o relatório JSON versionado.

```sh
normfix --check --format json | jq '.summary'
```

Sempre ramifique pelo `schema_version` antes de ler o JSON. O layout humano pode
melhorar entre versões; a estrutura do JSON, não.

### `--lang`

Escolhe o idioma da saída humana: `en`, `pt`, `es` ou `fr`.

```sh
normfix check --lang pt
```

```console
$ normfix check --lang pt
normfix · iniciando
  ação             check
  modo             somente leitura
  escopo           /home/student/demo (recursivo)
...
Resumo: arquivos: 1 | propostos: 1 | gravados: 0 | correções: 1 | pendentes: 0 | informativos: 0 | com falha: 0 | inesperados: 0 | 0 em quarentena
Concluído em 219 ms.
```

Sem a flag, a locale do processo é usada — `NORMFIX_LANG`, depois `LC_ALL`,
`LC_MESSAGES` e `LANG` — recorrendo ao inglês. Só o subtag primário importa,
então `pt_BR.UTF-8` seleciona o português. Um valor de `--lang` não publicado
continua em inglês com um aviso, em vez de falhar.

Isso muda apenas as explicações. Nomes de comandos, grafias de flags, IDs de
regra, códigos de saída e todos os valores em `--format json` permanecem
idênticos nos quatro idiomas, então um script nunca precisa selecionar um idioma
para continuar funcionando.

As mensagens de regra vindas dos analisadores continuam em inglês. Uma execução
não-inglesa diz isso em uma linha, em vez de apresentar um relatório
parcialmente traduzido como se estivesse completo.

### `--no-color`

Desativa as cores ANSI mesmo em um terminal.

```sh
normfix --no-color
```

As cores já ficam desativadas quando a saída não é um terminal, ou quando
`NO_COLOR` está definido.

### `-v`, `--verbose`

Lista cada correção aceita em vez de apenas a contagem.

```sh
normfix --check -v
```

Útil quando você quer saber exatamente quais dezessete correções um arquivo
recebeu.

## Execução

### `--threads N`

Define a contagem de processos paralelos. O padrão é o hardware disponível.

```sh
normfix --threads 1
```

Use `1` para tornar a ordem da saída trivialmente reproduzível durante uma
depuração. Resultados e gravações são ordenados por caminho de qualquer forma,
então a contagem de processos nunca muda o relatório nem a ordem em que os
arquivos são gravados.

### `--timeout SECONDS`

Tempo limite da Norminette por arquivo. Padrão `5`.

```sh
normfix --timeout 15
```

Aumente em uma máquina lenta ou em um arquivo muito grande. Um tempo esgotado é
uma falha operacional daquele arquivo, não um diagnóstico.

### `--no-cache`

Desativa o cache externo de análise.

```sh
normfix --no-cache
```

O cache guarda resultados do verificador oficial fora do projeto, indexados pelos
bytes da fonte e pela impressão digital verificada do verificador. Desative-o
para forçar uma reexecução completa; uma falha de cache já falha aberta como uma
perda.

### `--norminette PATH`

Usa um executável exato da Norminette em vez de procurar no `PATH`.

```sh
normfix --norminette ~/.local/pipx/venvs/norminette/bin/norminette
```

A versão tem sua impressão digital registrada. A versão `3.3.59` é a testada;
outra versão analisável continua com um aviso destacado
`NORMINETTE_VERSION_UNTESTED`.

## Verificações do compilador

### `--strict-norminette-version`

Recusa uma versão da Norminette contra a qual esta versão não foi verificada.

```sh
normfix --strict-norminette-version
```

O padrão continua funcionando quando um campus instala uma versão oficial mais
nova, ainda nomeando a lacuna de compatibilidade. O modo estrito é útil para uma
CI reproduzível que fixa deliberadamente a `3.3.59`. A grafia anterior
`--allow-untested-norminette` permanece como um no-op oculto durante a transição
das versões candidatas.

### `--no-compiler-preflight`

Pula a passagem estrita `cc -fsyntax-only -Wall -Wextra -Werror`.

```sh
normfix --no-compiler-preflight
```

A passagem é ativa por padrão e é só de diagnóstico: ela nunca autoriza nem
rejeita uma edição do formatador. Pule-a quando seu projeto precisa de flags de
build que o contexto inferido não consegue fornecer, e o ruído não é útil.

### `--cc PATH`

Usa um compilador exato para a passagem estrita de sintaxe e para o analisador
profundo. O analisador é automático no `preflight`; fluxos comuns exigem
`--analyzer`.

```sh
normfix --cc /usr/bin/gcc-14
```

O compilador é identificado pelo banner de versão, então um comando chamado
`gcc` que na verdade é o Clang é tratado como Clang.

### `--analyzer`

Também roda o analisador estático profundo que seu compilador traz durante um
fluxo comum. O `preflight` já ativa essa passagem limitada automaticamente.

```sh
normfix --analyzer
```

O `normfix` escolhe as flags a partir do próprio banner de versão do compilador,
não do nome do comando:

| Compilador | O que roda |
|---|---|
| GCC | `-fanalyzer` |
| Clang | `--analyze -Xclang -analyzer-output=text` |
| Qualquer outro | Nada; a execução relata `CC_ANALYZER_UNAVAILABLE` e continua |

::: warning `/usr/bin/gcc` no macOS é o Clang
A Apple distribui um comando `gcc` que responde `Apple clang version ...`.
Escolhê-lo com `--cc` não lhe dá `-fanalyzer`. O `normfix` detecta isso e usa o
analisador do Clang, então a flag faz o que você quis dizer de qualquer forma.
:::

Os dois analisadores são mais lentos e informativos. Eles são automáticos no
`preflight` e opcionais no resto. Eles podem sugerir um vazamento ou um acesso
inválido ao longo de um caminho; nenhum é prova de qualquer um dos dois, e
nenhum é jamais prova da ausência deles. Um analisador ausente nunca muda o
status de saída.

Para um GCC de verdade no macOS, instale um e aponte para ele explicitamente:

```sh
brew install gcc
normfix preflight --cc "$(brew --prefix)/bin/gcc-14"
```

## Conteúdo que é reescrito

### `--no-reorder-includes`

Deixa os blocos contíguos de `#include` na ordem atual.

```sh
normfix --no-reorder-includes
```

Por padrão, uma sequência de diretivas de include é ordenada com os cabeçalhos de
sistema primeiro, depois os do projeto, em ordem alfabética dentro de cada um. Um
bloco só é reescrito enquanto cada linha dele for exatamente uma diretiva de
include, então um comentário ou um condicional encerra a sequência e nada a
atravessa.

### `--no-format-markdown`

Deixa os documentos README inalterados.

```sh
normfix --no-format-markdown
```

Arquivos README são reimpressos como CommonMark canônico por padrão. Isso pode
produzir um diff grande na primeira execução, que é o motivo usual para
desligar.

O documento é lido no dialeto em que foi escrito, então listas de tarefas,
notas de rodapé, tabelas e texto riscado voltam como eles mesmos. Lidos como
CommonMark puro, seriam texto comum, e a reimpressão escaparia os colchetes:
`- [x] feito` voltaria como `- \[x\] feito` literal.

## Operações destrutivas

Cada uma destas apaga ou move algo. Todas mantêm armazenamento externo
recuperável, e todas exigem confirmação.

### `--remove-invalid-comments`

Apaga apenas os comentários que o verificador oficial rejeitou em localizações
exatas.

```sh
normfix --remove-invalid-comments
```

Nada mais é tocado: um comentário que o verificador aceita nunca é removido.

### `--remove-unused`

Remove funções `static` comprovadamente inalcançáveis no projeto completo.

```sh
normfix --remove-unused
```

A prova precisa que toda fonte do projeto seja legível e inequívoca. Um único
arquivo ilegível desativa a análise inteira, em vez de produzir uma resposta
parcial.

### `--remove-unexpected`

Move arquivos regulares inesperados para a quarentena externa.

```sh
normfix --remove-unexpected
```

Nada é apagado: os arquivos são movidos para o armazenamento de recuperação com
o caminho relativo preservado, e um destino existente nunca é sobrescrito.

### `--unsafe`

Ativa o conjunto fechado acima, mais a compactação de comparações com NULL, a
remoção de fontes obsoletas do Makefile e a exclusão de uma variável local que
nada lê.

Essa última é recusada sempre que a declaração guarda algo que executa.
`int n = g();` é uma chamada, e um `malloc` ali teria o vazamento consertado
por acidente — virando um programa que você não escreveu. Esses casos são
reportados.

```sh
normfix --unsafe
```

É um conjunto nomeado, não um interruptor aberto. Ele não pode ativar uma
operação que já não exista como uma flag própria.

### `--force`

Confirma operações destrutivas sem um prompt, ou reconhece explicitamente um
escopo protegido do sistema/amplo.

```sh
normfix --unsafe --force
```

Para CI e scripts. O `--force` sozinho, sem nenhuma flag destrutiva, é um erro, a
menos que o escopo selecionado seja protegido. Reconhecer um escopo protegido não
cria nenhuma capacidade destrutiva; essas continuam exigindo suas próprias
flags.

## Ambiente

### `NORMFIX_NO_UPDATE_CHECK`

Desativa o aviso diário de release.

```sh
export NORMFIX_NO_UPDATE_CHECK=1
```

O aviso só aparece para saída humana interativa e é silencioso em caso de falha.
Veja [`upgrade`](/pt/commands/upgrade) para saber exatamente o que ele envia.

## Informação

### `-h`, `--help`

```sh
normfix --help
normfix undo --help
```

### `-V`, `--version`

```sh
normfix --version
```
