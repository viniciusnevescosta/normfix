# Política de compatibilidade

Este documento define o que o `normfix` considera suportado. Ele é
intencionalmente estreito: afirmações de compatibilidade fazem parte do modelo de
segurança e precisam ser sustentadas por evidência automatizada.

## Norminette oficial

O verificador testado é a
[Norminette oficial](https://github.com/42School/norminette) `3.3.59`.

O `normfix` registra a impressão digital da versão do executável antes da
análise. Uma versão diferente continua, por padrão, com um aviso destacado
`NORMINETTE_VERSION_UNTESTED`; o `--strict-norminette-version` a recusa numa CI
com versão fixada. Isso não é uma afirmação de compatibilidade com versão
mínima, porque os nomes dos diagnósticos oficiais, as localizações, o
comportamento do parser e os layouts aceitos são entradas da camada nativa de
compatibilidade. O aviso torna essa garantia reduzida explícita.

A Norminette continua sendo uma dependência externa. Os arquivos de release
contêm o binário nativo do `normfix`, não o Python nem o verificador oficial.

### Adotando outra versão do verificador

Uma atualização da Norminette exige uma mudança revisada que:

1. registre as notas de versão upstream e as mudanças de nomes de regras;
2. rode a suíte nativa completa contra a versão candidata;
3. atualize as fixtures de saída oficial somente depois de explicar cada
   diferença;
4. verifique a idempotência das correções seguras e a ausência de regressão em
   projetos representativos da 42;
5. atualize a constante exata da versão, a instalação na CI, o README e este
   arquivo;
6. seja publicada como uma nova versão do `normfix`.

Suportar uma faixa de versões só é adequado depois que a CI provar cada versão
dentro dela e o oráculo tiver um adaptador explícito para qualquer diferença de
protocolo.

### Quando a 42 se move primeiro

Uma ferramenta que recusa todas as versões menos uma para de funcionar para todo
mundo no dia em que a escola atualiza. Por isso o padrão é continuar e reportar
`NORMINETTE_VERSION_UNTESTED`; uma CI fixada pode optar pela recusa:

```sh
normfix --strict-norminette-version
```

O comportamento padrão é defensável, e não um buraco no argumento, porque a
propriedade que a ferramenta realmente promete não depende de saber a versão: a
prova de regressão antes/depois compara duas respostas do **mesmo executável**,
então uma execução continua não podendo deixar um arquivo com mais diagnósticos
oficiais do que ele tinha no começo. O que uma versão não verificada custa é a
garantia de que as regras nativas concordam com ela — que é exatamente o que o
aviso diz.

## Toolchain do Rust

- Versão mínima suportada do [Rust](https://www.rust-lang.org/tools/install)
  (MSRV): `1.85`.
- Toolchain do repositório e das releases: `1.97.1`, fixada em
  `rust-toolchain.toml`.

A CI verifica a MSRV de forma independente da toolchain de desenvolvimento
fixada. Elevar a MSRV exige uma mudança de release documentada, e não uma
atualização incidental de dependência.

## Sistemas operacionais e alvos de release

As releases pré-compiladas cobrem os ambientes Unix usados por estudantes da 42:

| Sistema operacional | Arquitetura | Arquivo público da release |
|---|---|---|
| Linux | x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux | ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS | Intel | `normfix-x86_64-macos.tar.gz` |
| macOS | Apple Silicon | `normfix-aarch64-macos.tar.gz` |
| Windows | x86-64 | `normfix-x86_64-windows.zip` |
| Windows | ARM64 | `normfix-aarch64-windows.zip` |
| FreeBSD | x86-64 | `normfix-x86_64-freebsd.tar.gz` |

Os nomes públicos dos arquivos omitem deliberadamente os marcadores de fornecedor
do Rust e os rótulos de fabricante da máquina. Os identificadores de alvo da
toolchain continuam sendo entradas internas de build, não nomes de release nem de
produto.

O Windows passou a ser suportado nativamente na 1.4.0, com base na evidência que
a CI produz para ele, e não na suposição de que código portátil porta. Os dois
alvos de Windows rodam a suíte completa, dirigem a Norminette oficial de verdade
e provam a propriedade diferencial — que uma execução nunca deixa um arquivo com
mais diagnósticos oficiais do que ele tinha no começo — na própria plataforma.

Duas diferenças em relação ao Unix são reais, e ficam ditas aqui em vez de
alisadas:

- **A contenção de processos tem uma janela estreita.** O Unix coloca a
  ferramenta no próprio grupo de processos entre o fork e o exec, então nenhum
  descendente escapa. O Windows não tem gancho pré-início: a ferramenta entra num
  job object logo depois do spawn, e o que ela criar nos microssegundos até lá
  poderia se desprender. O job mata o resto da árvore ao fechar.
- **Um rename não é write-through.** O POSIX exige sincronizar o diretório-pai
  para uma criação ou renomeação sobreviver a uma queda, e a transação faz isso.
  O Windows não tem contrapartida; o conteúdo do arquivo é sincronizado e o NTFS
  registra os metadados, mas uma máquina que perde energia entre o commit e os
  metadados chegarem ao disco tem garantia mais fraca que o mesmo instante no
  Unix. O backup e o journal não são afetados: a recuperação os lê por conteúdo,
  não por ordem.

Os arquivos de Windows são `.zip`, que a plataforma abre sozinha. O instalador de
uma linha funciona lá em qualquer shell POSIX — Git Bash, MSYS2, Cygwin ou WSL.
Rodar o build de Linux dentro do WSL continua suportado e inalterado.

O FreeBSD x86-64 é suportado nos mesmos termos. Ele é um Unix, então compartilha
a contenção por grupo de processos e a sincronização de diretório em vez de
precisar dos substitutos do Windows, e a CI roda a suíte completa, o verificador
oficial e a prova diferencial dentro de uma máquina virtual FreeBSD — o GitHub
não tem runner de FreeBSD, e compilar cruzado publicaria um binário que nunca
rodou no sistema a que se destina. O arquivo de release dele é construído nessa
mesma máquina virtual, pelo mesmo motivo.

O FreeBSD em ARM64 não é publicado. O `aarch64-unknown-freebsd` não tem
biblioteca padrão pré-compilada na toolchain fixada, então construí-lo exigiria
um compilador nightly sem fixação, e não há como rodar a suíte nele. Qualquer um
dos dois já bastaria para tornar a afirmação insustentável.

## Diagnósticos de C e de build

A Norminette oficial é a autoridade de compatibilidade de estilo. Um compilador C
do sistema roda por padrão como um oráculo separado, apenas de diagnósticos, para
`-fsyntax-only -Wall -Wextra -Werror`. Os caminhos de include inferidos a partir
dos diretórios de headers não substituem as flags do Makefile do projeto, seus
defines, entradas geradas, modo de linguagem, entradas do linker ou testes de
execução.

O `-fanalyzer` do GCC é automático no `preflight` e opcional nos fluxos comuns.
Seus achados sobre tempo de vida de alocações e fluxo de controle podem sugerir um
possível vazamento ou acesso inválido, mas não são prova de que um comportamento
arbitrário em C está correto nem de que um projeto está livre de vazamentos.

O `normfix preflight` não executa receitas do Make, não linka um binário e não
roda o programa nem os testes. Ele reporta explicitamente esses passos manuais
restantes.

O `normfix leaks` executa um programa, e é o único comando que faz isso. Ele
nunca constrói um — roda um binário para o qual é apontado, sob um verificador
de vazamentos localizado no `PATH` e verificado pelo próprio `--version`. O que
ele relata é o que uma execução observou em um caminho, nunca prova de que o
programa não vaza, e uma saída que ele não consegue ler como sumário de
vazamentos é erro, não resultado limpo. O Valgrind cobre Linux e FreeBSD
diretamente e o Windows pelo WSL. Ports comunitários nativos do macOS são
recusados para resultados limpos depois que um teste real mostrou que um deles
podia deixar passar um leak C conhecido.

## Compatibilidade com navegadores

O playground tem como alvo navegadores modernos com suporte padrão a WebAssembly
e a módulos ES. Sua interface HTML/CSS/TypeScript deliberadamente pequena e à
moda antiga é construída como site estático com o
[Vite 8.2.1](https://vite.dev/releases) fixado, e pode ser servida localmente ou
pela Vercel. Seu contrato de compatibilidade é o subconjunto nativo de
formatação e diagnóstico em memória descrito em
[`web/README.md`](https://github.com/viniciusnevescosta/normfix/blob/main/web/README.md).
Ele consegue montar um cabeçalho oficial a partir de uma identidade informada
àquela aba do navegador, e consegue pré-visualizar C, headers, Makefiles e
Markdown. Ele não embute nem emula a Norminette, um compilador, o Git, provas de
guarda de header para o projeto inteiro ou transações no sistema de arquivos.

## Compatibilidade do relatório

A interface humana agrupa diagnósticos para facilitar a leitura e pode melhorar
entre versões. Automação deve usar `--format json` e verificar o
`schema_version`; o JSON preserva os achados individuais. Uma estrutura JSON
incompatível exige um incremento da versão do schema e notas de compatibilidade.

Vale dizer uma consequência de forma direta: a linha e a coluna impressas ao lado
de um trecho seguem a convenção do compilador C e contam caracteres, enquanto a
Norminette oficial conta colunas de exibição. Os dois discordam numa linha
indentada com tabulação. Nenhum dos dois números faz parte da superfície
versionada, e o que localiza o achado é o acento circunflexo sob o código. Veja
[Relatórios](/pt/reference/reporting#lendo-um-diagnóstico).

## O que o versionamento cobre

O `normfix` segue o Versionamento Semântico. O número da versão descreve as
superfícies abaixo, e somente elas:

| Superfície | Coberta | O que significa uma quebra |
|---|---|---|
| Flags e subcomandos da linha de comando | sim | Remover ou renomear um, ou mudar o que um existente faz |
| Códigos de saída | sim | Mudar o significado de `0`, `1`, `2` ou `130` |
| Estrutura do relatório JSON | sim, via `schema_version` | Remover um campo ou mudar seu tipo |
| Arquivos de configuração (`normfix.toml`, `.normfixignore`) | sim | Mudar como uma chave ou padrão existente é interpretado |
| Layout de backup, journal e quarentena | sim | Tornar um ponto de recuperação antigo ilegível para o `undo` |
| Quais códigos são editados automaticamente | não | Novas edições provadas chegam em versões minor |
| Texto, agrupamento e ajuda dos diagnósticos | não | Melhorados continuamente |
| APIs das crates Rust | não | Toda crate define `publish = false` e é interna |
| A versão suportada da Norminette | à parte | Mudá-la é uma alteração de release documentada, nunca incidental |

Uma nova edição automática é uma versão minor, porque um formatador cuja saída
nunca mudasse não valeria a pena executar. Uma execução que produz um resultado
oficial *pior* é um bug em qualquer versão, e o teste diferencial existe para
pegar exatamente isso.

A versão mínima suportada do Rust é uma decisão de release, não um detalhe de
build. Elevá-la exige uma mudança documentada; uma dependência que precise de um
compilador mais novo fica para trás.
