# Usando o normfix a partir de um agente de IA

Esta página é o contrato operacional para agentes de código, agentes de editor,
bots de CI e outros chamadores não humanos. Ela impede que um agente
acidentalmente transforme uma verificação de status em uma gravação recursiva.

## A única regra para lembrar

O comando puro formata o diretório atual recursivamente:

```sh
normfix
```

Um agente deve, portanto, começar com um caminho explícito e um comando somente
leitura:

```sh
normfix check /caminho/absoluto/para/o/projeto --format json --no-color
```

Use um caminho absoluto do projeto. Não confie em um diretório de trabalho
herdado, especialmente quando o agente pode ter iniciado em um diretório
pessoal, no diretório pai de um clone, na raiz de um workspace montado ou em um
diretório de sistema.

## Verificação de capacidades

Antes da primeira execução em um projeto, registre as versões da ferramenta e do
verificador:

```sh
normfix --version
norminette --version
normfix --help
```

O `normfix` registra a impressão digital de todo verificador. Quando a 42 publica
uma versão diferente, a execução padrão continua e emite
`NORMINETTE_VERSION_UNTESTED`; um agente precisa expor essa garantia reduzida.
Use `--strict-norminette-version` apenas quando a pessoa usuária ou a política de
CI exigir explicitamente a versão testada do verificador.

Na inicialização, o modo humano escreve um bloco de ação/configuração sem cor no
`stderr`. O modo JSON escreve um evento JSON `execution_start` no `stderr` e
mantém o relatório final versionado como o único documento JSON no `stdout`.
Nenhum dos modos pergunta nada quando o stdin não é interativo.

Leia o escopo desse evento antes de fazer qualquer coisa com o resultado. Ele é a
declaração da própria execução sobre o que ela estava prestes a tocar, então um
agente pode abortar uma execução cujo escopo não corresponde à tarefa recebida,
em vez de descobrir a divergência no resumo.

Um escopo amplo ou sensível do sistema operacional é recusado antes que qualquer
arquivo seja lido:

```console
$ normfix check /
normfix
error: refusing to scan or modify protected scope `/` because it is a filesystem root; inspect the path and pass --force to acknowledge it explicitly
No unvalidated changes were written.
```

Isso é saída `2` sem relatório JSON no `stdout`. Raízes do sistema de arquivos,
diretórios pessoais completos, árvores do sistema operacional e diretórios amplos
com vários projetos recusam dessa forma, e a verificação resolve links
simbólicos e `..` antes. Não adicione `--force` para fazer a mensagem sumir: a
recusa quase sempre significa que o escopo foi calculado errado, e o `--force` é
uma decisão da pessoa usuária sobre um caminho que ela inspecionou.

O formatador comum não precisa de Rust. Um compilador é usado apenas para
verificações consultivas de preflight; os achados dele nunca autorizam uma
edição.

## Fluxo recomendado para agentes

1. Inspecione o estado do repositório e resolva qualquer conflito de merge antes
   de formatar.
2. Rode uma pré-visualização legível por máquina contra um escopo explícito.
3. Leia o `schema_version` antes de consumir campos do relatório JSON.
4. Mostre à pessoa usuária os arquivos propostos, os diagnósticos restantes e
   quaisquer falhas operacionais.
5. Se as gravações já estiverem autorizadas, rode o mesmo escopo explícito com
   `normfix format`.
6. Inspecione o diff resultante e rode o build/testes do próprio projeto.
7. Rode `normfix check` de novo. Uma segunda passagem bem-sucedida não deve
   propor nenhuma edição.

```sh
project=/caminho/absoluto/para/o/projeto
normfix check "$project" --format json --no-color > normfix-report.json
normfix format "$project" --no-color
git -C "$project" diff --check
normfix check "$project" --format json --no-color
```

Não crie `normfix-report.json` dentro de um diretório de entrega da 42, a menos
que a pessoa usuária queira isso: um arquivo inesperado é, por si só, um achado
de avaliação. Use um diretório de saída temporário ou do próprio agente.

## Lendo o contrato JSON

O relatório estável usa atualmente `schema_version: 2`. Campos úteis são:

| Campo | Decisão do agente |
|---|---|
| `summary.changed` | Uma pré-visualização encontrou mudanças de bytes que consegue provar seguras |
| `summary.remaining` | Restam achados manuais/bloqueantes |
| `summary.failed` | Uma operação de ferramenta, descoberta, E/S ou transação falhou |
| `summary.unexpected_files` | Foram encontrados arquivos fora do conjunto aceito de arquivos de projeto |
| `files[].failure` | Este arquivo não foi concluído; não o descreva como corrigido |
| `files[].after` | Diagnósticos contra o buffer sombra final |
| `files[].fixes` | Edições comprovadas propostas ou gravadas para aquele arquivo |
| `identity.available` | Um cabeçalho oficial da 42 pode ser criado ou atualizado |
| `evaluation.conclusive` | Sempre `false`; nunca apresente a estimativa como nota oficial |
| `evaluation.verdict` | `hard_fail` significa que uma regra objetiva de rejeição do preflight foi atendida |
| `evaluation.hard_failures` | Evidência exata de caminho/linha/coluna/regra para expor primeiro |

Buffers de fonte e diffs estão intencionalmente ausentes do JSON. Use
`normfix --diff /caminho/absoluto` quando um patch legível por humanos for
necessário.

O status de saída faz parte da API:

| Código | Significado |
|---:|---|
| `0` | Limpo, ou uma gravação concluída sem problema bloqueante |
| `1` | Uma pré-visualização encontrou trabalho, ou resta um achado manual |
| `2` | A própria execução falhou |
| `130` | Uma pessoa cancelou a revisão interativa |

A saída `1` não é uma falha operacional. A saída `2` nunca pode ser escondida
atrás da afirmação de que o projeto passou.

## Escolhendo um comando

| Objetivo | Comando |
|---|---|
| Pré-visualização exata | `normfix --diff PATH` |
| Portão de máquina | `normfix check PATH --format json --no-color` |
| Diagnosticar os bytes sem editar | `normfix lint PATH --format json --no-color` |
| Revisão pré-defesa | `normfix preflight PATH --format json --no-color` |
| Folga das funções | `normfix budget PATH --format json --no-color` |
| Explicar uma regra offline | `normfix explain RULE` |
| Formatar um escopo autorizado | `normfix format PATH --no-color` |
| Restaurar uma transação do normfix | `normfix undo --list`, depois `normfix undo --run ID` |

`--changed` e `--staged` são convenientes para a árvore de trabalho de quem
desenvolve, mas selecionam nomes através do Git e analisam os bytes da árvore de
trabalho. Use um caminho explícito para uma avaliação completa e um escopo do
Git para uma edição focada.

## Autoridade e flags destrutivas

Estas opções pedem capacidades materialmente diferentes:

- `--remove-invalid-comments` apaga apenas comentários rejeitados em localizações
  oficiais exatas;
- `--remove-unused` remove apenas funções `static` inalcançáveis sob uma prova
  fechada de projeto;
- `--remove-unexpected` move arquivos para uma quarentena externa recuperável;
- `--unsafe` ativa o conjunto fechado e documentado de limpezas destrutivas;
- `--force` fornece confirmação não interativa para essas capacidades.

Um agente não pode inferir permissão para elas a partir de um pedido de
verificar, formatar, avaliar ou "corrigir erros da Norm". Pré-visualizar um plano
destrutivo também exige a capacidade, porque a análise é intencionalmente
condicionada à autorização.

Nunca apague dados de backup ou de quarentena para fazer um relatório parecer
limpo. Use `normfix undo` para recuperação, e informe o caminho do journal se o
rollback precisar de revisão manual.

## Limites da avaliação

O `preflight` combina o resultado oficial da Norm, verificações de arquivos de
projeto, diagnósticos estritos do compilador, verificações de política e uma
passagem automática e limitada do analisador do compilador. É um forte auxílio de
revisão, não uma nota conclusiva da 42. Ele não conhece o PDF do assunto, não
executa uma lista de verificação de defesa, não prova correção algorítmica e não
prova a ausência de vazamentos. Ele não executa receitas do Make, um binário
produzido, o `clang-tidy` nem sanitizers. Rode o Makefile do próprio projeto, os
testes, o build com sanitizer e o testador específico do assunto separadamente,
e somente quando a pessoa usuária autorizar a execução daquele projeto.

Não trate a presença ou ausência de um README como uma regra universal de
aprovação/reprovação. Quando existir um, verifique-o contra as seções exigidas
pelo assunto atual. Da mesma forma, `MAKEFILE_NOT_FOUND` é consultivo até que a
política do assunto prove que um Makefile é exigido. Não relate uma correção
proposta na sombra como aprovação do preflight: a avaliação reprova nos achados
originais em disco da Norminette e do Makefile.

## Higiene de terminal e CI

- Prefira `--format json --no-color` para analisadores e saída redirecionada.
- Nunca analise a tabela humana decorativa quando o JSON estiver disponível.
- Defina `NORMFIX_NO_UPDATE_CHECK=1` em CI hermética ou sem rede.
- Mantenha as versões do verificador oficial e do `normfix` nos logs da CI.
- Não canalize um comando de gravação por um filtro que esconda o status de
  saída.
- Não rode contra `/`, `/System`, `/usr`, `/etc`, um diretório pessoal ou um
  workspace contendo vários projetos. Selecione a raiz real da entrega.

Para cada opção e limite de prova, continue em
[Todas as flags](/pt/reference/flags),
[Segurança e recuperação](/pt/reference/safety),
[Relatórios](/pt/reference/reporting) e [Arquitetura](/ARCHITECTURE).
