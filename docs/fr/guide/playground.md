# Playground dans le navigateur

Le <a href="/fr/" target="_self">playground</a>, c'est normfix qui tourne dans
votre onglet. Collez ou déposez un projet, appuyez sur Exécuter, et vous obtenez
le code formaté, les constats qu'il a pu prouver et le diff — sans que rien ne
quitte votre machine.

C'est le même code que celui de la ligne de commande : ce qu'il corrige ici, il
le corrige là-bas. Ce qu'il ne peut pas faire ici, c'est confronter votre
travail à la Norminette officielle ou à un compilateur, car aucun des deux
n'existe dans un navigateur. Chaque résultat le dit.

Sur les navigateurs de bureau, l’éditeur est Monaco, avec numéros de ligne,
recherche, curseurs multiples, paires de crochets et coloration pour tous les
types de fichiers pris en charge. Les mobiles et les appareils à pointeur
grossier utilisent une zone de texte légère, car Monaco ne prend pas
officiellement en charge les navigateurs mobiles.

## Ajouter votre projet

Faites glisser des fichiers sur la page, ou le dossier du projet lui-même. Un
dossier déposé conserve sa structure : `libft/src/ft_strlen.c` arrive sous ce
chemin plutôt qu’aplati parmi d’autres noms.

Un vrai répertoire de projet contient plus que du code. Les fichiers objets, le
binaire compilé, `.git` et la configuration de l’éditeur sont ignorés au lieu de
devenir une erreur, et le nombre d’éléments ignorés est toujours affiché :
l’import n’écarte jamais rien en silence, et ne refuse pas tout le dépôt parce
qu’un fichier n’est pas quelque chose que normfix formate. **Choisir des
fichiers** fait la même chose avec un sélecteur.

Le bouton **+** crée un fichier. Choisissez le type — `.c`, `.h`, `Makefile` ou
`.md` — plutôt que de taper l'extension et de découvrir ensuite qu'elle n'était
pas l'une des quatre. Un chemin comme `src/utils.c` crée le dossier avec lui, et
les dossiers s'imbriquent autant que nécessaire. **Tout télécharger (.zip)**
conserve cette structure.

## Les constats soulignés là où ils sont

Les erreurs et les avertissements sont soulignés dans l'éditeur comme votre
propre éditeur les souligne, pour que vous cessiez de rapprocher un numéro de
ligne d'une liste d'une ligne de votre fichier. Au survol, la règle et
l'explication apparaissent.

Un constat sans position — un en-tête 42 invalide appartient au fichier, pas à
une ligne — reste hors des soulignements plutôt que d'être dessiné n'importe où.
Vous le retrouvez dans le panneau des diagnostics.

## Apparence

**Système**, **Clair** ou **Sombre**, à côté du sélecteur de langue. Elle suit
votre système d’exploitation sauf indication contraire, et le choix est retenu
sur cet appareil jusqu’à ce que vous en changiez — comme la langue, elle change
l’aspect de la page et rien d’autre : aucune exécution, aucune requête, aucun
rechargement.

## En-tête officiel 42

Saisissez une adresse étudiante valide dans le panneau **Identité 42**. L’option
**Retenir sur cet appareil** est désactivée au départ. Lorsque vous l’activez
explicitement, l’adresse est conservée uniquement dans le stockage local de même
origine de ce navigateur et peut être effacée à tout moment avec **Oublier**.
Sinon, elle ne vaut que pour l’onglet courant.

L’adresse est transmise à WebAssembly dans l’onglet pour générer l’en-tête
officiel 42. Elle n’est jamais envoyée à un serveur de formatage. Sans identité
valide, le code reste sans en-tête généré et le résultat comporte un diagnostic
qui le signale.

## Récupérer le résultat

Une exécution couvre toujours le projet entier, car un en-tête et le fichier qui
l’inclut ne s’évaluent correctement qu’ensemble. Le choix porte sur ce qu’on
fait de la réponse : appliquer d’un coup tout ce qui a été prouvé, ou seulement
ce que vous avez sous les yeux. Dans les deux cas, une correction cesse d’être
applicable si le fichier a été modifié depuis l’exécution, puisqu’elle a été
prouvée contre le code que normfix a lu, pas contre ce qui se trouve maintenant
dans l’éditeur.

- **Corriger tous les fichiers** écrit d’un coup, dans le projet, chaque
  résultat prouvé.
- **Corriger ce fichier** fait de même pour le fichier que vous regardez.
- **Copier le fichier** copie le résultat stable sélectionné. Si l’accès au
  presse-papiers est refusé, le navigateur sélectionne le texte pour une copie
  au clavier.
- **Télécharger le fichier** enregistre le résultat sélectionné.
- **Tout télécharger (.zip)** enregistre tous les résultats stables dans une
  seule archive que chaque système de bureau ouvre sans rien installer.
- **Utiliser comme nouvelle entrée** renvoie un résultat dans l’éditeur pour une
  nouvelle exécution.

## Confidentialité et comportement réseau

Le code et l’identité restent dans l’onglet. Il n’y a ni envoi de code, ni
compte, ni dépendance analytique, ni backend de formatage. La seule requête
externe est une consultation non authentifiée et sans referrer du nombre public
d’étoiles du dépôt officiel sur GitHub ; lorsqu’elle échoue, l’interface affiche
une valeur intégrée au site.

## Utilisation hors ligne

Le playground s’installe dès la première ouverture. Ensuite, la page, le
formateur WebAssembly et l’interface n’ont plus besoin de réseau : ouvrez la
même adresse dans un avion, sur le wifi de l’école au pire moment, ou même
pendant que le site est indisponible, et le formatage s’exécute comme avant.
Rien n’a jamais été envoyé nulle part : travailler hors ligne change la façon
d’atteindre l’outil, pas ce qu’il fait.

Le navigateur peut aussi l’installer comme application, depuis la barre
d’adresse ou le menu. Il s’ouvre alors dans sa propre fenêtre, sous le nom de la
langue que vous avez choisie.

Deux points méritent d’être connus :

- L’éditeur de bureau ne fait pas partie de l’installation. Monaco est un
  téléchargement volumineux qui apporte la coloration syntaxique et la
  recherche : il n’est récupéré qu’avec une connexion, et conservé dès qu’il y
  en a une. Ouvrir le playground hors ligne avant cela donne la zone de texte
  simple, qui formate à l’identique.
- Seul le playground est conservé. La documentation que vous lisez est un autre
  site et nécessite toujours un réseau.

Une nouvelle version ne remplace jamais la page pendant que vous y travaillez.
Elle est téléchargée en arrière-plan et l’en-tête propose **Nouvelle version
prête** avec un bouton **Recharger**. Tant que vous ne l’avez pas actionné, vous
gardez la version avec laquelle vous avez commencé.

## Frontières entre la CLI et le playground

| Capacité | CLI | Playground |
|---|---:|---:|
| Formatage sûr du C et des headers | oui | oui |
| Formatage sûr des Makefiles et du Markdown | oui | oui |
| En-tête officiel 42 à partir d’une identité fournie | oui | oui |
| Diagnostics structurels et budgets de fonction | oui | oui |
| Diffs unifiés | oui | oui |
| Vérification par la Norminette officielle | oui | non |
| Preflight strict du compilateur et analyseur | oui | non |
| Découverte automatique de l’identité | oui | non |
| Portées Git | oui | non |
| Sauvegardes, transactions et undo | oui | non |

Le bac à sable du navigateur n’exécute ni le binaire de la
[Norminette officielle](https://github.com/42school/norminette), ni un
compilateur, ni Git, ni Make. Utilisez la
[ligne de commande](/fr/guide/command-line) pour
la vérification officielle et pour le déroulé complet de préparation à la
soutenance.

## Limites et portabilité

Le playground accepte au maximum 128 fichiers, 1 Mio par fichier et 4 Mio au
total. Les chemins doivent être relatifs, portables et normalisés en NFC, avec
au plus 240 octets UTF-8. Il rejette les doublons qui se confondent sur un
système insensible à la casse, les noms réservés de plateforme, l’UTF-8 invalide
et les chemins dangereux pour une archive, avant même de lancer le formateur. Un
BOM UTF-8 initial est consommé de façon cohérente. Tout résultat du formateur
qui n’atteint pas de point fixe est écarté plutôt qu’exposé comme une édition
partielle en apparence utilisable.

## Exécuter localement

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
npm ci
npm run dev
```

La compilation exige aussi une installation de Clang dont la cible WebAssembly
fonctionne. Sur macOS, la construction sonde les chemins du LLVM de Homebrew et
explique comment installer LLVM lorsque le compilateur du système ne peut pas
produire de code pour `wasm32`.
