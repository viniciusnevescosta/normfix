# Ligne de commande

L'interface sans sous-commande est le chemin le plus court pour formater un
projet. Les sous-commandes rendent l'intention plus claire dans les scripts et
en relecture interactive.

```sh
normfix format src includes
normfix lint
normfix check main.c
normfix budget src
normfix preflight
normfix explain TOO_MANY_LINES
normfix undo --list
normfix undo --run RUN_ID
```

## Flux

| Commande | Écrit des fichiers | Ce qu'elle fait |
|---|---|---|
| `format` | oui | Applique les modifications acceptées |
| `lint` | non | Signale des diagnostics sur les octets d'origine ; ne propose ni mise en forme, ni en-tête, ni Makefile, ni remplacement Markdown |
| `check` | non | Exécute mise en forme et lint dans un tampon fantôme |
| `budget` | non | Une exécution de lint plus une ligne informative lignes/variables/paramètres par fonction analysée |
| `preflight` | non | Une exécution orientée check avec la vérification stricte du compilateur activée ; elle n'exécute ni `make` ni le programme |
| `explain` | non | Affiche l'explication intégrée en anglais d'un identifiant de règle stable, sans analyser un projet |
| `undo` | oui | Liste ou restaure une sauvegarde de transaction intacte |

`undo` refuse d'écraser des octets modifiés après l'exécution qu'il restaure.
Sans `--run`, il choisit le point de récupération valide le plus récent après une
confirmation interactive ; la restauration non interactive exige `--force`.

## Options

| Option | Comportement |
|---|---|
| `PATH...` | Zéro, un ou plusieurs fichiers/répertoires ; zéro signifie le répertoire courant |
| `--check` | Planifie et signale les changements sans écrire |
| `--diff` | Affiche des diffs unifiés dans la sortie humaine sans écrire |
| `--changed` | Sélectionne les changements suivis non indexés plus les fichiers non suivis et non ignorés par Git |
| `--staged` | Sélectionne uniquement les chemins enregistrés comme modifiés dans l'index Git |
| `--interactive` | Prévisualise, montre le diff de chaque fichier modifié et demande lesquels écrire |
| `--use-gitignore` | Respecte `.gitignore` pendant la découverte récursive de répertoires |
| `--login LOGIN` | Fournit ou contraint le login 42 utilisé pour valider l'identité |
| `--email EMAIL` | Fournit l'adresse vérifiée d'étudiant 42 utilisée dans les en-têtes officiels |
| `--no-backup` | Désactive les sauvegardes conservées pour les écritures de mise en forme sûres et ordinaires |
| `--backup-dir PATH` | Utilise une base externe de sauvegarde précise |
| `--format human\|json` | Choisit la sortie terminal ou le rapport JSON versionné |
| `--lang CODE` | Langue de la sortie humaine : `en`, `pt`, `es` ou `fr` |
| `--no-color` | Désactive la couleur ANSI |
| `-v`, `--verbose` | Liste chaque correction acceptée dans la sortie humaine |
| `--timeout SECONDS` | Délai Norminette par invocation ; par défaut : 5 secondes |
| `--threads N` | Nombre de processus parallèles ; par défaut : le matériel disponible |
| `--remove-invalid-comments` | Supprime uniquement les commentaires rejetés à des emplacements officiels exacts |
| `--remove-unused` | Supprime uniquement les fonctions `static` inatteignables prouvées dans un projet complet |
| `--remove-unexpected` | Déplace les fichiers ordinaires inattendus vers une quarantaine externe récupérable |
| `--unsafe` | Active l'ensemble fermé d'actions risquées/destructives |
| `--force` | Confirme les capacités destructives demandées ou accepte une portée protégée |
| `--no-reorder-includes` | Laisse les blocs d'includes contigus dans leur ordre actuel |
| `--no-format-markdown` | Analyse les documents README sans réimpression canonique en CommonMark |
| `--no-cache` | Désactive le cache d'analyse externe persistant |
| `--norminette PATH` | Utilise un exécutable Norminette précis |
| `--strict-norminette-version` | Refuse une version du vérificateur autre que celle testée |
| `--no-compiler-preflight` | Saute la passe consultative stricte du compilateur C, active par défaut |
| `--cc PATH` | Utilise un compilateur C précis pour le preflight et l'analyse |
| `--analyzer` | Ajoute l'analyseur borné GCC/Clang aux flux ordinaires ; le preflight l'active automatiquement |
| `-h`, `--help` | Affiche l'aide intégrée |
| `-V`, `--version` | Affiche la version de la CLI native |

`--check` et `--diff` s'excluent mutuellement. `--changed` et `--staged`
s'excluent mutuellement et ne peuvent pas être combinés avec des chemins
explicites. `--force` sans `--unsafe`, `--remove-unused` ou
`--remove-unexpected` est une erreur, sauf si la portée elle-même est protégée.
Les racines du système de fichiers, le répertoire personnel complet, les racines
larges comme `/Users` et `/home` et les arborescences du système d'exploitation
refusent avant la découverte sans cette acceptation explicite.

## Ordre des includes

Une suite de directives `#include` est réordonnée pour que les en-têtes système
viennent d'abord, puis ceux du projet, par ordre alphabétique dans chaque
catégorie :

```c
# include "libft.h"          # include <limits.h>
# include "ft_printf.h"  ->  # include <stdlib.h>
# include <stdlib.h>         # include "ft_printf.h"
# include <limits.h>         # include "libft.h"
```

::: warning Le bloc doit être contigu de façon prouvable
Une suite n'est réécrite que tant que **chaque** ligne est exactement une
directive d'include. La première ligne qui est autre chose (un commentaire, une
ligne vide, une condition, une définition de macro ou du texte après le
délimiteur final) termine la suite, et chaque côté est trié indépendamment.
Aucune directive ne traverse une telle construction, car le faire peut changer
des déclarations, des macros de fonctionnalité ou la compilation conditionnelle.
:::

Les noms sont comparés sans tenir compte de la casse et les noms identiques
gardent leur ordre relatif d'origine. `--no-reorder-includes` laisse chaque bloc
intact ; le rapport se rabat alors sur l'avertissement `INCLUDE_ORDER_REVIEW`,
que `normfix explain INCLUDE_ORDER_REVIEW` décrit hors ligne.

## Portées Git

La sélection de portée par Git a lieu avant la découverte normale :

```sh
normfix check --changed
normfix format --staged
```

`--changed` signifie les changements suivis non indexés plus les fichiers non
suivis que Git n'ignore pas ; il n'inclut délibérément pas les chemins
uniquement indexés. `--staged` utilise le diff de l'index pour choisir des noms,
puis analyse et formate les octets actuels de l'arbre de travail. Il ne réécrit
pas l'index et n'indexe pas le résultat.

Une portée vide est un no-op réussi et ne se rabat jamais sur un balayage de
répertoire complet. Git est invoqué directement, avec des chemins délimités par
NUL, un délai, une limite de sortie et des vérifications de confinement de
chemin. Les noms absolus ou qui s'échappent sont rejetés. Un candidat qui est un
lien symbolique ou qui n'est pas un fichier ordinaire est omis sans risque ; un
échec de métadonnées ou de Git rejette la portée entière plutôt que de balayer
silencieusement un autre ensemble.

::: tip Une portée n'est pas une preuve
La portée Git est un confort de relecture, pas une preuve de projet complet. Les
constats à l'échelle du projet qui exigent un instantané fermé sont désactivés
quand la portée ne peut pas en fournir un.
:::

## Relecture interactive

```sh
normfix format --interactive
```

La première passe est en lecture seule : `normfix` affiche le rapport et le diff
de chaque fichier proposé, en acceptant `y`, `n`, `a` (tous) ou `q` (annuler). Il
analyse ensuite à nouveau la même portée sélectionnée. Chaque approbation est liée
aux empreintes des octets d'origine et proposés exacts montrés à la première
passe, et la transaction n'écrit que les fichiers dont le plan de la deuxième
passe correspond encore à cette approbation liée à l'instantané.

Le mode interactif exige un terminal humain et ne peut pas être combiné avec
l'aperçu, le JSON, lint/budget ou les opérations risquées/destructives.

## Comportement d'exclusion

Les balayages récursifs respectent `.normfixignore` par défaut, avec le style
d'exclusion Git pris en charge par le crate `ignore`. Le nom historique
`.norminetteignore` reste pris en charge pour que les projets existants ne
retrouvent pas silencieusement des entrées ignorées.

`.gitignore` est délibérément optionnel, via `--use-gitignore`, car des fichiers C
ignorés peuvent quand même intervenir dans des preuves à l'échelle du projet. Les
arguments de fichier explicites restent explicites et ne sont pas filtrés par les
fichiers d'exclusion.

## Codes de sortie

| Code | Signification |
|---:|---|
| `0` | Le mode correction est allé au bout sans diagnostic bloquant, ou l'entrée était déjà propre |
| `1` | Des diagnostics manuels subsistent, ou le mode aperçu a trouvé des changements proposés/candidats à la quarantaine |
| `2` | Échec de découverte, de configuration, d'outil, d'E/S, de transaction ou de quarantaine |
| `130` | Une relecture interactive fichier par fichier a été annulée |

Les avis informatifs ne font pas échouer une exécution.
