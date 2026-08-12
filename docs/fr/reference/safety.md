# Sûreté, récupération et opérations destructives

## Chaque exécution annonce ce qu’elle va faire

Avant de lire le moindre fichier, `normfix` affiche l’action, la portée résolue
et la configuration de sûreté réellement en vigueur :

```console
$ normfix --unsafe --force
normfix · starting
  action       format
  mode         write
  scope        /home/student/demo (recursive)
  working dir  /home/student/demo
  identity     student@student.42.fr (user config)
  workers      auto
  checks       Norminette + strict compiler
  norminette   automatic PATH discovery
  version rule advisory (other releases continue)
  timeout      5s per file
  cache        enabled
  gitignore    not applied
  backups      automatic external backup
  destructive  invalid comments, NULL-check compaction, missing or trivia-only Makefile entries, orphan header prototypes, unreachable static functions, unexpected-file quarantine
  force        acknowledged
```

La ligne `destructive` nomme chaque capacité que l’exécution détient réellement :
`--unsafe` ne s’élargit donc jamais en silence.

La ligne `scope` est celle qu’il faut lire. Une commande tapée dans le mauvais
répertoire a l’air fausse ici, avant que rien ne soit touché, plutôt que dans le
résumé d’après. Avec `--format json`, cette même information est le premier
événement sur la sortie standard : un agent peut donc refuser une exécution dont
la portée n’était pas celle qu’il visait.

## Portées protégées

Les racines du système de fichiers, les répertoires personnels complets, les
arborescences du système d’exploitation et les répertoires larges regroupant
plusieurs projets sont refusés d’emblée :

```console
$ normfix check /
normfix
error: refusing to scan or modify protected scope `/` because it is a filesystem root; inspect the path and pass --force to acknowledge it explicitly
No unvalidated changes were written.

$ normfix check ~
normfix
error: refusing to scan or modify protected scope `/home/student` because it is the complete user home directory; inspect the path and pass --force to acknowledge it explicitly
No unvalidated changes were written.
```

Les deux se terminent avec le statut `2` et ne lisent rien. Le contrôle résout
d’abord les liens symboliques et réduit les `..`, si bien qu’un chemin comme
`/work/../etc` ou un lien pointant vers `/etc` est refusé pour la même raison
qu’un `/etc` littéral. Une exécution avec portée Git est jugée sur la racine du
dépôt et non sur les fichiers qu’elle sélectionne : `--git-changed` depuis un
répertoire personnel est donc refusé au lieu de parcourir discrètement tous les
projets qu’il contient.

`--force` reconnaît une portée protégée, et rien d’autre. Il n’accorde pas de
capacité destructive par lui-même, et une capacité destructive exige toujours sa
propre option :

```console
$ normfix --force
normfix
error: --force requires --unsafe, --remove-unused, --remove-unexpected, or a protected system scope
```

## Listes de fonctions autorisées

Les projets ayant une liste de fonctions autorisées propre au sujet peuvent
ajouter un `normfix.toml` à la racine du projet :

```toml
[project]
name = "get_next_line"
allowed = ["read", "malloc", "free"]
```

L’analyseur borné n’interprète volontairement que le `name` entre guillemets et
le tableau `allowed` d’identifiants entre guillemets. Lorsqu’une portée C/headers
est sélectionnée, `normfix` découvre lui-même l’ensemble complet des fichiers
C/header du projet depuis sa racine, en considérant les fichiers réguliers sans
suivre les liens symboliques et avec les filtres `.gitignore`, `.normfixignore`
et `.norminetteignore` désactivés. Chaque fichier découvert doit être un UTF-8
lisible et s’analyser sans perte. Les définitions non `static` de cet instantané
fermé autorisent les appels entre unités de traduction ; les définitions du même
fichier sont traitées localement, tandis qu’une définition `static` dans un autre
fichier n’autorise jamais l’appel.

Les appels candidats sont recalculés contre la source fantôme finale, afin que
les plages signalées restent correctes après l’insertion de l’en-tête et le
formatage. Les paramètres, les appels par pointeur de fonction, l’ambiguïté de
macro ou de préprocesseur et les identifiants en majuscules ressemblant à des
macros échouent en position fermée plutôt que de produire une supposition. Si la
découverte, la lecture, l’analyse, l’absence de perte ou la revalidation de
l’instantané reste incomplète, tous les résultats liés à la liste autorisée sont
désactivés et `FUNCTION_POLICY_PROOF_INCOMPLETE` explique pourquoi. Le
`normfix.toml` lui-même doit être un fichier régulier borné, et non un lien
symbolique. La politique ne remplace toujours ni le sujet du projet ni
l’évaluateur.

## Commentaires et capacités destructives

Les commentaires rejetés en tant que `WRONG_SCOPE_COMMENT` ou `COMMENT_ON_INSTR`
sont seulement signalés par défaut. `--remove-invalid-comments` ne supprime qu’un
commentaire trouvé exactement à la ligne et à la colonne d’affichage signalées
par le vérificateur officiel. Il ne retire jamais l’en-tête officiel, et
l’empreinte des jetons de code restants doit rester inchangée.

`--remove-unused` et `--remove-unexpected` demandent des capacités destructives
plus fortes :

- la suppression des fonctions inutilisées ne considère que les définitions
  `static` ;
- elle exige que les entrées sélectionnées soient égales à l’ensemble complet
  des `.c`/`.h` du projet ;
- la récupération de l’analyseur, les octets inconnus, l’ambiguïté du
  préprocesseur, le collage de jetons, les attributs, les références par chaîne,
  les définitions dupliquées ou un graphe de références incertain préservent la
  fonction ;
- la suppression des fichiers inattendus est une mise en quarantaine
  récupérable, jamais une suppression définitive fondée sur l’extension.

Dans une exécution humaine et interactive, ces capacités demandent une
confirmation `y/N` avant l’analyse. L’invite n’accorde que la capacité demandée ;
chaque candidat doit toujours passer ses preuves d’analyse, de hachage, de portée
et de transaction. Répondre oui n’affaiblit aucune preuve.

Les exécutions en JSON et les autres exécutions non interactives exigent
`--force` :

```sh
normfix --remove-unused --force
normfix --remove-unexpected --force
normfix --unsafe --force
```

`--unsafe` est un raccourci fermé pour six opérations implémentées :

- suppression d’un commentaire invalide à un emplacement exact ;
- compactage des comparaisons simples avec `NULL` uniquement lorsque la forme C
  dédiée est prouvée ;
- suppression des jetons prouvés absents ou constitués uniquement de trivia dans
  les listes littérales simples de sources du Makefile ;
- suppression de prototypes de headers locaux au projet uniquement lorsqu’une
  preuve complète et sans perte de la source ne trouve ni implémentation ni
  usage ou ambiguïté ;
- suppression de `static` inatteignable sous une preuve à source fermée ;
- mise en quarantaine des fichiers inattendus.

Les avertissements sur l’implémentation des prototypes sont déjà actifs dans les
exécutions normales. Le mode non sûr peut supprimer une déclaration absente et
inutilisée après la preuve complète ; il ne supprime jamais une définition
existante constituée uniquement de trivia ni son prototype, car un corps vide
peut être intentionnel.

Il n’autorise pas des éditions arbitraires. La suppression de commentaires peut
aussi être demandée seule avec `--remove-invalid-comments` ; les autres plans
destructifs exigent toujours une autorisation de capacité.

Utilisez le mode aperçu avant une exécution destructive :

```sh
normfix --diff --remove-unused
normfix --check --remove-unexpected
```

Les modes aperçu demandent la même autorisation interactive, car les
planificateurs à monde fermé sont eux-mêmes protégés par capacité, mais ils
n’écrivent, ne suppriment et ne déplacent aucun fichier du projet.

## Sauvegardes, transactions et récupération

Les sauvegardes de sources sont par défaut externes au projet analysé :

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
```

Sur Unix sans `XDG_DATA_HOME`, le repli est :

```text
~/.local/share/normfix/backups/<run-id>/
```

Chaque transaction sauvegardée contient les octets d’origine exacts et un
`journal.json`. Avant que la première cible ne change, l’écrivain :

- canonicalise la frontière du projet ;
- rejette les cibles dupliquées, externes, les liens symboliques et les fichiers
  non réguliers ;
- confirme que chaque fichier actuel correspond encore aux octets analysés ;
- écrit les sauvegardes externes ;
- prépare et synchronise chaque remplacement.

Les cibles sont validées dans l’ordre des chemins. Une erreur en cours de
validation déclenche un rollback au mieux, à partir des octets d’origine
capturés ; un rollback incomplet est signalé avec le chemin du journal de
récupération.

`--no-backup` ne s’applique qu’au formatage sûr ordinaire. Une suppression de
source planifiée par le retrait de commentaires invalides, la réconciliation des
sources du Makefile, la suppression de prototypes orphelins ou la suppression de
`static` inatteignable exige un stockage de récupération externe et échoue en
position fermée s’il est indisponible.

La quarantaine conserve toujours une copie externe récupérable, y compris lorsque
`--no-backup` a été fourni :

```text
<backup-base>/quarantine/<run-id>/<original-relative-path>
```

Le type du fichier, sa longueur en octets et son empreinte BLAKE3 sont revérifiés
juste avant le déplacement. Les destinations de récupération existantes ne sont
jamais écrasées. Un échec partiel de quarantaine tente de restaurer les fichiers
déjà déplacés.
