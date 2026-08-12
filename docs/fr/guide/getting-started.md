# Bien démarrer

## Prérequis

- La commande de la
  [Norminette officielle](https://github.com/42School/norminette) disponible
  dans le `PATH`, ou fournie avec `--norminette CHEMIN`. La version `3.3.59` est
  la base de compatibilité testée.
- [Rust](https://www.rust-lang.org/tools/install) 1.85 ou plus récent
  **uniquement** pour compiler depuis les sources. Les archives de release
  contiennent un binaire natif et ne demandent aucune toolchain Rust.

Installez la Norminette en suivant les instructions de son propre dépôt :
**[42School/norminette](https://github.com/42School/norminette)**. Ce projet est
propriétaire de la façon dont il s’installe, et son README est la seule source
qui reste correcte quand cela change.

Une fois installée, vérifiez que `normfix` va la trouver :

```sh
norminette --version
```

Un environnement géré par le campus convient aussi. Pour `normfix`, seules
comptent la version de la commande et sa présence dans le `PATH`.

::: warning Compatibilité de version
Une autre version de la Norminette encore interprétable s’exécute avec un
avertissement de compatibilité bien visible, afin qu’une mise à jour du campus
ne désactive pas l’outil. Utilisez `--strict-norminette-version` pour refuser
tout ce qui n’est pas `3.3.59` dans une CI à version figée. Voyez la
[politique de compatibilité](/fr/COMPATIBILITY).
:::

## Installation

### L’installateur en une ligne

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

Il détecte votre plateforme, télécharge l’archive de release correspondante, la
vérifie contre le `SHA256SUMS` publié et installe le binaire dans
`~/.local/bin`. Il n’utilise jamais `sudo`, n’écrit jamais dans un répertoire
système et n’installe jamais de toolchain : il fonctionne donc sur un poste de
42 où vous n’avez pas les droits d’administration. Par défaut il prend la
dernière release stable de GitHub. Si le projet n’a pas encore publié de version
stable, il se rabat sans risque sur la pré-version la plus récente, pour que le
candidat de release courant reste installable.

Deux variables d’environnement changent son comportement :

```sh
NORMFIX_VERSION=v1.3.0 sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
NORMFIX_BIN_DIR=~/bin sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
```

`NORMFIX_VERSION` est exacte : l’installateur télécharge cette étiquette et
n’effectue aucune sélection de canal.

Une somme de contrôle qui ne correspond pas interrompt l’installation et affiche
les deux empreintes. Lisez le script avant de le donner à un shell, si vous
préférez voir ce qu’il fait :
<https://normfix.vercel.app/install.sh>

### Homebrew

```sh
brew install viniciusnevescosta/normfix/normfix
```

La formule installe le même binaire précompilé et vérifié ; elle ne compile pas
depuis les sources. Disponible pour macOS et Linuxbrew.

### Binaires précompilés

Les releases étiquetées fournissent des archives natives pour Linux x86-64 et
ARM64, ainsi que macOS Intel et Apple Silicon. Téléchargez l’archive qui
correspond à votre machine depuis la
[dernière release](https://github.com/viniciusnevescosta/normfix/releases/latest),
vérifiez-la contre `SHA256SUMS`, et placez `normfix` dans le `PATH`.

| Plateforme | Archive de release |
|---|---|
| Linux x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS Intel | `normfix-x86_64-macos.tar.gz` |
| macOS Apple Silicon | `normfix-aarch64-macos.tar.gz` |

Par exemple, sur Apple Silicon avec la release `1.3.0` :

```sh
version=1.3.0
archive="normfix-aarch64-macos.tar.gz"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/${archive}"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/SHA256SUMS"
grep " ${archive}$" SHA256SUMS | shasum -a 256 -c -
tar -xzf "${archive}"
install -m 0755 normfix "$HOME/.local/bin/normfix"
normfix --version
```

Créez d’abord `$HOME/.local/bin` si nécessaire et assurez-vous qu’il est dans le
`PATH`.

### Compiler depuis les sources

```sh
git clone https://github.com/viniciusnevescosta/normfix.git
cd normfix
cargo install --path crates/normfix-cli --locked
```

Ou produisez un binaire de release sans l’installer :

```sh
cargo build --release --locked -p normfix
./target/release/normfix --version
```

Cargo installe normalement la commande dans `~/.cargo/bin` ; assurez-vous que ce
répertoire est dans le `PATH`.

### Windows

Il n’existe pas d’archive Windows native. Exécutez la CLI Linux et sa dépendance
Norminette dans [WSL](https://learn.microsoft.com/windows/wsl/install), ou
utilisez le [playground dans le navigateur](/fr/guide/playground) pour l’aperçu
du formateur en mémoire. Le comportement natif de PowerShell et des processus
Windows ne fait pas encore partie du contrat de support de la CLI.

## Première exécution sûre

Prévisualisez un projet avant d’écrire quoi que ce soit :

```sh
normfix --check
normfix --diff
```

Appliquez ensuite les changements retenus :

```sh
normfix
```

Le mode de correction par défaut écrit sur place, mais conserve les fichiers
d’origine dans un répertoire de sauvegarde externe. Aucun fichier du projet
n’est écrit dans les modes `--check` ou `--diff`.

## Étapes suivantes

- [Ligne de commande](/fr/guide/command-line) : déroulés, options, portées Git
  et codes de sortie.
- [Playground dans le navigateur](/fr/guide/playground) : essayez le formateur
  sans rien installer.
- [Architecture](/fr/ARCHITECTURE) : ce que possède chaque crate et pourquoi les
  frontières existent.
