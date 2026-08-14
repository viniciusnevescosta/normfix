# Primeiros passos

Ao fim desta página você vai ter o `normfix` instalado e vai ter rodado ele uma
vez num projeto de verdade, sem que ele mexa em nenhum arquivo.

## Instalando

Um comando só, no Linux, no macOS e no Windows:

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

Ele descobre em que máquina você está, baixa a versão certa, confere com as
somas de verificação publicadas e coloca o programa em `~/.local/bin`. Não pede
`sudo`, não escreve fora da sua pasta pessoal e não instala compilador nenhum —
é isso que faz ele funcionar num computador da 42, onde você não é
administrador.

No Windows, rode a partir de qualquer shell POSIX: Git Bash, MSYS2, Cygwin ou
WSL.

Confira se chegou:

```sh
normfix --version
```

Se o terminal não achar o comando, é porque `~/.local/bin` ainda não está no seu
`PATH`. Acrescente essa pasta no arquivo de inicialização do seu shell e abra um
terminal novo.

::: tip Ler antes de rodar
Jogar um script direto no shell é rodar código que você não leu. Se preferir
olhar antes, ele está aqui:
<https://normfix.vercel.app/install.sh>
:::

## Você também precisa da Norminette

Quem decide o que a Norm diz não é o `normfix` — é a Norminette oficial, e o
`normfix` pergunta pra ela. Então o verificador precisa estar instalado e no seu
`PATH`, senão uma execução não significa nada.

Instale pelo repositório dela, [42School/norminette][norminette]. Quem manda em
como ela é instalada é aquele projeto, e o README de lá é a única página que
continua certa quando isso muda.

[norminette]: https://github.com/42School/norminette

Depois confira se o `normfix` vai encontrar:

```sh
norminette --version
```

Uma instalação gerenciada pelo campus serve. O que importa é o comando rodar e
dizer a versão.

::: warning Se o seu campus atualizar a Norminette
A `3.3.59` é a versão contra a qual esta release foi testada. Outra continua
rodando, com um aviso, para que uma atualização do campus nunca deixe você sem a
ferramenta. Numa pipeline em que você quer a versão travada,
`--strict-norminette-version` recusa qualquer outra. A [política de
compatibilidade](/pt/COMPATIBILITY) explica o que esse aviso custa.
:::

## A primeira execução não muda nada

Entre num projeto e pergunte o que o `normfix` faria:

```sh
normfix --check
```

Isso lê seus arquivos e não escreve nenhum. Ele mostra o que consertaria, o que
não consegue consertar, e por quê.

Para ver as edições em si, em vez de um resumo:

```sh
normfix --diff
```

Quando estiver pronto:

```sh
normfix
```

Esse escreve. Antes disso, ele copia cada arquivo que vai tocar para uma pasta
de backup fora do seu projeto, para o `normfix undo` conseguir devolver.

## Outras formas de instalar

O instalador lá de cima é o que funciona em todo lugar. As outras estão aqui
porque talvez você já use alguma delas.

### Homebrew

```sh
brew install viniciusnevescosta/normfix/normfix
brew upgrade viniciusnevescosta/normfix/normfix  # depois
brew uninstall normfix                            # para remover
```

Instala o mesmo programa já verificado, em vez de compilar. Serve para macOS e
Linuxbrew.

### Scoop, no Windows sem um shell POSIX

```powershell
scoop bucket add normfix https://github.com/viniciusnevescosta/scoop-normfix
scoop install normfix
scoop update normfix     # depois, para atualizar
scoop uninstall normfix  # para remover
```

O Scoop é o dono dessa instalação, então o `normfix upgrade` e o `normfix uninstall`
se recusam e te mandam de volta para estes — trocar o binário por baixo deixaria
o manifesto do Scoop descrevendo algo que não está mais lá.

### Baixando o pacote você mesmo

Toda release publica um pacote por plataforma, com um arquivo `SHA256SUMS` ao
lado:

| Plataforma | Pacote |
|---|---|
| Linux x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS Intel | `normfix-x86_64-macos.tar.gz` |
| macOS Apple Silicon | `normfix-aarch64-macos.tar.gz` |
| Windows x86-64 | `normfix-x86_64-windows.zip` |
| Windows ARM64 | `normfix-aarch64-windows.zip` |
| FreeBSD x86-64 | `normfix-x86_64-freebsd.tar.gz` |

Num Apple Silicon, por exemplo:

```sh
version=1.6.2
archive="normfix-aarch64-macos.tar.gz"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/${archive}"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/SHA256SUMS"
grep " ${archive}$" SHA256SUMS | shasum -a 256 -c -
tar -xzf "${archive}"
install -m 0755 normfix "$HOME/.local/bin/normfix"
normfix --version
```

Crie a pasta `$HOME/.local/bin` antes, se ela não existir, e garanta que ela
está no seu `PATH`.

### Travando uma versão, ou escolhendo onde ele fica

```sh
NORMFIX_VERSION=v1.6.2 sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
NORMFIX_BIN_DIR=~/bin sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
```

`NORMFIX_VERSION` é literal: baixa aquela tag, sem escolher canal. Sem ela, você
recebe a release estável mais nova — ou, se ainda não houver nenhuma, a
pré-release mais nova, para um candidato a release continuar instalável.

Se uma soma de verificação não bater, a instalação para e mostra os dois
valores.

### Compilando do código-fonte

```sh
git clone https://github.com/viniciusnevescosta/normfix.git
cd normfix
cargo install --path crates/normfix-cli --locked
```

Ou compile sem instalar:

```sh
cargo build --release --locked -p normfix
./target/release/normfix --version
```

Esse é o único caminho que precisa do Rust — 1.85 ou mais novo. O Cargo instala
em `~/.cargo/bin`, então essa pasta precisa estar no seu `PATH`.

## Se o sistema disser que o desenvolvedor não pode ser verificado

O macOS e o Windows avisam sobre programas que não foram assinados com um
certificado pago de desenvolvedor. O `normfix` não é assinado, então esse aviso
pode aparecer — e o caminho que você escolheu decide se ele aparece.

O instalador de uma linha baixa com `curl`, que não põe a marca que dispara o
aviso; instalando assim, você nunca vai vê-lo. O navegador põe essa marca, então
baixar o pacote pela página de releases é o caso que avisa.

**No macOS**, a mensagem diz que o desenvolvedor não pôde ser verificado. Abra
uma vez pelo Finder com **botão direito → Abrir**, que oferece um botão que o
duplo clique não oferece. Ou tire a marca você mesmo:

```sh
xattr -d com.apple.quarantine ./normfix
```

**No Windows**, o SmartScreen diz que protegeu seu PC. Escolha **Mais
informações** e depois **Executar assim mesmo**. Pelo PowerShell:

```powershell
Unblock-File .\normfix.exe
```

Não assinar é uma decisão, não um descuido. Um certificado prova que alguém
pagou uma autoridade certificadora; não diz nada sobre o que tem dentro do
arquivo. Cada pacote aqui é publicado com sua soma de verificação e com a
procedência de compilação que o liga à execução exata que o gerou — uma
afirmação mais forte, que o sistema operacional simplesmente não olha:

```sh
gh attestation verify normfix-aarch64-macos.tar.gz --repo viniciusnevescosta/normfix
```

Se esse comando funcionar, o arquivo que você tem saiu da esteira de release
deste projeto, diga o seu sistema o que disser sobre a assinatura.

## Para onde ir agora

- [Linha de comando](/pt/guide/command-line) — os fluxos, as flags e o que cada
  código de saída quer dizer.
- [Playground no navegador](/pt/guide/playground) — experimente sem instalar
  nada.
- [Arquitetura](/pt/ARCHITECTURE) — como as peças se encaixam e por que as
  fronteiras estão onde estão.
