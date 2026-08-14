# O que é o normfix, e por quê

## Para que serve

O recurso mais escasso de um estudante da 42 é tempo. Não talento, não esforço.
Horas. E uma parcela significativa dessas horas vai para espaço em branco:
consertar indentação, mover declarações, quebrar linhas em 80 colunas, colar
cabeçalhos. Ao longo de um cursus que tem milhares de arquivos, projeto após
projeto, e nada disso te ensina nada na segunda vez que você faz.

O `normfix` existe para devolver essas horas. Ele corrige, em um comando e no
projeto inteiro, os erros que são mecânicos, e se recusa a tocar nos que são
sobre o seu programa de verdade — porque esses são os que valem o seu tempo.

## Em um parágrafo

Você escreve C para um projeto da 42. A
[Norminette oficial](https://github.com/42School/norminette) diz que a linha 47
tem a indentação errada, que uma função é longa demais, que uma declaração está
no lugar errado, e então para, porque relatar é tudo o que ela faz.
O `normfix` lê o mesmo projeto, corrige os erros que consegue provar que é seguro
corrigir, e explica o resto com uma frase, em vez de um nome de regra. É um comando
que deixa o seu projeto mais perto de passar do que estava, ou te diz
exatamente por que não conseguiu.

```sh
cd caminho/para/um/projeto-42
normfix
```

Essa é a interface inteira. Nenhum arquivo de configuração é obrigatório, nada é
enviado para lugar nenhum, e todo arquivo que ele reescreve tem backup fora do
projeto antes.

## O problema

A Norm da 42 é um padrão de layout: tabulações reais, 80 colunas, uma declaração
por linha, uma linha em branco depois do bloco de declarações, 25 linhas por
função, cinco funções por arquivo, um cabeçalho oficial no topo de cada arquivo.
Nada disso é difícil. Tudo isso é tedioso, e tudo isso é verificado por uma
ferramenta que só sabe dizer *não*.

Então, na véspera de uma defesa, você está fazendo uma de duas coisas: editando
espaço em branco à mão em quarenta arquivos, ou rodando um formatador genérico e
torcendo. As duas terminam mal. A primeira é lenta e você vai deixar passar algo.
A segunda é pior, porque um formatador que não conhece a Norm vai produzir com
confiança um código que a Norminette rejeita, e vai reescrever seu arquivo
inteiro para isso, então você não consegue distinguir o que ele mudou do que você
escreveu.

## O que o normfix faz de diferente

**Ele usa o verificador oficial como autoridade.** A Norminette instalada roda
antes e depois de cada lote de edições. Se um lote introduz uma violação de regra
que não existia antes, o lote inteiro é revertido e os seus bytes originais
permanecem. A 3.3.59 é a versão contra a qual ele foi testado; outra versão
instalada continua funcionando, mas aparece num aviso bem visível, porque as
regras nativas não passaram pela mesma checagem. O `normfix`
nunca discute com a ferramenta pela qual você é de fato avaliado.

**Ele mexe em trechos exatos, não em arquivos inteiros.** Uma mudança toca só o
pedaço sobre o qual ele provou alguma coisa; o resto do arquivo fica igualzinho,
byte por byte. Por isso dá para rodar no meio do trabalho, com o diff ainda
legível.

**Ele recusa mais do que aceita.** Reordenar includes que atravessam um `#ifdef`
poderia mudar quais declarações existem, então ele para no condicional. Tirar uma
função de um corpo de 40 linhas exige dar um nome pra ela, e o nome é você quem
escolhe — então ele avisa o tamanho e deixa a decisão com você. Toda recusa vem
com o motivo e o próximo passo.

**Tudo o que ele grava é recuperável.** As gravações passam por uma única
transação com backups externos e um journal. O `normfix undo` restaura uma
execução, e se recusa a fazer isso se você mexeu nesses arquivos depois.

## O que ele não vai fazer

Esta é a lista honesta, e ela é o propósito da ferramenta, não uma limitação da
versão de agora:

- Ele não vai extrair uma função longa por você.
- Ele não vai redesenhar fluxo de controle, renomear no projeto inteiro nem
  mudar uma assinatura pública.
- Ele não vai provar que seu programa não tem vazamentos. O analisador pode
  apontar um vazamento provável; ele não consegue provar que não existe nenhum.
- Ele não vai chamar uma versão não testada da Norminette de "suportada". Ele
  continua com um aviso visível de compatibilidade para que uma atualização da 42
  não torne a ferramenta inutilizável, e `--strict-norminette-version` é como você
  pede que ele recuse em vez de continuar.
- Ele não vai garantir 80 colunas quando não existe uma quebra segura. Uma string
  longa ou uma macro continua longa e é relatada.

## Onde ele se encaixa

| Momento | Comando |
|---|---|
| Enquanto escreve | `normfix --changed` no que você acabou de tocar |
| Antes de commitar | `normfix --check` como portão; o código de saída `1` significa que resta trabalho |
| Em uma revisão | `normfix lint --format json` para um diagnóstico sem edições |
| Antes de uma defesa | [`normfix preflight`](/pt/commands/preflight), que adiciona a passagem estrita do compilador |
| Depois de uma execução ruim | [`normfix undo`](/pt/commands/undo) |

## A regra sobre a qual ele foi construído

> Mude o que pode ser provado, explique o que não pode, e nunca transforme
> incerteza em permissão.

Toda decisão descrita [na arquitetura](/pt/ARCHITECTURE) sai dessa frase —
inclusive as que fazem a ferramenta fazer menos do que poderia.

## A seguir

- [Primeiros passos](/pt/guide/getting-started): instale e faça uma primeira
  execução reversível.
- [Comandos](/pt/commands/): uma página por subcomando, com saída real.
- [Todas as flags](/pt/reference/flags): o que cada uma faz, com um exemplo.
- [Playground no navegador](/pt/guide/playground): experimente o formatador sem
  instalar nada.
