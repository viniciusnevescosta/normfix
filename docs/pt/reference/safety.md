# Segurança, recuperação e operações destrutivas

## Toda execução diz o que vai fazer

Antes de ler um único arquivo, o `normfix` imprime a ação, o escopo resolvido e a
configuração de segurança que está realmente em vigor:

```console
$ normfix --unsafe --force
normfix · starting
  action       format
  mode         write
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
  destructive  invalid comments, NULL-check compaction, missing or trivia-only Makefile entries, orphan header prototypes, unreachable static functions, unexpected-file quarantine
  force        acknowledged
```

A linha `destructive` nomeia cada capacidade que a execução realmente detém,
então `--unsafe` nunca se amplia em silêncio.

A linha `scope` é a que você deve ler. Um comando digitado no diretório errado
parece errado aqui, antes de qualquer coisa ser tocada, em vez de aparecer no
resumo depois. Com `--format json`, essa mesma informação é o primeiro evento na
saída padrão, então um agente pode recusar uma execução cujo escopo ele não
pretendia.

## Escopos protegidos

Raízes do sistema de arquivos, diretórios pessoais completos, árvores do sistema
operacional e diretórios amplos com vários projetos são recusados de imediato:

```console
$ normfix check /
normfix
error: refusing to scan or modify protected scope `/` because it is a filesystem root; inspect the path and pass --force to acknowledge it explicitly
No unvalidated changes were written.

$ normfix check ~
normfix
error: refusing to scan or modify protected scope `/home/student` because it is the complete user home directory; inspect the path and pass --force to acknowledge it explicitly
No unvalidated changes were written.
```

Os dois terminam com status `2` e não leem nada. A verificação resolve links
simbólicos e colapsa `..` primeiro, então um caminho como `/work/../etc` ou um
link apontando para `/etc` é recusado pelo mesmo motivo que um `/etc` literal
seria. Uma execução com escopo de Git é julgada pela raiz do repositório, e não
pelos arquivos que ela seleciona, então `--git-changed` a partir de um diretório
pessoal é recusado em vez de percorrer silenciosamente todos os projetos dentro
dele.

O `--force` reconhece um escopo protegido, e nada além disso. Ele não concede
uma capacidade destrutiva por conta própria, e uma capacidade destrutiva
continua exigindo a própria flag:

```console
$ normfix --force
normfix
error: --force requires --unsafe, --remove-unused, --remove-unexpected, or a protected system scope
```

## Listas de funções permitidas

Projetos com uma lista de funções permitidas específica do subject podem
adicionar um `normfix.toml` na raiz do projeto:

```toml
[project]
name = "get_next_line"
allowed = ["read", "malloc", "free"]
```

O parser limitado interpreta intencionalmente apenas o `name` entre aspas e o
array `allowed` de identificadores entre aspas. Quando um escopo de C/headers é
selecionado, o `normfix` descobre por conta própria o conjunto completo de
arquivos C/header do projeto a partir da raiz, considerando arquivos regulares
sem seguir links simbólicos e com os filtros de `.gitignore`, `.normfixignore` e
`.norminetteignore` desativados. Todo arquivo descoberto precisa ser UTF-8
legível e ser interpretado sem perdas. Definições não-`static` daquele instantâneo
fechado autorizam chamadas entre unidades de tradução; definições no mesmo
arquivo são tratadas localmente, enquanto uma definição `static` em outro arquivo
nunca autoriza a chamada.

As chamadas candidatas são recalculadas contra os bytes propostos, para que as
faixas reportadas continuem corretas depois da inserção do cabeçalho e da
formatação. Parâmetros, chamadas por ponteiro de função, ambiguidade de
macro/pré-processador e identificadores em maiúsculas com cara de macro falham
fechado, em vez de produzir um palpite. Se a descoberta, a leitura, a
interpretação, a ausência de perdas ou a revalidação do instantâneo ficar
incompleta, todos os achados da lista permitida são desativados e o
`FUNCTION_POLICY_PROOF_INCOMPLETE` explica por quê. O próprio `normfix.toml`
precisa ser um arquivo regular limitado e não um link simbólico. A política
continua não substituindo o subject do projeto nem o avaliador.

## Comentários e capacidades destrutivas

Comentários rejeitados como `WRONG_SCOPE_COMMENT` ou `COMMENT_ON_INSTR` são
apenas reportados por padrão. O `--remove-invalid-comments` apaga somente um
comentário encontrado exatamente na linha e na coluna de exibição reportadas pelo
verificador oficial. Ele nunca remove o cabeçalho oficial, e a impressão digital
dos tokens de código restantes precisa continuar inalterada.

O `--unsafe` também apaga uma variável local que nada lê, e a prova
deliberadamente não é a do compilador. O `-Wunused-variable` dispara para
`int n = g();` exatamente como dispara para `int n;`, e apagar o primeiro apaga
uma chamada — uma declaração com um `malloc` teria o vazamento consertado por
acidente, virando um programa que você não escreveu. Esses ficam e são
reportados. Um nome qualifica quando aparece exatamente uma vez no arquivo
inteiro, contado no texto cru, porque um corpo de macro que cita o nome é texto
que nenhuma árvore de análise mostra.

O `--remove-unused` e o `--remove-unexpected` pedem capacidades destrutivas mais
fortes:

- a remoção de funções não utilizadas considera apenas definições `static`;
- ela exige que as entradas selecionadas sejam iguais ao conjunto completo de
  `.c`/`.h` do projeto;
- recuperação do parser, bytes desconhecidos, ambiguidade de pré-processador,
  colagem de tokens, atributos, referências baseadas em strings, definições
  duplicadas ou um grafo de referências incerto preservam a função;
- a remoção de arquivos inesperados é uma operação de quarentena recuperável,
  nunca uma exclusão permanente baseada em extensão.

Numa execução humana e interativa, essas capacidades exigem uma confirmação
`y/N` antes da análise. O prompt concede apenas a capacidade pedida; cada
candidato ainda precisa passar pelas provas de parser, hash, escopo e transação.
Responder que sim não enfraquece nenhuma prova.

Execuções em JSON e outras não interativas exigem `--force`:

```sh
normfix --remove-unused --force
normfix --remove-unexpected --force
normfix --unsafe --force
```

O `--unsafe` é um atalho fechado para seis operações implementadas:

- remoção de comentário inválido em localização exata;
- compactação de comparações simples com `NULL` apenas quando a forma dedicada
  em C está provada;
- remoção de tokens comprovadamente ausentes ou compostos só de trivia em listas
  literais simples de fontes do Makefile;
- remoção de protótipos de headers locais do projeto apenas quando uma prova
  completa e sem perdas do código não encontra nem implementação nem qualquer
  uso ou ambiguidade;
- remoção de `static` inalcançável sob uma prova de código fechado;
- quarentena de arquivos inesperados.

Os avisos sobre implementação de protótipos, em si, já ficam ativos em execuções
normais. O modo inseguro pode remover uma declaração ausente e não utilizada
depois da prova completa; ele nunca remove uma definição existente composta só de
trivia nem o protótipo dela, porque um corpo vazio pode ser intencional.

Ele não habilita edições arbitrárias. A remoção de comentários também pode ser
pedida sozinha com `--remove-invalid-comments`; os demais planos destrutivos
continuam exigindo autorização de capacidade.

Use o modo de prévia antes de uma execução destrutiva:

```sh
normfix --diff --remove-unused
normfix --check --remove-unexpected
```

Os modos de prévia exigem a mesma autorização interativa, porque os próprios
planejadores de mundo fechado são protegidos por capacidade, mas eles não
escrevem, não apagam e não movem arquivos do projeto.

## Backups, transações e recuperação

Os backups padrão do código ficam fora do projeto analisado:

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
```

Em Unix sem `XDG_DATA_HOME`, o caminho alternativo é:

```text
~/.local/share/normfix/backups/<run-id>/
```

Cada transação com backup inclui os bytes originais exatos e um `journal.json`.
Antes de o primeiro alvo mudar, o escritor:

- canonicaliza o limite do projeto;
- rejeita alvos duplicados, externos, links simbólicos e não regulares;
- confirma que cada arquivo atual ainda corresponde aos bytes analisados;
- grava os backups externos;
- prepara e sincroniza cada substituição.

Os alvos são efetivados na ordem dos caminhos. Um erro no meio da efetivação
dispara um rollback de melhor esforço a partir dos bytes originais capturados; um
rollback incompleto é reportado junto com o caminho do journal de recuperação.

O `--no-backup` vale apenas para a formatação segura comum. Uma exclusão de
código planejada pela remoção de comentários inválidos, pela reconciliação de
fontes do Makefile, pela remoção de protótipos órfãos ou pela remoção de `static`
inalcançável exige armazenamento externo de recuperação e falha fechado se ele
não estiver disponível.

A quarentena sempre mantém uma cópia externa recuperável, inclusive quando o
`--no-backup` foi informado:

```text
<backup-base>/quarantine/<run-id>/<original-relative-path>
```

O tipo do arquivo, o tamanho em bytes e o hash BLAKE3 são reconferidos
imediatamente antes da movimentação. Destinos de recuperação já existentes nunca
são sobrescritos. Uma falha parcial de quarentena tenta restaurar os arquivos que
já foram movidos.
