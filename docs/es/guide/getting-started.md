# Primeros pasos

## Requisito principal

Instala la [Norminette oficial](https://github.com/42school/norminette) y deja
el comando disponible en el `PATH`:

```sh
pipx install norminette==3.3.59
norminette --version
```

El binario listo de `normfix` no necesita Python ni Rust.
[Rust](https://www.rust-lang.org/tools/install) solo es necesario para compilar
desde el código fuente. En Windows, usa
[WSL](https://learn.microsoft.com/windows/wsl/install) para la CLI completa; el
playground funciona directamente en un navegador moderno.

## Instalación

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

El instalador elige el archivo de tu plataforma, verifica SHA-256 e instala en
`~/.local/bin` sin `sudo`. También puedes usar Homebrew:

```sh
brew install viniciusnevescosta/normfix/normfix
```

## Primer uso

```sh
normfix
normfix check
normfix --diff
normfix preflight
```

Consulta la [referencia completa de la CLI](/guide/command-line) para archivos
concretos, ámbitos Git, identidad, copias y opciones destructivas.
