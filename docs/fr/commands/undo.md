# `normfix undo`

Restaure une exécution précédente depuis sa sauvegarde externe, et refuse
d'écraser tout ce qui a changé depuis.

```sh
normfix undo --list
normfix undo
normfix undo --run run-1785950998077000000-53423
```

## Trouver un point de récupération

```console
$ normfix undo --list
normfix undo: 1 recovery point(s)
  run-1785950998077000000-53423  1 file(s)
```

Chaque exécution conserve les octets d'origine exacts et un `journal.json`
prouvant quels fichiers elle a écrits et ce qu'elle y a écrit.

## Restaurer

Sans `--run`, `undo` sélectionne le point de récupération intact le plus récent
et demande confirmation. La restauration non interactive exige `--force` :

```sh
normfix undo --force
```

## Quand il refuse

`undo` échoue fermé. Il ne restaure pas quand :

- un fichier cible ne correspond plus aux octets que cette exécution a écrits,
  parce que quelqu'un l'a modifié ensuite, et restaurer effacerait ce travail en
  silence ;
- un fichier de sauvegarde manque ou son empreinte ne correspond pas au
  journal ;
- un chemin de la sauvegarde ou du projet passe par un lien symbolique ;
- le journal est illisible ou son schéma est inconnu.

Un refus nomme le fichier et la raison. C'est délibéré : un outil de
récupération qui devine est pire qu'un outil qui s'arrête.

## Ce qui n'est pas couvert

Les exécutions avec `--no-backup` ne laissent rien à restaurer, et c'est le prix
de l'absence de sauvegardes. Les opérations destructives conservent toujours un
stockage de récupération quoi qu'il arrive, donc un fichier en quarantaine ou un
commentaire supprimé reste récupérable même quand `--no-backup` a été passé.
