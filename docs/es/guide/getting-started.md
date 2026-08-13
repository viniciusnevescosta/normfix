# Primeros pasos

## Requisitos

- El comando de la [Norminette oficial](https://github.com/42School/norminette)
  disponible en el `PATH`, o indicado con `--norminette RUTA`. La versión
  `3.3.59` es la base de compatibilidad probada.
- [Rust](https://www.rust-lang.org/tools/install) 1.85 o más reciente **solo**
  para compilar desde el código fuente. Los archivos de release contienen un
  binario nativo y no necesitan toolchain de Rust.

Instala la Norminette siguiendo las instrucciones de su propio repositorio:
**[42School/norminette](https://github.com/42School/norminette)**. Ese proyecto
es el dueño de cómo se instala, y su README es la única fuente que sigue siendo
correcta cuando eso cambia.

Una vez instalada, comprueba que `normfix` la va a encontrar:

```sh
norminette --version
```

Un entorno gestionado por el campus también sirve. A `normfix` solo le importan
la versión del comando y su disponibilidad en el `PATH`.

::: warning Compatibilidad de versión
Otra versión de la Norminette que aún se pueda interpretar se ejecuta con un
aviso destacado de compatibilidad, para que una actualización del campus no
deshabilite la herramienta. Usa `--strict-norminette-version` para rechazar
cualquier cosa distinta de `3.3.59` en una CI con versión fijada. Consulta la
[política de compatibilidad](/es/COMPATIBILITY).
:::

## Instalación

### El instalador de una línea

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

Detecta tu plataforma, descarga el archivo de release correspondiente, lo
verifica contra el `SHA256SUMS` publicado e instala el binario en
`~/.local/bin`. Nunca usa `sudo`, nunca escribe en un directorio del sistema y
nunca instala una toolchain, así que funciona en una estación de trabajo de 42
donde no tienes permisos de administrador. Por defecto usa la última release
estable de GitHub. Si el proyecto aún no ha publicado una versión estable,
recurre con seguridad a la pre-release más reciente, para que el candidato a
release actual siga siendo instalable.

Dos variables de entorno cambian lo que hace:

```sh
NORMFIX_VERSION=v1.6.0 sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
NORMFIX_BIN_DIR=~/bin sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
```

`NORMFIX_VERSION` es exacta: el instalador descarga esa etiqueta y no realiza
selección de canal.

Una discrepancia de checksum aborta la instalación e imprime ambos digests. Lee
el script antes de pasarlo a un shell, si prefieres ver lo que hace:
<https://normfix.vercel.app/install.sh>

### Homebrew

```sh
brew install viniciusnevescosta/normfix/normfix
```

La fórmula instala el mismo binario precompilado y verificado; no compila desde
el código fuente. Disponible para macOS y Linuxbrew.

### Binarios precompilados

Las releases etiquetadas ofrecen archivos nativos para Linux x86-64 y ARM64,
además de macOS Intel y Apple Silicon. Descarga el archivo que corresponda a tu
máquina desde la
[última release](https://github.com/viniciusnevescosta/normfix/releases/latest),
verifícalo contra `SHA256SUMS` y coloca `normfix` en el `PATH`.

| Plataforma | Archivo de release |
|---|---|
| Linux x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS Intel | `normfix-x86_64-macos.tar.gz` |
| macOS Apple Silicon | `normfix-aarch64-macos.tar.gz` |
| Windows x86-64 | `normfix-x86_64-windows.zip` |
| Windows ARM64 | `normfix-aarch64-windows.zip` |
| FreeBSD x86-64 | `normfix-x86_64-freebsd.tar.gz` |

Por ejemplo, en Apple Silicon con la release `1.6.0`:

```sh
version=1.6.0
archive="normfix-aarch64-macos.tar.gz"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/${archive}"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/SHA256SUMS"
grep " ${archive}$" SHA256SUMS | shasum -a 256 -c -
tar -xzf "${archive}"
install -m 0755 normfix "$HOME/.local/bin/normfix"
normfix --version
```

Crea antes `$HOME/.local/bin` si hace falta y asegúrate de que esté en el
`PATH`.

### Cuando el sistema dice que no puede verificar al desarrollador

macOS y Windows avisan sobre programas que no están firmados con un certificado
de desarrollador de pago. normfix no lo está, así que puede que veas un aviso — y
la ruta que elegiste decide si lo ves.

El instalador de una línea descarga con `curl`, que no adjunta la marca que
dispara el aviso. Instalando así no verás nada. Un navegador sí la adjunta, así
que un archivo descargado desde la página de releases es el caso que avisa.

En macOS el mensaje dice que no se puede verificar al desarrollador. Ábrelo una
vez desde el Finder con **clic derecho → Abrir**, que ofrece un botón que el
doble clic normal no da, o quita la marca directamente:

```sh
xattr -d com.apple.quarantine ./normfix
```

En Windows, SmartScreen dice que protegió tu PC. Elige **Más información** y
luego **Ejecutar de todas formas**. Desde PowerShell:

```powershell
Unblock-File .\normfix.exe
```

No estar firmado es una postura deliberada, no un descuido. Un certificado de
firma demuestra que alguien pagó a una autoridad certificadora; no dice nada
sobre lo que contiene el binario. Cada archivo aquí se publica con un manifiesto
de checksums y con procedencia de compilación que lo vincula a la ejecución del
workflow que lo produjo, una afirmación más fuerte que el sistema operativo
sencillamente no consulta:

```sh
gh attestation verify normfix-aarch64-macos.tar.gz --repo viniciusnevescosta/normfix
```

Si ese comando pasa, el archivo que tienes salió del workflow de release de este
proyecto, diga lo que diga el sistema operativo sobre su firma.

### Compilar desde el código fuente

```sh
git clone https://github.com/viniciusnevescosta/normfix.git
cd normfix
cargo install --path crates/normfix-cli --locked
```

O genera un binario de release sin instalarlo:

```sh
cargo build --release --locked -p normfix
./target/release/normfix --version
```

Cargo normalmente instala el comando en `~/.cargo/bin`; asegúrate de que ese
directorio esté en el `PATH`.

### Windows

El mismo instalador de una línea funciona en cualquier shell POSIX — Git Bash,
MSYS2, Cygwin o WSL — e instala `normfix.exe`:

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

Si solo tienes PowerShell, Scoop es la comodidad:

```powershell
scoop bucket add normfix https://github.com/viniciusnevescosta/scoop-normfix
scoop install normfix
```

O descarga `normfix-x86_64-windows.zip` o `normfix-aarch64-windows.zip` desde la
página de releases y pon `normfix.exe` en el `PATH`.

La Norminette oficial es un programa Python y se instala en Windows igual que en
cualquier otro sitio. Ejecutar la compilación de Linux dentro de WSL sigue
soportado y sin cambios; la [política de compatibilidad](/es/COMPATIBILITY)
nombra los dos puntos en los que Windows nativo se comporta distinto de Unix.

## Primera ejecución segura

Previsualiza un proyecto antes de escribir nada:

```sh
normfix --check
normfix --diff
```

Después aplica los cambios aceptados:

```sh
normfix
```

El modo de corrección por defecto escribe en el sitio, pero conserva los
archivos originales en un directorio externo de copia de seguridad. Ningún
archivo del proyecto se escribe en los modos `--check` o `--diff`.

## Siguientes pasos

- [Línea de comandos](/es/guide/command-line): flujos, flags, ámbitos de Git y
  códigos de salida.
- [Playground en el navegador](/es/guide/playground): prueba el formateador sin
  instalar nada.
- [Arquitectura](/es/ARCHITECTURE): de qué es dueño cada crate y por qué existen
  los límites.
