# Bien démarrer

## Prérequis principal

Installez la [Norminette officielle](https://github.com/42School/norminette) en
suivant les instructions de son propre dépôt et rendez sa commande disponible
dans le `PATH`. Ce projet est propriétaire de sa procédure d'installation, et
son README est la seule source qui reste correcte quand elle change.

Une fois installée, vérifiez que `normfix` la trouvera :

```sh
norminette --version
```

Le binaire `normfix` prêt à l’emploi n’exige ni Python ni Rust.
[Rust](https://www.rust-lang.org/tools/install) n’est nécessaire que pour
compiler les sources. Sous Windows, utilisez
[WSL](https://learn.microsoft.com/windows/wsl/install) pour la CLI complète ;
le playground fonctionne directement dans un navigateur moderne.

## Installation

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

L’installateur choisit l’archive adaptée, vérifie SHA-256 et place le binaire
dans `~/.local/bin` sans `sudo`. Homebrew est aussi disponible :

```sh
brew install viniciusnevescosta/normfix/normfix
```

## Première utilisation

```sh
normfix
normfix check
normfix --diff
normfix preflight
```

Consultez la [référence complète de la CLI](/guide/command-line) pour les
fichiers précis, les portées Git, l’identité, les sauvegardes et les options
destructives.
