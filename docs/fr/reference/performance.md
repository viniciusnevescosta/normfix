# Performance

Chaque chiffre de benchmark ici a été mesuré, et les commandes reproductibles
sont indiquées. Le relevé d’acceptation décrit aussi un corpus de terrain
volontairement temporaire, sans prétendre qu’il s’agit d’un benchmark stable.

::: tip Il n'existe pas de `normfix bench`
Les benchmarks sont un outil de développement, pas une partie de la surface de
commandes. Ils s'exécutent via `cargo bench` depuis un clone du dépôt.
:::

## Ce que coûte réellement une exécution

Sur un vrai projet, `libft` avec 44 sources et en-têtes :

| Exécution | Temps |
|---|---:|
| Cache froid, tout activé | 1,82 s |
| Cache chaud, tout activé | 0,19 s |
| Cache chaud, sans le preflight du compilateur | 0,17 s |

Le cache vaut environ **dix fois**, ce qui compte car le cas courant est de
lancer l'outil de façon répétée sur un projet en cours, pas une fois sur un
projet jamais vu.

### Pourquoi une exécution à froid coûte ce qu'elle coûte

Une invocation de la Norminette officielle coûte **107 ms** sur cette machine, et
c'est un interpréteur Python qui démarre, pas quelque chose que ce projet
contrôle. Pour 44 fichiers cela fait environ 4,7 s de travail en série, que le
parallélisme ramène à 1,82 s.

Le résumé honnête d'une exécution à froid est donc : elle est dominée par un
sous-processus par fichier. Optimiser le Rust de ce dépôt déplace ce chiffre de
quelques pour cent. Le cache existe précisément parce que la solution au coût
dominant est de ne pas refaire le travail deux fois.

## Résultat d’acceptation : une Libft volontairement désordonnée

Le candidat de la version 1.9.1 a aussi été exécuté sur une Libft adversariale
temporaire : 11 fichiers analysés, un `normfix.toml` et un fichier texte
inattendu. Elle mélangeait une garde d’en-tête erronée, des en-têtes officiels
absents, une source Makefile inexistante, des espaces à la place des
tabulations, des instructions compactées, des lignes longues, des commentaires
invalides, une boucle `for`, un ternaire, des déclarations mal alignées et des
fonctions dépassant les budgets de la Norme.

| Opération | Résultat | Temps |
|---|---|---:|
| Passe en lecture seule, cache désactivé | 351 corrections sûres proposées dans 10 fichiers | 1,06 s |
| Passe d’écriture autorisée, cache désactivé | 356 corrections écrites dans 10 fichiers ; 1 fichier inattendu mis en quarantaine | 1,30 s |
| Vérification avec un cache neuf après formatage | 0 changement ; 7 constats manuels | 0,472 s |
| Même vérification, cache chaud | médiane de cinq exécutions | 0,121 s |

Le cache chaud était **3,9 fois plus rapide** sur ce petit corpus. Plus
important que le temps, toutes les limites du résultat ont tenu :

- `make` a construit `libft.a` avec `cc -Wall -Wextra -Werror` et `ar` ;
- le même pilote d’assertions a réussi avant et après le formatage ;
- les huit objets C optimisés étaient identiques octet pour octet avant et
  après ;
- toutes les lignes C, d’en-tête et du Makefile tenaient dans 80 colonnes
  visuelles avec des tabulations de quatre colonnes ;
- la Norminette officielle n’a ensuite signalé que les six problèmes
  structurels délibérés : deux emplacements avec trop d’arguments, deux avec
  trop de fonctions, une fonction longue et une fonction avec trop de
  variables ;
- normfix a ajouté un avertissement de liste d’autorisation pour l’appel
  volontaire à `puts`, soit sept constats manuels au total ;
- une seconde passe n’a proposé aucun changement, et `normfix undo` a restauré
  exactement les dix fichiers écrits tandis que la note inattendue restait
  récupérable dans la quarantaine.

### Projets historiques, volontairement désordonnés

La régression finale de la 1.9.1 n’a pas utilisé les pointes déjà propres des
dépôts d’exemple. Elle a vérifié d’anciens commits naturellement hors Norme,
ainsi qu’une copie actuelle de `ft_printf` endommagée de façon déterministe,
toujours par un chemin absolu depuis un autre répertoire :

| Corpus | Avant | Résultat en lecture seule | Seconde passe |
|---|---:|---|---|
| [Libft `e19a16b`](https://github.com/viniciusnevescosta/Libft/commit/e19a16bcf52e9d364e1887c701248a86526184b0) | 240 constats officiels | 224 corrections sûres ; 5 constats structurels, sémantiques ou du compilateur restent | 0 changement |
| [Piscine `ca9502f`](https://github.com/viniciusnevescosta/Piscine/commit/ca9502ff2eae293e9aa46884ca40b263ac042022) | 581 constats officiels | 346 corrections sûres ; 24 constats manuels ou de nom invalide restent | 0 changement |
| [GNL `47cd2c3`](https://github.com/viniciusnevescosta/Get-Next-Line/commit/47cd2c37e6b1a0306b4d12dcb830accc173a9e27) | prototype d’en-tête mal formé | aucune modification inventée ; le point-virgule absent reste visible à l’analyseur et au compilateur | 0 changement |
| [`ft_printf` `ddd0020`](https://github.com/viniciusnevescosta/ft_printf/commit/ddd00207f42a0436f98ab4f8f38b6fdab7d81353), trois fichiers endommagés | 37 constats officiels | 35 corrections sûres dans 3 fichiers ; `make` réussit avant et après | 0 changement, 0 restant |

La mutation de `ft_printf` a retiré deux en-têtes officiels et introduit des
instructions tassées, des espaces à la place des tabulations et des erreurs de
mise en page des accolades, opérateurs, préprocesseur et `return`, sans modifier
le programme. Chaque exécution utilisait Norminette 3.3.59 sans cache. Ce corpus
a exposé la régression de racine externe ; compilateur, politique, Makefile et
transaction suivent désormais le chemin explicite du projet plutôt que le
répertoire d’invocation.

Mesuré le 2026-08-26 sur un MacBook Pro Apple M1 à 8 cœurs et 8 Go de RAM,
macOS 26.5.2, Norminette 3.3.59 et le MSRV Rust 1.85. Les temps muraux varient
selon le stockage, le démarrage de Python, la charge CPU et la forme du projet ;
les vérifications de correction ci-dessus sont les critères d’acceptation, pas
un seuil de temps.

## Ce que coûte le code propre à ce projet

Ces chiffres excluent tout outil externe, ils mesurent donc ce qu'un changement
dans ce dépôt peut réellement dégrader :

| Cas | Temps |
|---|---:|
| Fichier de 50 lignes déjà correct | 0,95 ms |
| Fichier désordonné de 40 lignes, toutes les actions de disposition | 1,89 ms |
| Fichier désordonné de 800 lignes | 38,2 ms |
| Construire un analyseur | 0,34 µs |

Mesuré sur un Apple M1, 8 cœurs, macOS 26.5, avec la chaîne d'outils figée dans
`rust-toolchain.toml`.

```sh
cargo bench -p normfix-c-actions
```

La CI exécute les mêmes benchmarks à chaque push comme tâche informative. Un
runner partagé est trop bruyant pour servir de garde-fou, mais un benchmark qui
ne tourne jamais est un benchmark qui cesse silencieusement de compiler.

## Ce que les benchmarks ont trouvé

Les benchmarks ont été ajoutés après des semaines de chronométrage à la main, et
la première exécution a contredit deux hypothèses en quelques minutes.

Un fichier de 50 lignes déjà correct mettait **4,5 ms** à décider qu'il n'y avait
rien à faire. La cause suspectée était la construction de l'analyseur ; la
mesurer a donné **340 nanosecondes**, ce n'était donc pas ça. La vraie cause :
la source était réanalysée une fois par phase de mise en forme, alors qu'elle ne
peut pas changer pendant la boucle de phases : accepter un lot est la seule chose
qui la réécrit, et cela sort immédiatement de la boucle.

En analysant une fois par passe :

| Cas | Avant | Après |
|---|---:|---:|
| Fichier de 50 lignes déjà correct | 4,49 ms | 0,95 ms |
| Fichier désordonné de 800 lignes | 108 ms | 38,2 ms |

De bout en bout sur un vrai projet, c'est une amélioration de 29 pour cent à
chaud et de 5 pour cent à froid, pour la raison ci-dessus : une exécution à froid
attend Python.

La leçon vaut plus que les chiffres. Deux explications plausibles étaient
fausses, et seule la mesure l'a dit.

## Ce qui n'est pas optimisé

- **Le sous-processus par fichier.** Norminette accepte plusieurs fichiers en une
  invocation, ce qui remplacerait 44 lancements de processus par un seul. Le
  faire signifie que le pipeline ne peut plus vérifier les octets proposés d'un
  fichier à la
  fois, ce qui est la structure actuelle de la preuve avant/après. C'est le plus
  grand gain restant et celui au coût architectural le plus élevé.
- **Les très gros fichiers uniques.** Au-delà de quelques milliers de lignes, le
  coût est dominé par autre chose que l'index de lignes, et cela n'a pas été
  creusé. Les vraies sources 42 sont très en dessous.
- **L'allocation des jetons.** Chaque analyse copie le texte de chaque jeton dans
  une chaîne propriétaire. L'emprunter à la source est un changement contenu qui
  n'a pas encore été mesuré.
