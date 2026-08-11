# Performance

Chaque chiffre ici a été mesuré, et chacun est reproductible avec une commande
que vous pouvez lancer vous-même. Là où un chiffre n'est pas impressionnant, le
texte le dit et dit pourquoi.

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
  faire signifie que le pipeline ne peut plus analyser un tampon fantôme à la
  fois, ce qui est la structure actuelle de la preuve avant/après. C'est le plus
  grand gain restant et celui au coût architectural le plus élevé.
- **Les très gros fichiers uniques.** Au-delà de quelques milliers de lignes, le
  coût est dominé par autre chose que l'index de lignes, et cela n'a pas été
  creusé. Les vraies sources 42 sont très en dessous.
- **L'allocation des jetons.** Chaque analyse copie le texte de chaque jeton dans
  une chaîne propriétaire. L'emprunter à la source est un changement contenu qui
  n'a pas encore été mesuré.
