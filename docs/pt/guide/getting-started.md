# Primeiros passos

## Requisito principal

Instale a [Norminette oficial](https://github.com/42school/norminette) e deixe o
comando disponível no `PATH`:

```sh
pipx install norminette==3.3.59
norminette --version
```

O binário pronto do `normfix` não exige Python nem Rust. O
[Rust](https://www.rust-lang.org/tools/install) é necessário apenas para
compilar o projeto a partir do código-fonte. No Windows, use o
[WSL](https://learn.microsoft.com/windows/wsl/install) para a CLI completa; o
playground funciona diretamente em um navegador moderno.

## Instalação

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

O instalador escolhe o arquivo da sua plataforma, verifica o SHA-256 e coloca o
binário em `~/.local/bin` sem `sudo`. Também é possível usar Homebrew:

```sh
brew install viniciusnevescosta/normfix/normfix
```

## Primeiro uso

Dentro do projeto:

```sh
normfix
```

Para revisar sem escrever:

```sh
normfix check
normfix --diff
normfix preflight
```

O comando sempre mostra o escopo e as configurações antes de uma operação que
escreve. Consulte a [referência completa da CLI](/guide/command-line) para
arquivos específicos, escopos Git, identidade, backup e opções destrutivas.
