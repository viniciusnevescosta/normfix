# `normfix uninstall`

Supprime ce binaire et — uniquement si vous le demandez explicitement — les
données qu'il a créées.

```sh
normfix uninstall --dry-run   # montre le plan, ne supprime rien
normfix uninstall             # supprime le binaire, garde vos données
normfix uninstall --purge     # supprime aussi configuration, cache et sauvegardes
```

Installé avec Homebrew ? Utilisez `brew`, à qui appartient cette copie :

```sh
brew uninstall viniciusnevescosta/normfix/normfix
```

`normfix uninstall` refuse un binaire géré par Homebrew et affiche cette
commande, plutôt que de supprimer un fichier que la formule décrit encore. Votre
configuration, le cache et les sauvegardes vivent hors de la formule :
supprimez-les séparément si vous voulez qu'ils disparaissent :

```sh
rm -rf ~/.config/normfix ~/.cache/normfix ~/.local/share/normfix
```

Ce dernier chemin contient les sauvegardes et les fichiers en quarantaine, qui
sont l'unique copie de tout ce qu'une exécution précédente a remplacé ou
déplacé.

## Il montre d'abord le plan

Rien n'est supprimé avant que vous ayez vu exactement ce qui le serait :

```console
$ normfix uninstall --dry-run
normfix uninstall
  remove  /usr/local/bin/normfix
  keep    /home/student/.config/normfix (configuration)
  keep    /home/student/.cache/normfix (cache)
  keep    /home/student/.local/share/normfix (backups and quarantine)
Pass --purge to remove the kept directories as well.
```

Par défaut, vos données sont conservées. C'est délibéré : le répertoire de
sauvegardes contient l'unique copie de tout ce qu'une exécution précédente a
remplacé ou déplacé, et désinstaller un formateur n'est pas une façon de dire que
vous voulez perdre le travail qu'il a mis de côté pour vous.

## `--purge`

```console
$ normfix uninstall --purge --dry-run
normfix uninstall
  remove  /usr/local/bin/normfix
  remove  /home/student/.config/normfix (configuration)
  remove  /home/student/.cache/normfix (cache)
  remove  /home/student/.local/share/normfix (backups and quarantine)
This also deletes backups and quarantined files, which is the only copy of anything a previous run replaced or moved.
```

La configuration et le cache sont reproductibles : la première est votre identité
42, que vous pouvez fournir à nouveau, et le second est un cache. Les sauvegardes
et les fichiers en quarantaine ne le sont pas. Lancez
[`normfix undo --list`](/fr/commands/undo) d'abord si vous n'êtes pas sûr que
quelque chose soit encore récupérable.

## Confirmation

Une exécution interactive demande avant de supprimer quoi que ce soit :

```console
Supprimer les fichiers listés ci-dessus ? [y/N]
```

`y` est la réponse acceptée dans toutes les langues. Une exécution non
interactive — un script, la CI ou `--format json` — refuse au lieu de supposer,
et exige `--force` :

```sh
normfix uninstall --force
normfix uninstall --purge --force
```

## Quand il refuse

| Situation | Ce qu'il dit |
|---|---|
| Installé par Homebrew | Vous renvoie à `brew uninstall viniciusnevescosta/normfix/normfix` |
| Pas de droit d'écriture | Nomme le chemin et dit de vérifier le propriétaire ; il ne demande jamais `sudo` |
| Un répertoire de données ne peut pas être supprimé | Nomme ce répertoire et s'arrête, le binaire étant toujours installé |

Homebrew est refusé plutôt que contourné : supprimer un fichier que la formule
décrit encore laisse `brew` comme seule chose capable de remettre la machine dans
un état cohérent.

Les répertoires de données sont supprimés avant le binaire. Si l'un d'eux échoue,
l'outil qui a signalé l'échec est toujours sur le disque pour réessayer.

## Supprimer un binaire en cours d'exécution

Sous Unix, délier l'exécutable en cours est sûr : le noyau garde le fichier
vivant jusqu'à la fin du processus, si bien que la commande se termine et affiche
son résultat normalement. Ce qui est supprimé, c'est le nom dans le système de
fichiers.
