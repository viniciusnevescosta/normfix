# Primeiros passos

## Requisitos

- O comando da [Norminette oficial](https://github.com/42School/norminette)
  disponível no `PATH`, ou informado com `--norminette CAMINHO`. A versão
  `3.3.59` é a base de compatibilidade testada.
- [Rust](https://www.rust-lang.org/tools/install) 1.85 ou mais novo **apenas**
  para compilar a partir do código-fonte. Os arquivos de release contêm um
  binário nativo e não exigem toolchain do Rust.

Instale a Norminette seguindo as instruções do repositório dela:
**[42School/norminette](https://github.com/42School/norminette)**. Aquele
projeto é o dono de como ele se instala, e o README dele é a única fonte que
continua correta quando isso muda.

Depois de instalada, confira se o `normfix` vai encontrá-la:

```sh
norminette --version
```

Um ambiente gerenciado pelo campus também serve. Para o `normfix`, só importam a
versão do comando e sua disponibilidade no `PATH`.

::: warning Compatibilidade de versão
Outra versão da Norminette que ainda seja interpretável roda com um aviso
destacado de compatibilidade, para que uma atualização do campus não desabilite
a ferramenta. Use `--strict-norminette-version` para rejeitar qualquer coisa
diferente de `3.3.59` numa CI com versão fixada. Veja a
[política de compatibilidade](/pt/COMPATIBILITY).
:::

## Instalação

### O instalador de uma linha

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

Ele detecta sua plataforma, baixa o arquivo de release correspondente, verifica
o resultado contra o `SHA256SUMS` publicado e instala o binário em
`~/.local/bin`. Ele nunca usa `sudo`, nunca escreve em diretório de sistema e
nunca instala uma toolchain, então funciona numa estação de trabalho da 42 onde
você não tem direitos administrativos. Por padrão ele usa a última release
estável do GitHub. Se o projeto ainda não publicou uma versão estável, ele cai,
com segurança, na pré-release mais recente, para que o candidato a release
atual continue instalável.

Duas variáveis de ambiente mudam o que ele faz:

```sh
NORMFIX_VERSION=v1.6.2 sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
NORMFIX_BIN_DIR=~/bin sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
```

`NORMFIX_VERSION` é exata: o instalador baixa aquela tag e não faz seleção de
canal.

Uma divergência de checksum aborta a instalação e imprime os dois digests. Leia
o script antes de mandá-lo para um shell, se preferir ver o que ele faz:
<https://normfix.vercel.app/install.sh>

### Homebrew

```sh
brew install viniciusnevescosta/normfix/normfix
```

A fórmula instala o mesmo binário pré-compilado e verificado; ela não compila a
partir do código-fonte. Disponível para macOS e Linuxbrew.

### Binários pré-compilados

As releases com tag oferecem arquivos nativos para Linux x86-64 e ARM64, além de
macOS Intel e Apple Silicon. Baixe o arquivo correspondente à sua máquina na
[release mais recente](https://github.com/viniciusnevescosta/normfix/releases/latest),
verifique-o contra o `SHA256SUMS` e coloque o `normfix` no `PATH`.

| Plataforma | Arquivo da release |
|---|---|
| Linux x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS Intel | `normfix-x86_64-macos.tar.gz` |
| macOS Apple Silicon | `normfix-aarch64-macos.tar.gz` |
| Windows x86-64 | `normfix-x86_64-windows.zip` |
| Windows ARM64 | `normfix-aarch64-windows.zip` |
| FreeBSD x86-64 | `normfix-x86_64-freebsd.tar.gz` |

Por exemplo, no Apple Silicon com a release `1.6.2`:

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

Crie o `$HOME/.local/bin` antes, se for necessário, e garanta que ele esteja no
`PATH`.

### Quando o sistema diz que o desenvolvedor não pode ser verificado

O macOS e o Windows avisam sobre programas que não são assinados com um
certificado pago de desenvolvedor. O normfix não é, então você pode ver um
aviso — e o caminho que você escolheu decide se vai ver.

O instalador de uma linha baixa com `curl`, que não anexa a marca que dispara o
aviso. Instalando assim, você não verá nada. Um navegador anexa, então um
arquivo baixado pela página de releases é o caso que avisa.

No macOS a mensagem diz que o desenvolvedor não pode ser verificado. Abra o
arquivo uma vez pelo Finder com **clique direito → Abrir**, que oferece um botão
que o clique duplo normal não oferece, ou remova a marca diretamente:

```sh
xattr -d com.apple.quarantine ./normfix
```

No Windows, o SmartScreen diz que protegeu seu PC. Escolha **Mais informações**
e depois **Executar assim mesmo**. Pelo PowerShell:

```powershell
Unblock-File .\normfix.exe
```

Não ser assinado é uma posição deliberada, não um descuido. Um certificado de
assinatura prova que alguém pagou a uma autoridade certificadora; ele não diz
nada sobre o que o binário contém. Todo arquivo aqui é publicado com um
manifesto de checksums e com procedência de build que o liga à execução do
workflow que o produziu — uma afirmação mais forte, que o sistema operacional
simplesmente não consulta:

```sh
gh attestation verify normfix-aarch64-macos.tar.gz --repo viniciusnevescosta/normfix
```

Se esse comando passa, o arquivo que você tem saiu do workflow de release deste
projeto, diga o sistema operacional o que disser sobre a assinatura.

### Compilando do código-fonte

```sh
git clone https://github.com/viniciusnevescosta/normfix.git
cd normfix
cargo install --path crates/normfix-cli --locked
```

Ou gere um binário de release sem instalá-lo:

```sh
cargo build --release --locked -p normfix
./target/release/normfix --version
```

O Cargo normalmente instala o comando em `~/.cargo/bin`; garanta que esse
diretório esteja no `PATH`.

### Windows

O mesmo instalador de uma linha funciona em qualquer shell POSIX — Git Bash,
MSYS2, Cygwin ou WSL — e instala o `normfix.exe`:

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

Se você só tem o PowerShell, o Scoop é a conveniência:

```powershell
scoop bucket add normfix https://github.com/viniciusnevescosta/scoop-normfix
scoop install normfix
```

Ou baixe `normfix-x86_64-windows.zip` ou `normfix-aarch64-windows.zip` da página
de releases e coloque o `normfix.exe` no `PATH`.

A Norminette oficial é um programa Python e se instala no Windows do mesmo jeito
que em qualquer lugar. Rodar o build de Linux dentro do WSL continua suportado e
inalterado; a [política de compatibilidade](/pt/COMPATIBILITY) nomeia os dois
pontos em que o Windows nativo se comporta de forma diferente do Unix.

## Primeira execução segura

Veja o que aconteceria antes de escrever qualquer coisa:

```sh
normfix --check
normfix --diff
```

Depois aplique as mudanças aceitas:

```sh
normfix
```

O modo padrão de correção escreve no lugar, mas guarda os arquivos originais num
diretório externo de backup. Nenhum arquivo do projeto é escrito nos modos
`--check` ou `--diff`.

## Próximos passos

- [Linha de comando](/pt/guide/command-line): fluxos, flags, escopos do Git e
  códigos de saída.
- [Playground no navegador](/pt/guide/playground): experimente o formatador sem
  instalar nada.
- [Arquitetura](/pt/ARCHITECTURE): o que cada crate é dona e por que os limites
  existem.
