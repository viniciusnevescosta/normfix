# Toutes les options

Chaque entrée dit ce que fait l'option, quand vous y auriez recours, et la montre
en usage. Les options sont globales : elles fonctionnent avec la commande nue et
avec chaque sous-commande.

Lancez `normfix --help` pour la même liste sans la prose.

## Choisir ce qui est traité

### `PATH...`

Zéro, un ou plusieurs fichiers et répertoires. Zéro signifie le répertoire
courant, balayé récursivement sans suivre les liens symboliques.

```sh
normfix                                   # the whole project
normfix main.c                            # one file
normfix src includes                      # two directories
normfix src/parser.c includes/shell.h     # a mixture
```

Un argument de fichier explicite est toujours traité, même si un fichier
d'exclusion l'aurait écarté.

### `--changed`

Traite les changements suivis non indexés plus les fichiers non suivis que Git
n'ignore pas.

```sh
normfix --changed
```

Utilisez-la pendant le travail : elle formate ce que vous venez de toucher au
lieu de réécrire tout le projet. Elle exclut délibérément les chemins uniquement
indexés.

### `--staged`

Traite uniquement les chemins enregistrés comme modifiés dans l'index Git.

```sh
normfix check --staged
```

Elle lit l'index pour choisir des *noms*, puis analyse les octets actuels de
l'arbre de travail. Elle ne réécrit pas l'index et n'indexe pas le résultat, si
bien que `git diff --staged` n'est pas affecté.

Ne peut pas être combinée avec `--changed` ni avec des chemins explicites. Une
portée vide est un no-op réussi, et elle ne se rabat jamais sur un balayage
complet.

### `--use-gitignore`

Respecte aussi `.gitignore` pendant la découverte récursive.

```sh
normfix --use-gitignore
```

Désactivée par défaut, délibérément : un fichier C que vous avez demandé à Git
d'ignorer participe quand même aux preuves à l'échelle du projet, comme la
vérification des fonctions autorisées. `.normfixignore` est toujours respecté.

## Prévisualiser plutôt qu'écrire

### `--check`

Planifie tout, n'écrit rien.

```sh
normfix --check
normfix --check --format json > report.json
```

Le code de sortie `1` signifie qu'il reste du travail, ce qui en fait un
garde-fou de CI d'une seule ligne.

### `--diff`

Affiche un diff unifié de chaque changement proposé, et n'écrit rien.

```sh
normfix --diff
normfix --diff src/parser.c
```

Les tabulations sont rendues par `\t` pour que les changements d'indentation
restent visibles. S'exclut mutuellement avec `--check`.

### `--interactive`

Prévisualise chaque fichier modifié et choisit lesquels sont écrits.

```sh
normfix format --interactive
```

Répondez `y`, `n`, `a` (tous) ou `q` (annuler). L'approbation est liée aux octets
exacts qui vous ont été montrés ; si un fichier change sous vos pieds, il est
ignoré plutôt qu'écrit. Exige un terminal, et refuse de se combiner avec
`--check`, `--diff`, la sortie JSON ou les options destructives.

## Identité pour les en-têtes officiels

### `--login LOGIN`

Fournit ou contraint le login 42 utilisé dans l'en-tête officiel.

```sh
normfix --login vneves-c
```

### `--email EMAIL`

Fournit l'adresse vérifiée d'étudiant 42. L'adresse est la source de vérité ; le
login est validé face à elle.

```sh
normfix --email vneves-c@student.42.fr
```

Sans l'une ou l'autre option, `normfix` résout l'identité depuis votre
environnement et la configuration Git, et pose la question de façon interactive
quand il ne peut pas et que l'exécution en a besoin. Une identité valide fournie
explicitement, ou une réponse valide à cette invite, est enregistrée
atomiquement dans la configuration privée par utilisateur de la plateforme, pour
que les exécutions suivantes ne redemandent pas. Voir
[en-têtes officiels](/fr/reference/headers) pour les chemins et les permissions.

## Sauvegardes et récupération

### `--no-backup`

Saute les sauvegardes conservées pour les écritures ordinaires de mise en forme.

```sh
normfix --no-backup
```

Elle ne saute **pas** la récupération d'une suppression destructive. Celles-ci
exigent toujours un stockage externe et échouent fermées sans lui. Sauter les
sauvegardes signifie que [`undo`](/fr/commands/undo) n'a rien à restaurer pour
cette exécution.

### `--backup-dir PATH`

Utilise une base externe de sauvegarde précise au lieu de celle par défaut sous
`$XDG_DATA_HOME`.

```sh
normfix --backup-dir ~/normfix-backups
```

Le répertoire ne doit pas chevaucher le projet. Un chemin à l'intérieur, ou
au-dessus, est refusé, avant et après résolution des liens symboliques.

## Sortie

### `--format human|json`

Sortie terminal, ou rapport JSON versionné.

```sh
normfix --check --format json | jq '.summary'
```

Branchez toujours sur `schema_version` avant de lire le JSON. La mise en page
humaine peut s'améliorer d'une version à l'autre ; la structure du JSON, non.

### `--lang`

Choisit la langue de la sortie humaine : `en`, `pt`, `es` ou `fr`.

```sh
normfix check --lang fr
```

```console
$ normfix check --lang fr
normfix · démarrage
  action            check
  mode              lecture seule
  portée            /home/student/demo (récursif)
...
Résumé : fichiers : 1 | proposés : 1 | écrits : 0 | corrections : 1 | restants : 0 | informatifs : 0 | en échec : 0 | inattendus : 0 | 0 en quarantaine
Terminé en 219 ms.
```

Sans l'option, la locale du processus est utilisée — `NORMFIX_LANG`, puis
`LC_ALL`, `LC_MESSAGES` et `LANG` — avec repli sur l'anglais. Seul le sous-tag
principal compte, donc `fr_FR.UTF-8` sélectionne le français. Une valeur de
`--lang` non publiée continue en anglais avec un avis plutôt que d'échouer.

Cela ne change que les explications. Les noms de commandes, l'orthographe des
options, les identifiants de règle, les codes de sortie et chaque valeur de
`--format json` restent identiques dans les quatre langues, si bien qu'un script
n'a jamais à choisir une langue pour continuer à fonctionner.

Les messages de règle des analyseurs sont encore en anglais. Une exécution non
anglaise le dit en une ligne plutôt que de présenter un rapport partiellement
traduit comme s'il était complet.

### `--no-color`

Désactive les couleurs ANSI même sur un terminal.

```sh
normfix --no-color
```

Les couleurs sont déjà désactivées quand la sortie n'est pas un terminal, ou
quand `NO_COLOR` est défini.

### `-v`, `--verbose`

Liste chaque correction acceptée au lieu du seul décompte.

```sh
normfix --check -v
```

Utile quand vous voulez savoir exactement quelles dix-sept corrections un fichier
a reçues.

## Exécution

### `--threads N`

Fixe le nombre de processus parallèles. Par défaut, le matériel disponible.

```sh
normfix --threads 1
```

Utilisez `1` pour rendre l'ordre de sortie trivialement reproductible pendant un
débogage. Les résultats et les écritures sont triés par chemin de toute façon,
donc le nombre de processus ne change jamais le rapport ni l'ordre d'écriture des
fichiers.

### `--timeout SECONDS`

Délai Norminette par fichier. Par défaut `5`.

```sh
normfix --timeout 15
```

Augmentez-le sur une machine lente ou un très gros fichier. Un délai dépassé est
un échec opérationnel pour ce fichier, pas un diagnostic.

### `--no-cache`

Désactive le cache d'analyse externe.

```sh
normfix --no-cache
```

Le cache conserve les résultats du vérificateur officiel hors du projet, indexés
par les octets de la source et l'empreinte vérifiée du vérificateur.
Désactivez-le pour forcer une réexécution complète ; un échec de cache échoue
déjà ouvert comme une absence.

### `--norminette PATH`

Utilise un exécutable Norminette précis au lieu de chercher dans le `PATH`.

```sh
normfix --norminette ~/.local/pipx/venvs/norminette/bin/norminette
```

L'empreinte de la version est prise. La version `3.3.59` est celle testée ; une
autre version analysable continue avec un avis marqué
`NORMINETTE_VERSION_UNTESTED`.

## Vérifications du compilateur

### `--strict-norminette-version`

Refuse une version de Norminette face à laquelle cette version n'a pas été
vérifiée.

```sh
normfix --strict-norminette-version
```

Le comportement par défaut continue de fonctionner quand un campus installe une
version officielle plus récente, tout en nommant l'écart de compatibilité. Le
mode strict est utile pour une CI reproductible qui fige délibérément la
`3.3.59`. L'ancienne orthographe `--allow-untested-norminette` demeure comme un
no-op masqué pendant la transition des versions candidates.

### `--no-compiler-preflight`

Saute la passe stricte `cc -fsyntax-only -Wall -Wextra -Werror`.

```sh
normfix --no-compiler-preflight
```

La passe est active par défaut et purement diagnostique : elle n'autorise ni ne
rejette jamais une modification du formateur. Sautez-la quand votre projet a
besoin d'options de compilation que le contexte déduit ne peut pas fournir, et
que le bruit n'est pas utile.

### `--cc PATH`

Utilise un compilateur précis pour la passe stricte de syntaxe et pour
l'analyseur profond. L'analyseur est automatique dans `preflight` ; les flux
ordinaires exigent `--analyzer`.

```sh
normfix --cc /usr/bin/gcc-14
```

Le compilateur est identifié par sa bannière de version, si bien qu'une commande
nommée `gcc` qui est en réalité Clang est traitée comme Clang.

### `--analyzer`

Lance en plus l'analyseur statique profond fourni par votre compilateur pendant
un flux ordinaire. `preflight` active déjà cette passe bornée automatiquement.

```sh
normfix --analyzer
```

`normfix` choisit les options d'après la bannière de version du compilateur, pas
d'après le nom de la commande :

| Compilateur | Ce qui s'exécute |
|---|---|
| GCC | `-fanalyzer` |
| Clang | `--analyze -Xclang -analyzer-output=text` |
| Autre chose | Rien ; l'exécution signale `CC_ANALYZER_UNAVAILABLE` et continue |

::: warning `/usr/bin/gcc` sur macOS est Clang
Apple fournit une commande `gcc` qui répond `Apple clang version ...`. La choisir
avec `--cc` ne vous donne pas `-fanalyzer`. `normfix` le détecte et utilise
l'analyseur Clang, donc l'option fait ce que vous vouliez dire dans les deux cas.
:::

Les deux analyseurs sont plus lents et informatifs. Ils sont automatiques dans
`preflight` et optionnels ailleurs. Ils peuvent suggérer une fuite ou un accès
invalide le long d'un chemin ; aucun n'est une preuve de l'un ou de l'autre, et
aucun n'est jamais une preuve de leur absence. Un analyseur absent ne change
jamais le code de sortie.

Pour un vrai GCC sur macOS, installez-en un et pointez dessus explicitement :

```sh
brew install gcc
normfix preflight --cc "$(brew --prefix)/bin/gcc-14"
```

## Contenu qui est réécrit

### `--no-reorder-includes`

Laisse les blocs contigus de `#include` dans leur ordre actuel.

```sh
normfix --no-reorder-includes
```

Par défaut, une suite de directives d'include est triée avec les en-têtes système
d'abord, puis ceux du projet, par ordre alphabétique dans chacun. Un bloc n'est
réécrit que tant que chaque ligne est exactement une directive d'include, si bien
qu'un commentaire ou une condition termine la suite et que rien ne la traverse.

### `--no-format-markdown`

Laisse les documents README inchangés.

```sh
normfix --no-format-markdown
```

Les fichiers README sont réimprimés en CommonMark canonique par défaut. Cela peut
produire un large diff à la première exécution, ce qui est la raison habituelle
de le désactiver.

Le document est lu dans le dialecte dans lequel il a été écrit : les listes de
tâches, les notes de bas de page, les tableaux et le texte barré reviennent tels
quels. Lus comme du CommonMark simple, ce serait du texte ordinaire, et la
réimpression échapperait leurs crochets : `- [x] fait` reviendrait en
`- \[x\] fait` littéral.

## Opérations destructives

Chacune de celles-ci supprime ou déplace quelque chose. Toutes conservent un
stockage externe récupérable, et toutes exigent une confirmation.

### `--remove-invalid-comments`

Supprime uniquement les commentaires que le vérificateur officiel a rejetés à des
emplacements exacts.

```sh
normfix --remove-invalid-comments
```

Rien d'autre n'est touché : un commentaire que le vérificateur accepte n'est
jamais supprimé.

### `--remove-unused`

Supprime les fonctions `static` prouvées inatteignables dans le projet complet.

```sh
normfix --remove-unused
```

La preuve exige que chaque source du projet soit lisible et sans ambiguïté. Un
seul fichier illisible désactive toute l'analyse au lieu de produire une réponse
partielle.

### `--remove-unexpected`

Déplace les fichiers ordinaires inattendus vers la quarantaine externe.

```sh
normfix --remove-unexpected
```

Rien n'est supprimé : les fichiers sont déplacés vers le stockage de récupération
en conservant leur chemin relatif, et une destination existante n'est jamais
écrasée.

### `--unsafe`

Active l'ensemble fermé ci-dessus, plus le compactage des comparaisons avec NULL,
la suppression des sources obsolètes du Makefile et l'effacement d'une variable
locale que rien ne lit.

Cette dernière est refusée dès que la déclaration contient quelque chose qui
s'exécute. `int n = g();` est un appel, et un `malloc` à cet endroit verrait sa
fuite réparée par accident — dans un programme que vous n'avez pas écrit. Ces
cas sont signalés.

```sh
normfix --unsafe
```

C'est un ensemble nommé, pas un interrupteur ouvert. Il ne peut pas activer une
opération qui n'existe pas déjà comme sa propre option.

### `--force`

Confirme les opérations destructives sans invite, ou accepte explicitement une
portée système/large protégée.

```sh
normfix --unsafe --force
```

Pour la CI et les scripts. `--force` seule, sans aucune option destructive, est
une erreur sauf si la portée choisie est protégée. Accepter une portée protégée
ne crée aucune capacité destructive ; celles-ci exigent toujours leurs propres
options.

## Environnement

### `NORMFIX_NO_UPDATE_CHECK`

Désactive l'avis quotidien de version.

```sh
export NORMFIX_NO_UPDATE_CHECK=1
```

L'avis n'apparaît que pour la sortie humaine interactive et est silencieux en cas
d'échec. Voir [`upgrade`](/fr/commands/upgrade) pour savoir exactement ce qu'il
envoie.

## Information

### `-h`, `--help`

```sh
normfix --help
normfix undo --help
```

### `-V`, `--version`

```sh
normfix --version
```
