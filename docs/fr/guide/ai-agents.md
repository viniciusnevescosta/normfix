# Utiliser normfix depuis un agent d'IA

Cette page est le contrat opérationnel pour les agents de code, les agents
d'éditeur, les bots de CI et les autres appelants non humains. Elle empêche un
agent de transformer par accident une vérification d'état en écriture récursive.

## La seule règle à retenir

La commande nue formate le répertoire courant de façon récursive :

```sh
normfix
```

Un agent devrait donc commencer par un chemin explicite et une commande en
lecture seule :

```sh
normfix check /chemin/absolu/vers/le/projet --format json --no-color
```

Utilisez un chemin absolu de projet. Ne comptez pas sur un répertoire de travail
hérité, surtout quand l'agent a pu démarrer dans un répertoire personnel, dans
le parent d'un clone, à la racine d'un espace de travail monté ou dans un
répertoire système.

## Vérification des capacités

Avant la première exécution sur un projet, notez les versions de l'outil et du
vérificateur :

```sh
normfix --version
norminette --version
normfix --help
```

`normfix` prend l'empreinte de chaque vérificateur. Quand 42 publie une version
différente, l'exécution par défaut continue et émet
`NORMINETTE_VERSION_UNTESTED` ; un agent doit exposer cette garantie réduite.
N'utilisez `--strict-norminette-version` que lorsque la personne ou la politique
de CI exige explicitement la version testée du vérificateur.

Au démarrage, le mode humain écrit un bloc action/configuration sans couleur sur
`stderr`. Le mode JSON écrit un événement JSON `execution_start` sur `stderr` et
garde le rapport final versionné comme unique document JSON sur `stdout`. Aucun
des deux modes ne pose de question quand stdin n'est pas interactif.

Lisez la portée dans cet événement avant de faire quoi que ce soit du résultat.
C'est la déclaration de l'exécution elle-même sur ce qu'elle allait toucher, ce
qui permet à un agent d'interrompre une exécution dont la portée ne correspond
pas à la tâche reçue, au lieu de découvrir l'écart dans le résumé.

Une portée large ou sensible du système d'exploitation est refusée avant qu'un
seul fichier ne soit lu :

```console
$ normfix check /
normfix
error: refusing to scan or modify protected scope `/` because it is a filesystem root; inspect the path and pass --force to acknowledge it explicitly
No unvalidated changes were written.
```

C'est une sortie `2` sans rapport JSON sur `stdout`. Les racines du système de
fichiers, les répertoires personnels complets, les arborescences du système
d'exploitation et les répertoires larges contenant plusieurs projets refusent
tous ainsi, et la vérification résout d'abord les liens symboliques et `..`.
N'ajoutez pas `--force` pour faire disparaître le message : le refus signifie
presque toujours que la portée a été mal calculée, et `--force` est une décision
que la personne prend sur un chemin qu'elle a inspecté.

Le formateur ordinaire n'a pas besoin de Rust. Un compilateur ne sert qu'aux
vérifications consultatives de preflight ; ses constats n'autorisent jamais une
modification.

## Flux recommandé pour un agent

1. Inspectez l'état du dépôt et résolvez tout conflit de fusion avant de
   formater.
2. Lancez un aperçu lisible par une machine sur une portée explicite.
3. Lisez `schema_version` avant de consommer des champs du rapport JSON.
4. Montrez à la personne les fichiers proposés, les diagnostics restants et tout
   échec opérationnel.
5. Si les écritures sont déjà autorisées, lancez la même portée explicite avec
   `normfix format`.
6. Inspectez le diff obtenu et lancez la compilation/les tests du projet.
7. Relancez `normfix check`. Une deuxième passe réussie ne devrait proposer
   aucune modification.

```sh
project=/chemin/absolu/vers/le/projet
normfix check "$project" --format json --no-color > normfix-report.json
normfix format "$project" --no-color
git -C "$project" diff --check
normfix check "$project" --format json --no-color
```

Ne créez pas `normfix-report.json` dans un répertoire de rendu 42 sauf si la
personne le souhaite : un fichier inattendu est lui-même un constat
d'évaluation. Utilisez plutôt un répertoire de sortie temporaire ou propre à
l'agent.

## Lire le contrat JSON

Le rapport stable utilise actuellement `schema_version: 2`. Champs utiles :

| Champ | Décision de l'agent |
|---|---|
| `summary.changed` | Un aperçu a trouvé des changements d'octets qu'il peut prouver sûrs |
| `summary.remaining` | Des constats manuels/bloquants subsistent |
| `summary.failed` | Une opération d'outil, de découverte, d'E/S ou de transaction a échoué |
| `summary.unexpected_files` | Des fichiers hors de l'ensemble accepté de fichiers de projet ont été trouvés |
| `files[].failure` | Ce fichier n'a pas été terminé ; ne le décrivez pas comme corrigé |
| `files[].after` | Diagnostics sur les octets proposés |
| `files[].fixes` | Modifications prouvées proposées ou écrites pour ce fichier |
| `identity.available` | Un en-tête officiel 42 peut être créé ou mis à jour |
| `evaluation.conclusive` | Toujours `false` ; ne présentez jamais l'estimation comme une note officielle |
| `evaluation.verdict` | `hard_fail` signifie qu'une règle objective de rejet du preflight s'applique |
| `evaluation.hard_failures` | Indices exacts chemin/ligne/colonne/règle à montrer en premier |

Les tampons de source et les diffs sont volontairement absents du JSON. Utilisez
`normfix --diff /chemin/absolu` quand un correctif lisible par un humain est
nécessaire.

Le code de sortie fait partie de l'API :

| Code | Signification |
|---:|---|
| `0` | Propre, ou une écriture terminée sans problème bloquant |
| `1` | Un aperçu a trouvé du travail, ou un constat manuel subsiste |
| `2` | L'exécution elle-même a échoué |
| `130` | Une personne a annulé la relecture interactive |

La sortie `1` n'est pas un plantage opérationnel. La sortie `2` ne doit jamais
être masquée derrière l'affirmation que le projet a réussi.

## Choisir une commande

| Objectif | Commande |
|---|---|
| Aperçu exact | `normfix --diff PATH` |
| Garde-fou machine | `normfix check PATH --format json --no-color` |
| Diagnostiquer les octets sans modifier | `normfix lint PATH --format json --no-color` |
| Relecture avant soutenance | `normfix preflight PATH --format json --no-color` |
| Marge des fonctions | `normfix budget PATH --format json --no-color` |
| Expliquer une règle hors ligne | `normfix explain RULE` |
| Formater une portée autorisée | `normfix format PATH --no-color` |
| Restaurer une transaction normfix | `normfix undo --list`, puis `normfix undo --run ID` |

`--changed` et `--staged` sont pratiques pour l'arbre de travail de la personne
qui développe, mais ils choisissent des noms via Git et analysent les octets de
l'arbre de travail. Utilisez un chemin explicite pour une évaluation complète et
une portée Git pour une modification ciblée.

## Autorité et options destructives

Ces options demandent des capacités matériellement différentes :

- `--remove-invalid-comments` supprime uniquement les commentaires rejetés à des
  emplacements officiels exacts ;
- `--remove-unused` supprime uniquement les fonctions `static` inatteignables
  sous une preuve fermée de projet ;
- `--remove-unexpected` déplace des fichiers vers une quarantaine externe
  récupérable ;
- `--unsafe` active l'ensemble fermé et documenté de nettoyages destructifs ;
- `--force` fournit la confirmation non interactive de ces capacités.

Un agent ne doit pas en déduire l'autorisation à partir d'une demande de
vérifier, formater, évaluer ou « corriger les erreurs de Norme ». Prévisualiser
un plan destructif exige aussi la capacité, car l'analyse est volontairement
conditionnée à l'autorisation.

Ne supprimez jamais des données de sauvegarde ou de quarantaine pour qu'un
rapport paraisse propre. Utilisez `normfix undo` pour récupérer, et signalez le
chemin du journal si la restauration demande une relecture manuelle.

## Limites de l'évaluation

`preflight` combine le résultat officiel de la Norme, les vérifications de
fichiers de projet, les diagnostics stricts du compilateur, les vérifications de
politique et une passe automatique et bornée de l'analyseur du compilateur.
C'est une aide solide à la relecture, pas une note 42 concluante. Il ne connaît
pas le PDF du sujet, n'exécute pas de liste de contrôle de soutenance, ne prouve
pas la justesse algorithmique et ne prouve pas l'absence de fuites. Il n'exécute
ni recettes Make, ni binaire produit, ni `clang-tidy`, ni sanitizers. Lancez le
Makefile du projet, ses tests, la compilation avec sanitizer et le testeur propre
au sujet séparément, et uniquement quand la personne autorise l'exécution de ce
projet.

Ne traitez pas la présence ou l'absence d'un README comme une règle universelle
de réussite/échec. Quand il en existe un, vérifiez-le au regard des sections
exigées par le sujet actuel. De même, `MAKEFILE_NOT_FOUND` est consultatif tant
que la politique du sujet n'a pas prouvé qu'un Makefile est exigé. Ne signalez
pas une correction proposée dans l'ombre comme une réussite du preflight :
l'évaluation échoue sur les constats d'origine de Norminette et du Makefile
présents sur le disque.

## Hygiène de terminal et de CI

- Préférez `--format json --no-color` pour les analyseurs et la sortie
  redirigée.
- N'analysez jamais le tableau humain décoratif quand le JSON est disponible.
- Définissez `NORMFIX_NO_UPDATE_CHECK=1` en CI hermétique ou sans réseau.
- Gardez les versions du vérificateur officiel et de `normfix` dans les journaux
  de CI.
- Ne faites pas passer une commande d'écriture par un filtre qui masque son code
  de sortie.
- N'exécutez pas sur `/`, `/System`, `/usr`, `/etc`, un répertoire personnel ou
  un espace de travail contenant plusieurs projets. Sélectionnez la vraie racine
  du rendu.

Pour chaque option et chaque limite de preuve, poursuivez avec
[Toutes les options](/fr/reference/flags),
[Sûreté et récupération](/fr/reference/safety),
[Rapports](/fr/reference/reporting) et [Architecture](/fr/ARCHITECTURE).
