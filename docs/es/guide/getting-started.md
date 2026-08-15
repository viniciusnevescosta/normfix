# Primeros pasos

Al terminar esta página tendrás `normfix` instalado y lo habrás ejecutado una
vez sobre un proyecto real, sin que toque ni un solo archivo.

## Instalarlo

Un solo comando, en Linux, macOS y Windows:

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

Averigua en qué máquina estás, descarga la versión que corresponde, la compara
con las sumas de verificación publicadas y deja el programa en `~/.local/bin`.
No pide `sudo`, no escribe fuera de tu carpeta personal y no instala ningún
compilador: eso es lo que hace que funcione en un ordenador de 42, donde no eres
administrador.

En Windows, ejecútalo desde cualquier shell POSIX: Git Bash, MSYS2, Cygwin o
WSL.

Comprueba que llegó:

```sh
normfix --version
```

Si la terminal no encuentra el comando, es que `~/.local/bin` todavía no está en
tu `PATH`. Añádelo al archivo de inicio de tu shell y abre una terminal nueva.

::: tip Leer antes de ejecutar
Enviar un script directamente al shell es ejecutar código que no has leído. Si
prefieres mirarlo antes, está aquí:
<https://normfix.vercel.app/install.sh>
:::

## También necesitas la Norminette

Quien decide lo que dice la Norm no es `normfix`, sino la Norminette oficial, y
`normfix` se lo pregunta. Así que el verificador tiene que estar instalado y en
tu `PATH`, o una ejecución no significa nada.

Instálala desde su propio repositorio, [42School/norminette][norminette]. Ese
proyecto es el que decide cómo se instala, y su README es la única página que
sigue siendo correcta cuando eso cambia.

[norminette]: https://github.com/42School/norminette

Después comprueba que `normfix` la va a encontrar:

```sh
norminette --version
```

Una instalación gestionada por el campus sirve. Lo único que importa es que el
comando funcione y diga su versión.

::: warning Si tu campus actualiza la Norminette
`3.3.59` es la versión con la que se probó esta release. Otra sigue
funcionando, con un aviso, para que una actualización del campus nunca te deje
sin la herramienta. En una pipeline donde quieras la versión fijada,
`--strict-norminette-version` rechaza cualquier otra. La [política de
compatibilidad](/es/COMPATIBILITY) explica lo que cuesta ese aviso.
:::

## La primera ejecución no cambia nada

Entra en un proyecto y pregunta qué haría `normfix`:

```sh
normfix --check
```

Eso lee tus archivos y no escribe ninguno. Muestra qué arreglaría, qué no puede
arreglar y por qué.

Para ver las ediciones en sí, en lugar de un resumen:

```sh
normfix --diff
```

Cuando estés listo:

```sh
normfix
```

Este sí escribe. Antes de hacerlo, copia cada archivo que va a tocar a una
carpeta de respaldo fuera de tu proyecto, para que `normfix undo` pueda
devolverlos.

## Otras formas de instalar

El instalador de arriba es el que funciona en todas partes. Las demás están aquí
porque quizá ya uses alguna.

### Homebrew

```sh
brew install viniciusnevescosta/normfix/normfix
brew upgrade viniciusnevescosta/normfix/normfix  # después
brew uninstall normfix                            # para quitarlo
```

Instala el mismo programa ya verificado, en lugar de compilarlo. Para macOS y
Linuxbrew.

### Scoop, en Windows sin un shell POSIX

```powershell
scoop bucket add normfix https://github.com/viniciusnevescosta/scoop-normfix
scoop install normfix
scoop update normfix     # después, para actualizar
scoop uninstall normfix  # para quitarlo
```

Scoop es el dueño de esa instalación, así que `normfix upgrade` y `normfix uninstall`
se niegan y te devuelven a estos — cambiar el binario por debajo dejaría el
manifiesto de Scoop describiendo algo que ya no está.

### Descargando el paquete tú mismo

Cada release publica un paquete por plataforma, con un archivo `SHA256SUMS` al
lado:

| Plataforma | Paquete |
|---|---|
| Linux x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS Intel | `normfix-x86_64-macos.tar.gz` |
| macOS Apple Silicon | `normfix-aarch64-macos.tar.gz` |
| Windows x86-64 | `normfix-x86_64-windows.zip` |
| Windows ARM64 | `normfix-aarch64-windows.zip` |
| FreeBSD x86-64 | `normfix-x86_64-freebsd.tar.gz` |

En un Apple Silicon, por ejemplo:

```sh
version=1.7.0
archive="normfix-aarch64-macos.tar.gz"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/${archive}"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/SHA256SUMS"
grep " ${archive}$" SHA256SUMS | shasum -a 256 -c -
tar -xzf "${archive}"
install -m 0755 normfix "$HOME/.local/bin/normfix"
normfix --version
```

Crea antes la carpeta `$HOME/.local/bin` si no existe, y asegúrate de que esté
en tu `PATH`.

### Fijar una versión, o elegir dónde queda

```sh
NORMFIX_VERSION=v1.7.0 sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
NORMFIX_BIN_DIR=~/bin sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
```

`NORMFIX_VERSION` es literal: descarga esa etiqueta, sin elegir canal. Sin ella
recibes la release estable más nueva — o, si todavía no hay ninguna, la
preliberación más nueva, para que un candidato a release siga siendo instalable.

Si una suma de verificación no coincide, la instalación se detiene y muestra
ambos valores.

### Compilando desde el código fuente

```sh
git clone https://github.com/viniciusnevescosta/normfix.git
cd normfix
cargo install --path crates/normfix-cli --locked
```

O compila sin instalar:

```sh
cargo build --release --locked -p normfix
./target/release/normfix --version
```

Este es el único camino que necesita Rust — 1.85 o más nuevo. Cargo instala en
`~/.cargo/bin`, así que esa carpeta tiene que estar en tu `PATH`.

## Si el sistema dice que no se puede verificar al desarrollador

macOS y Windows avisan sobre programas que no están firmados con un certificado
de desarrollador de pago. `normfix` no está firmado, así que puede que veas ese
aviso — y el camino que elegiste decide si lo ves siquiera.

El instalador de una línea descarga con `curl`, que no pone la marca que dispara
el aviso; instalando así no lo verás nunca. El navegador sí la pone, así que
descargar el paquete desde la página de releases es el caso que avisa.

**En macOS**, el mensaje dice que no se pudo verificar al desarrollador. Ábrelo
una vez desde el Finder con **clic derecho → Abrir**, que ofrece un botón que el
doble clic no ofrece. O quita la marca tú mismo:

```sh
xattr -d com.apple.quarantine ./normfix
```

**En Windows**, SmartScreen dice que protegió tu PC. Elige **Más información** y
luego **Ejecutar de todas formas**. Desde PowerShell:

```powershell
Unblock-File .\normfix.exe
```

No firmar es una decisión, no un descuido. Un certificado demuestra que alguien
pagó a una autoridad certificadora; no dice nada sobre lo que hay dentro del
archivo. Cada paquete aquí se publica con su suma de verificación y con la
procedencia de compilación que lo ata a la ejecución exacta que lo produjo — una
afirmación más fuerte, que el sistema operativo sencillamente no mira:

```sh
gh attestation verify normfix-aarch64-macos.tar.gz --repo viniciusnevescosta/normfix
```

Si ese comando funciona, el archivo que tienes salió de la línea de release de
este proyecto, diga lo que diga tu sistema sobre la firma.

## Adónde ir ahora

- [Línea de comandos](/es/guide/command-line) — los flujos, las flags y qué
  significa cada código de salida.
- [Playground en el navegador](/es/guide/playground) — pruébalo sin instalar
  nada.
- [Arquitectura](/es/ARCHITECTURE) — cómo encajan las piezas y por qué las
  fronteras están donde están.
