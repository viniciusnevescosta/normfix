# Premiers pas

À la fin de cette page, `normfix` sera installé et vous l'aurez lancé une fois
sur un vrai projet, sans qu'il touche à un seul fichier.

## L'installer

Une seule commande, sur Linux, macOS et Windows :

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

Elle détermine sur quelle machine vous êtes, télécharge la version qui
correspond, la compare aux sommes de contrôle publiées et place le programme
dans `~/.local/bin`. Elle ne demande pas `sudo`, n'écrit rien hors de votre
dossier personnel et n'installe aucun compilateur — c'est ce qui lui permet de
fonctionner sur un poste de 42, où vous n'êtes pas administrateur.

Sous Windows, lancez-la depuis n'importe quel shell POSIX : Git Bash, MSYS2,
Cygwin ou WSL.

Vérifiez qu'elle est bien arrivée :

```sh
normfix --version
```

Si le terminal ne trouve pas la commande, c'est que `~/.local/bin` n'est pas
encore dans votre `PATH`. Ajoutez ce dossier au fichier de démarrage de votre
shell et ouvrez un nouveau terminal.

::: tip Lire avant de lancer
Envoyer un script directement au shell, c'est exécuter du code que vous n'avez
pas lu. Si vous préférez le regarder d'abord, il est ici :
<https://normfix.vercel.app/install.sh>
:::

## Il vous faut aussi la Norminette

Ce n'est pas `normfix` qui décide ce que dit la Norm, c'est la Norminette
officielle, et `normfix` le lui demande. Le vérificateur doit donc être installé
et présent dans votre `PATH`, sinon une exécution ne veut rien dire.

Installez-la depuis son propre dépôt, [42School/norminette][norminette]. C'est
ce projet qui décide comment elle s'installe, et son README est la seule page
qui reste juste quand cela change.

[norminette]: https://github.com/42School/norminette

Vérifiez ensuite que `normfix` la trouvera :

```sh
norminette --version
```

Une installation gérée par le campus convient. Seul compte que la commande
réponde et annonce sa version.

::: warning Si votre campus met la Norminette à jour
`3.3.59` est la version contre laquelle cette release a été testée. Une autre
fonctionne quand même, avec un avertissement, pour qu'une mise à jour du campus
ne vous prive jamais de l'outil. Dans une pipeline où vous voulez la version
figée, `--strict-norminette-version` refuse tout le reste. La [politique de
compatibilité](/fr/COMPATIBILITY) explique ce que cet avertissement coûte.
:::

## La première exécution ne change rien

Placez-vous dans un projet et demandez ce que `normfix` ferait :

```sh
normfix --check
```

Cela lit vos fichiers sans en écrire aucun. Il affiche ce qu'il corrigerait, ce
qu'il ne peut pas corriger, et pourquoi.

Pour voir les modifications elles-mêmes plutôt qu'un résumé :

```sh
normfix --diff
```

Quand vous êtes prêt :

```sh
normfix
```

Celle-ci écrit. Avant cela, elle copie chaque fichier qu'elle va toucher dans un
dossier de sauvegarde hors de votre projet, pour que `normfix undo` puisse les
remettre.

## Autres façons d'installer

L'installateur ci-dessus est celui qui marche partout. Les autres sont là parce
que vous en utilisez peut-être déjà une.

### Homebrew

```sh
brew install viniciusnevescosta/normfix/normfix
brew upgrade viniciusnevescosta/normfix/normfix  # plus tard
brew uninstall normfix                            # pour le retirer
```

Installe le même programme déjà vérifié, au lieu de le compiler. Pour macOS et
Linuxbrew.

### Scoop, sous Windows sans shell POSIX

```powershell
scoop bucket add normfix https://github.com/viniciusnevescosta/scoop-normfix
scoop install normfix
scoop update normfix     # plus tard, pour mettre à jour
scoop uninstall normfix  # pour le retirer
```

C'est Scoop qui possède cette installation : `normfix upgrade` et `normfix uninstall`
refusent et vous renvoient ici — remplacer le binaire en dessous laisserait le
manifeste de Scoop décrivant quelque chose qui n'y est plus.

### Télécharger l'archive vous-même

Chaque release publie une archive par plateforme, avec un fichier `SHA256SUMS` à
côté :

| Plateforme | Archive |
|---|---|
| Linux x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS Intel | `normfix-x86_64-macos.tar.gz` |
| macOS Apple Silicon | `normfix-aarch64-macos.tar.gz` |
| Windows x86-64 | `normfix-x86_64-windows.zip` |
| Windows ARM64 | `normfix-aarch64-windows.zip` |
| FreeBSD x86-64 | `normfix-x86_64-freebsd.tar.gz` |

Sur Apple Silicon, par exemple :

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

Créez d'abord `$HOME/.local/bin` s'il n'existe pas, et assurez-vous qu'il est
dans votre `PATH`.

### Figer une version, ou choisir où il atterrit

```sh
NORMFIX_VERSION=v1.6.2 sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
NORMFIX_BIN_DIR=~/bin sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
```

`NORMFIX_VERSION` est pris au pied de la lettre : ce tag est téléchargé, sans
choix de canal. Sans lui, vous obtenez la release stable la plus récente — ou,
s'il n'y en a pas encore, la préversion la plus récente, pour qu'un candidat à
la release reste installable.

Si une somme de contrôle ne correspond pas, l'installation s'arrête et affiche
les deux valeurs.

### Compiler depuis les sources

```sh
git clone https://github.com/viniciusnevescosta/normfix.git
cd normfix
cargo install --path crates/normfix-cli --locked
```

Ou compiler sans installer :

```sh
cargo build --release --locked -p normfix
./target/release/normfix --version
```

C'est le seul chemin qui demande Rust — 1.85 ou plus récent. Cargo installe dans
`~/.cargo/bin`, ce dossier doit donc être dans votre `PATH`.

## Si le système dit que le développeur ne peut pas être vérifié

macOS et Windows avertissent au sujet des programmes qui ne sont pas signés avec
un certificat de développeur payant. `normfix` ne l'est pas, vous verrez donc
peut-être cet avertissement — et le chemin que vous avez pris décide si vous le
voyez du tout.

L'installateur en une ligne télécharge avec `curl`, qui n'attache pas la marque
déclenchant l'avertissement : en installant ainsi, vous ne le verrez jamais. Un
navigateur l'attache, donc télécharger une archive depuis la page des releases
est le cas qui avertit.

**Sous macOS**, le message dit que le développeur n'a pas pu être vérifié.
Ouvrez-le une fois depuis le Finder avec **clic droit → Ouvrir**, qui propose un
bouton que le double-clic ne propose pas. Ou retirez la marque vous-même :

```sh
xattr -d com.apple.quarantine ./normfix
```

**Sous Windows**, SmartScreen annonce qu'il a protégé votre PC. Choisissez
**Informations complémentaires**, puis **Exécuter quand même**. Depuis
PowerShell :

```powershell
Unblock-File .\normfix.exe
```

Ne pas signer est une décision, pas un oubli. Un certificat prouve que quelqu'un
a payé une autorité de certification ; il ne dit rien de ce que contient le
fichier. Chaque archive ici est publiée avec sa somme de contrôle et avec une
provenance de compilation qui la relie à l'exécution exacte qui l'a produite —
une affirmation plus forte, que le système d'exploitation ne consulte tout
simplement pas :

```sh
gh attestation verify normfix-aarch64-macos.tar.gz --repo viniciusnevescosta/normfix
```

Si cette commande réussit, le fichier que vous avez sort de la chaîne de release
de ce projet, quoi que votre système dise de sa signature.

## Où aller ensuite

- [Ligne de commande](/fr/guide/command-line) — les flux, les options et ce que
  signifie chaque code de sortie.
- [Playground dans le navigateur](/fr/guide/playground) — essayez sans rien
  installer.
- [Architecture](/fr/ARCHITECTURE) — comment les pièces s'assemblent et pourquoi
  les frontières sont là où elles sont.
