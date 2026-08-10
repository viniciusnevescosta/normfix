# Sécurité, récupération et opérations destructives

`normfix` n’applique automatiquement une modification que lorsque sa preuve
aboutit. Un diagnostic ou une suggestion n’est pas une correction démontrée.
Utilisez l’aperçu avant toute opération destructive :

```sh
normfix --diff --remove-unused
normfix --check --remove-unexpected
```

## Autorisations destructives

Les commentaires invalides sont seulement signalés par défaut.
`--remove-invalid-comments` supprime uniquement le commentaire à la position
exacte donnée par la Norminette officielle et préserve l’en-tête 42. Les
options `--remove-unused`, `--remove-unexpected` et `--unsafe` exigent une
confirmation interactive `y/N` ; en JSON ou autre exécution non interactive,
elles exigent `--force`.

La confirmation n’autorise que la capacité demandée. Chaque candidat doit
encore réussir les preuves du parser, du hash, de la portée et de la
transaction. Une ambiguïté, un fichier illisible, une macro complexe ou un
ensemble de sources incomplet provoque un échec prudent.

## Sauvegardes et undo

Les octets originaux et `journal.json` sont stockés hors du projet :

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
~/.local/share/normfix/backups/<run-id>/
```

Avant d’écrire, le programme refuse les cibles externes, liens symboliques,
fichiers irréguliers, doublons ou fichiers modifiés après l’analyse. Une erreur
d’écriture déclenche rollback. Les suppressions exigent un stockage de
récupération même avec `--no-backup` ; les fichiers inattendus sont déplacés en
quarantaine, jamais supprimés définitivement.

Utilisez `normfix undo` pour restaurer la dernière transaction. Conservez le
chemin du journal affiché si rollback ne peut pas aboutir automatiquement.

`normfix.toml` et les listes de fonctions autorisées complètent, mais ne
remplacent jamais, le subject et l’évaluation officielle de 42.
