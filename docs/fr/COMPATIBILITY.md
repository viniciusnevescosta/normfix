# Politique de compatibilité

Ce document définit ce que `normfix` considère comme pris en charge. Il est
volontairement étroit : les affirmations de compatibilité font partie du modèle
de sûreté et doivent être appuyées par des preuves automatisées.

## Norminette officielle

Le vérificateur testé est la
[Norminette officielle](https://github.com/42School/norminette) `3.3.59`.

`normfix` relève l’empreinte de la version de l’exécutable avant l’analyse. Une
autre version continue, par défaut, avec un avertissement bien visible
`NORMINETTE_VERSION_UNTESTED` ; `--strict-norminette-version` la refuse dans une
CI à version figée. Ce n’est pas une affirmation de compatibilité avec une
version minimale, car les noms des diagnostics officiels, les emplacements, le
comportement de l’analyseur et les dispositions acceptées sont des entrées de la
couche native de compatibilité. L’avertissement rend cette garantie réduite
explicite.

La Norminette reste une dépendance externe. Les archives de release contiennent
le binaire natif de `normfix`, pas Python ni le vérificateur officiel.

### Adopter une autre version du vérificateur

Une mise à jour de la Norminette exige un changement relu qui :

1. consigne les notes de version amont et les changements de noms de règles ;
2. exécute la suite native complète contre la version candidate ;
3. rafraîchit les fixtures de sortie officielle seulement après avoir expliqué
   chaque différence ;
4. vérifie l’idempotence des corrections sûres et l’absence de régression sur
   des projets 42 représentatifs ;
5. met à jour la constante exacte de version, l’installation en CI, le README et
   ce fichier ;
6. est livré comme une nouvelle version de `normfix`.

Prendre en charge une plage de versions n’est approprié qu’après que la CI a
prouvé chaque version de cette plage et que l’oracle dispose d’un adaptateur
explicite pour toute différence de protocole.

### Quand 42 bouge en premier

Un outil qui refuse toutes les versions sauf une cesse de fonctionner pour tout
le monde le jour où l’école met à jour. Le comportement par défaut continue donc
et signale `NORMINETTE_VERSION_UNTESTED` ; une CI figée peut choisir le refus :

```sh
normfix --strict-norminette-version
```

Le comportement par défaut est défendable, et non un trou dans le raisonnement,
car la propriété que l’outil promet réellement ne dépend pas de la connaissance
de la version : la preuve de régression avant/après compare deux réponses du
**même exécutable**, donc une exécution ne peut toujours pas laisser un fichier
avec plus de diagnostics officiels qu’au départ. Ce qu’une version non vérifiée
coûte, c’est la garantie que les règles natives s’accordent avec elle — ce que
l’avertissement dit exactement.

## Toolchain Rust

- Version minimale de [Rust](https://www.rust-lang.org/tools/install) prise en
  charge (MSRV) : `1.85`.
- Toolchain du dépôt et des releases : `1.97.1`, figée dans
  `rust-toolchain.toml`.

La CI vérifie la MSRV indépendamment de la toolchain de développement figée.
Relever la MSRV exige un changement de release documenté, pas une mise à jour
accessoire de dépendance.

## Systèmes d’exploitation et cibles de release

Les releases précompilées couvrent les environnements Unix utilisés par les
étudiants de 42 :

| Système d’exploitation | Architecture | Archive publique de release |
|---|---|---|
| Linux | x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux | ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS | Intel | `normfix-x86_64-macos.tar.gz` |
| macOS | Apple Silicon | `normfix-aarch64-macos.tar.gz` |
| Windows | x86-64 | `normfix-x86_64-windows.zip` |
| Windows | ARM64 | `normfix-aarch64-windows.zip` |
| FreeBSD | x86-64 | `normfix-x86_64-freebsd.tar.gz` |

Les noms publics des archives omettent délibérément les marqueurs de fournisseur
Rust et les étiquettes de constructeur de machine. Les identifiants de cible de
la toolchain restent des entrées internes de compilation, pas des noms de
release ni de produit.

Windows est pris en charge nativement depuis la 1.4.0, sur les preuves que la CI
produit pour lui plutôt que sur l’hypothèse qu’un code portable se porte. Les
deux cibles Windows exécutent la suite complète, pilotent la vraie Norminette
officielle et démontrent la propriété différentielle — une exécution ne laisse
jamais un fichier avec plus de diagnostics officiels qu’au départ — sur la
plateforme elle-même.

Deux différences avec Unix sont réelles, et sont dites ici plutôt que lissées :

- **Le confinement des processus a une fenêtre étroite.** Unix place l’outil dans
  son propre groupe de processus entre le fork et l’exec : aucun descendant ne
  peut s’échapper. Windows n’a pas de point d’accroche avant le démarrage :
  l’outil rejoint un job object juste après le spawn, et ce qu’il crée dans les
  microsecondes précédentes pourrait s’en détacher. Le job tue le reste de
  l’arborescence à sa fermeture.
- **Un rename n’est pas write-through.** POSIX exige de synchroniser le
  répertoire parent pour qu’une création ou un renommage survive à une panne, ce
  que la transaction fait. Windows n’a pas d’équivalent ; le contenu du fichier
  est synchronisé et NTFS journalise les métadonnées, mais une machine qui perd
  le courant entre le commit et l’écriture des métadonnées a une garantie plus
  faible que le même instant sous Unix. La sauvegarde et le journal ne sont pas
  concernés : la récupération les lit par contenu, pas par ordre.

Les archives Windows sont des `.zip`, que la plateforme ouvre d’elle-même.
L’installateur en une ligne y fonctionne depuis n’importe quel shell POSIX — Git
Bash, MSYS2, Cygwin ou WSL. Exécuter la compilation Linux dans WSL reste pris en
charge et inchangé.

FreeBSD x86-64 est pris en charge aux mêmes conditions. C’est un Unix : il
partage le confinement par groupe de processus et la synchronisation de
répertoire au lieu d’avoir besoin des substituts de Windows, et la CI exécute la
suite complète, le vérificateur officiel et la preuve différentielle dans une
machine virtuelle FreeBSD — GitHub n’a pas de runner FreeBSD, et une compilation
croisée publierait un binaire qui n’a jamais tourné sur le système visé. Son
archive de release est construite dans cette même machine virtuelle, pour la même
raison.

FreeBSD en ARM64 n’est pas publié. `aarch64-unknown-freebsd` n’a pas de
bibliothèque standard précompilée sur la toolchain figée : le construire
exigerait un compilateur nightly non figé, et il n’existe aucun moyen d’y
exécuter la suite. L’un ou l’autre suffirait à rendre l’affirmation
intenable.

## Diagnostics C et de compilation

La Norminette officielle fait autorité pour la compatibilité de style. Un
compilateur C du système s’exécute par défaut comme oracle distinct, uniquement
de diagnostic, pour `-fsyntax-only -Wall -Wextra -Werror`. Les chemins d’include
déduits des répertoires de headers ne remplacent pas les options du Makefile du
projet, ses defines, ses entrées générées, son mode de langage, ses entrées
d’édition de liens ou ses tests d’exécution.

Le `-fanalyzer` de GCC est automatique dans `preflight` et optionnel dans les
déroulés habituels. Ses résultats sur la durée de vie des allocations et le flux
de contrôle peuvent suggérer une fuite possible ou un accès invalide, mais ils ne
prouvent ni qu’un comportement C arbitraire est correct, ni qu’un projet est sans
fuite.

`normfix preflight` n’exécute pas les recettes Make, n’édite pas les liens d’un
binaire et ne lance ni le programme ni les tests. Il signale explicitement ces
étapes manuelles restantes.

`normfix leaks` exécute bien un programme, et c’est la seule commande qui le
fait. Elle n’en compile jamais : elle exécute un binaire qu’on lui désigne, sous
un détecteur de fuites localisé dans le `PATH` et vérifié par son propre
`--version`. Ce qu’elle rapporte est ce qu’une exécution a observé sur un
chemin, jamais une preuve qu’un programme ne fuit pas, et une sortie qu’elle ne
peut pas lire comme un résumé de fuites est une erreur, pas un résultat propre.
Valgrind couvre Linux et FreeBSD directement, macOS via un port communautaire à
la prise en charge limitée d’Apple Silicon, et Windows via WSL.

## Compatibilité navigateur

Le playground vise les navigateurs modernes disposant du support standard de
WebAssembly et des modules ES. Son interface HTML/CSS/TypeScript volontairement
petite et à l’ancienne est construite en site statique avec
[Vite 8.2.1](https://vite.dev/releases) figé, et peut être servie localement ou
par Vercel. Son contrat de compatibilité est le sous-ensemble natif de formatage
et de diagnostic en mémoire décrit dans
[`web/README.md`](https://github.com/viniciusnevescosta/normfix/blob/main/web/README.md).
Il peut construire un en-tête officiel à partir d’une identité fournie à cet
onglet, et prévisualiser du C, des headers, des Makefiles et du Markdown. Il
n’embarque ni n’émule la Norminette, un compilateur, Git, les preuves de gardes
d’en-tête à l’échelle du projet ou les transactions du système de fichiers.

## Compatibilité du rapport

L’interface humaine regroupe les diagnostics pour la lisibilité et peut
s’améliorer d’une version à l’autre. L’automatisation doit utiliser
`--format json` et vérifier `schema_version` ; le JSON conserve les résultats
individuels. Une structure JSON incompatible exige d’incrémenter la version du
schéma et d’ajouter des notes de compatibilité.

Une conséquence mérite d’être dite franchement : la ligne et la colonne affichées
à côté d’un extrait suivent la convention du compilateur C et comptent des
caractères, tandis que la Norminette officielle compte des colonnes d’affichage.
Les deux divergent sur une ligne indentée par tabulation. Aucun de ces nombres ne
fait partie de la surface versionnée, et c’est le caret sous la source qui situe
le résultat. Voyez
[Rapports](/fr/reference/reporting#lire-un-diagnostic).

## Ce que couvre le versionnage

`normfix` suit le versionnage sémantique. Le numéro de version décrit les
surfaces suivantes, et uniquement celles-ci :

| Surface | Couverte | Ce que signifie une rupture |
|---|---|---|
| Options et sous-commandes de la ligne de commande | oui | En supprimer ou en renommer une, ou changer ce que fait une option existante |
| Codes de sortie | oui | Changer le sens de `0`, `1`, `2` ou `130` |
| Structure du rapport JSON | oui, via `schema_version` | Supprimer un champ ou en changer le type |
| Fichiers de configuration (`normfix.toml`, `.normfixignore`) | oui | Changer l’interprétation d’une clé ou d’un motif existant |
| Disposition des sauvegardes, du journal et de la quarantaine | oui | Rendre un ancien point de récupération illisible pour `undo` |
| Quelles sources sont modifiées automatiquement | non | Les nouvelles éditions prouvées arrivent en versions mineures |
| Formulation, regroupement et texte d’aide des diagnostics | non | Améliorés en continu |
| API des crates Rust | non | Chaque crate déclare `publish = false` et est interne |
| La version de la Norminette prise en charge | à part | La changer est un changement de release documenté, jamais accessoire |

Une nouvelle édition automatique donne une version mineure, car un formateur dont
la sortie ne changerait jamais ne vaudrait pas la peine d’être lancé. Une
exécution qui produit un résultat officiel *pire* est un bug dans n’importe quelle
version, et le test différentiel existe précisément pour l’attraper.

La version minimale de Rust prise en charge est une décision de release, pas un
détail de compilation. La relever exige un changement documenté ; une dépendance
qui réclame un compilateur plus récent est retenue à la place.
