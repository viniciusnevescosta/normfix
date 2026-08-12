# Guide de localisation

Ceci est le contrat des contributeurs pour traduire chaque surface de normfix
qu’une personne lit. Les premières langues publiées sont l’anglais (`en`), le
portugais (`pt`), l’espagnol (`es`) et le français (`fr`). Une langue n’est
complète que lorsqu’un lecteur peut installer l’outil, comprendre sa frontière de
sûreté, utiliser le playground du navigateur et suivre la documentation centrale
sans retomber sur l’anglais.

La localisation ne doit pas changer l’interface machine. Traduisez les
explications, pas les identifiants.

## Ce qui reste en anglais

Ces valeurs sont des jetons stables d’API ou de code source et doivent rester
inchangées dans toutes les langues :

- la commande `normfix` et les noms des sous-commandes ;
- les options comme `--check`, `--changed` et `--format json` ;
- les identifiants de règle comme `TOO_MANY_LINES` et
  `MAKEFILE_SOURCE_NOT_FOUND` ;
- les clés JSON, les valeurs d’énumération, `schema_version` et les codes de
  sortie ;
- les clés de configuration, les noms de variables d’environnement et les noms de
  fichiers ;
- les identifiants C, les commandes shell, les chemins, les noms d’archives et
  les exemples de code ;
- les messages de commit Git et les commentaires de code Rust/TypeScript.

Gardez inchangés les noms officiels de produits — Norminette, Rust, WSL, Clang,
Vite, Monaco, Git, GitHub et Vercel. Traduisez la phrase autour d’eux et
conservez le lien officiel.

## Surfaces actuelles

| Surface | Source du texte traduit | Comportement publié |
|---|---|---|
| Playground du navigateur | `web/src/i18n.ts` et attributs `data-i18n*` dans `web/index.html` | Interface complète en `en`, `pt`, `es` et `fr` ; choisir une langue ne fait que changer la langue, et le choix est retenu jusqu’à ce qu’il change |
| Playground installé | Un web app manifest par langue, généré dans `web/vite.config.ts` | Chaque langue s’installe avec son propre nom, sa propre identité et sa propre URL de départ, si bien qu’un playground installé s’ouvre dans la langue choisie par son lecteur |
| Documentation | Arborescences de langue sous `docs/`, plus la navigation par langue dans `docs/.vitepress/config.ts` | Landing, installation, playground, sûreté, compatibilité et parcours de contribution localisés, avec l’anglais comme repli explicite pour les pages pas encore publiées |
| SEO | Head/config de VitePress, `web/index.html`, sitemaps et `robots.txt` | URLs canoniques et `hreflang` uniquement pour les pages qui existent vraiment |
| CLI native | Catalogue dans `crates/normfix-i18n`, sélectionné avec `--lang` ou par la locale du processus | Annonce, prose du rapport, invites de sûreté, articles `explain` et diagnostics propres à ce projet en `en`, `pt`, `es` et `fr` ; les résultats relayés du vérificateur officiel ou du compilateur C restent tels que ces outils les ont produits ; commandes, options, JSON, identifiants de règle et codes de sortie restent neutres |

## Traduire le playground

1. Ajoutez le code de langue à `SUPPORTED_LOCALES` dans `web/src/i18n.ts`.
2. Renseignez chaque `MessageKey`. Ne publiez pas une langue qui hérite en
   silence d’un bouton, d’une erreur de validation, d’une déclaration de
   confidentialité ou d’un libellé d’accessibilité en anglais.
3. Placez le texte statique du HTML derrière `data-i18n`, `data-i18n-title`,
   `data-i18n-placeholder` ou `data-i18n-aria`. Placez le texte dynamique derrière
   `translate()` ; ne laissez pas de littéraux visibles par l’utilisateur dans
   `web/src/main.ts` ni dans `web/src/editor.ts`.
4. Utilisez des placeholders nommés comme `{path}` et `{count}`. Chaque traduction
   doit conserver exactement le même ensemble de placeholders que l’anglais.
5. Quand un message contient un compte, n’écrivez pas une phrase unique avec un
   placeholder dedans. Ajoutez une entrée par catégorie de pluriel CLDR —
   `importedOne`, `importedOther` — et rendez-la avec `translatePlural`, pour que
   le nom s’accorde avec son nombre au lieu de donner « 1 fichiers ajoutés ». Un
   message pluralisé doit dépendre d’exactement un compte ; une phrase qui en
   contient deux ne peut pas s’accorder dans toutes les langues, alors écrivez
   deux phrases.
6. Formatez nombres et dates avec la langue sélectionnée. Ne localisez pas
   l’horodatage figé de l’en-tête 42 ni les autres textes de protocole.
7. Définissez la valeur `lang` du document et proposez un sélecteur de langue
   visible.
8. N’injectez jamais une traduction avec `innerHTML`. Continuez d’utiliser
   `textContent` et des nœuds du DOM, afin qu’un texte traduit ou versionné ne
   puisse pas devenir du markup.
9. Testez le repli en zone de texte sur écran étroit, en plus de Monaco. Monaco
   lui-même ne définit pas la complétude de la localisation du produit. Le
   parcours hors ligne utilise ce même repli : c’est donc aussi ce que voit un
   lecteur qui ouvre le playground installé sans réseau.
10. Traduisez le nom de l’application dans `localizedPages`, dans
    `web/vite.config.ts`. C’est le libellé sous l’icône de qui installe le
    playground : il doit être court et se lire comme un nom, pas comme un titre
    de page.

Les diagnostics Rust natifs renvoyés par WebAssembly restent en anglais.
L’interface doit le dire clairement plutôt que de présenter un diagnostic
partiellement traduit comme une localisation complète.

## Traduire la documentation

Utilisez la page anglaise comme source de vérité. Conservez les titres qui sont
des cibles de lien, sauf si la configuration de la langue fournit aussi une
redirection testée. Gardez les exemples de commandes valides octet par octet ; ne
traduisez que la prose alentour et la sortie humaine attendue.

Pour une nouvelle langue :

1. créez son répertoire et traduisez la landing page ;
2. traduisez « bien démarrer », le guide du playground, sûreté/récupération,
   compatibilité et ce guide de localisation avant d’annoncer la langue ;
3. ajoutez les libellés, la navigation, la sidebar, les libellés de recherche, le
   pied de page et le texte du lien d’édition localisés dans VitePress ;
4. faites un lien vers les pages officielles de Norminette, Rust, WSL et Clang
   partout où ces outils sont nommés comme dépendances ;
5. n’ajoutez de métadonnées canoniques et de langue alternative qu’entre des
   pages traduites équivalentes ;
6. incluez chaque URL localisée publiée dans le sitemap généré ;
7. vérifiez chaque lien interne et chaque bloc de code dans la compilation de
   production.

Ne créez pas une page mince dont le seul contenu est une redirection automatique
vers l’anglais en l’appelant une traduction. Un lien explicite « Cette page est
disponible en anglais » est un repli temporaire acceptable lorsque la route
localisée n’est pas annoncée comme complète.

## Traduire la CLI native

Le crate `crates/normfix-i18n` possède la sélection de langue et le catalogue. Le
texte traduit y vit, jamais dans le code qui décide quoi dire.

La complétude est garantie par le compilateur, pas par la relecture. Chaque
langue est un unique littéral de struct `Messages` : une nouvelle entrée qu’une
langue ne traduit pas est donc une erreur de compilation. Deux tests couvrent ce
que le système de types n’atteint pas : aucune entrée ne peut être vide, et
chaque traduction doit porter le même ensemble de `{placeholder}` que son
original anglais. Les placeholders sont nommés, pas positionnels : une traduction
peut donc les réordonner.

Pour ajouter une entrée :

1. ajoutez le champ à `Messages` avec un commentaire de documentation nommant ses
   placeholders ;
2. renseignez-le dans les quatre littéraux de langue dans le même changement ;
3. rendez-le via `messages.<champ>` et `normfix_i18n::fill`, jamais en littéral au
   point d’appel.

La sélection de langue suit `--lang`, puis `NORMFIX_LANG`, `LC_ALL`,
`LC_MESSAGES` et `LANG`, puis l’anglais. Seul le sous-tag primaire compte :
`pt_BR.UTF-8` sélectionne donc le portugais. Une valeur `--lang` non publiée
retombe sur l’anglais avec un avertissement concis ; une locale de processus non
publiée retombe en silence, car un indice n’est pas une décision. Aucun de ces cas
n’est fatal : la langue de sortie ne peut pas être une raison de refuser
d’analyser un projet.

Le JSON n’est jamais localisé. L’événement `execution_start` et le rapport final
conservent des valeurs anglaises dans toutes les langues, si bien qu’un script
n’a jamais à choisir une langue pour rester fiable.

### Ce qui est traduit, et ce qui ne le sera jamais

Traduit : l’annonce de l’exécution, la prose du rapport lui-même, toutes les
invites critiques pour la sûreté, les articles `explain` et les diagnostics que
ce projet rédige.

Jamais traduit : un résultat relayé de la Norminette officielle ou du compilateur
C. Ce texte est la sortie de ces outils. Le réécrire ferait diverger le rapport de
ce qu’affiche `norminette` lancé directement, ce qui est pire que de lire une
phrase en anglais. Une exécution non anglophone le dit en une ligne — comme un
fait sur l’origine de ces mots, non comme une excuse pour une traduction
manquante.

Les jetons d’état du tableau des fichiers (`CLEAN`, `WOULD FIX`, `REVIEW`,
`FAILED`) et les mots de sévérité restent en anglais, aux côtés des identifiants
de règle qu’ils accompagnent.

Pour traduire un nouveau diagnostic, ajoutez un `DiagnosticKey`, renseignez-le
dans les quatre `match` de langue et construisez-le avec `localized_text`.
L’anglais est toujours produit, car c’est lui qui atteint le JSON et qu’utilisent
l’égalité et le tri.

## Terminologie et ton

- Employez le vocabulaire que les étudiants voient déjà à 42.
- Préférez des phrases courtes et directes dans les avertissements et les
  boutons.
- Gardez précise la distinction entre **avertissement**, **échec**, **non sûr**,
  **récupérable**, **informatif** et **concluant**.
- Ne traduisez pas « safe » par « garanti correct ». Cela signifie que la preuve
  documentée de cette édition est passée.
- Ne traduisez pas l’estimation d’avant-soutenance comme une note officielle.
- Conservez l’affirmation selon laquelle l’identité dans le navigateur est une
  configuration locale à l’appareil, pas un secret chiffré.

Lorsqu’un terme fait débat, mettez à jour un petit glossaire dans les notes de
contribution de cette langue et employez une graphie cohérente entre le
playground et la documentation.

## Validation

Lancez les contrôles complets du site après tout changement de localisation :

```sh
npm ci
npm run typecheck
npm audit --audit-level=moderate
npm run build
```

Relisez ensuite chaque langue en largeur bureau et sur écran étroit. Vérifiez
l’accès clavier, les libellés de focus, les débordements de texte, la formulation
des pluriels et des comptes, le comportement du bouton de copie de code, les
liens cassés, les URLs canoniques, `hreflang` et le sitemap. Une personne qui
parle couramment la langue cible doit approuver le sens et le ton ; une
compilation TypeScript qui passe ne prouve que la forme du catalogue.

Pour un changement du catalogue de la CLI, lancez aussi les tests du workspace
Rust, Clippy avec avertissements refusés, rustdoc avec avertissements refusés, et
les fixtures du schéma JSON.

## Liste de contrôle de la pull request

- [ ] Chaque nouveau texte destiné à des personnes est dans le bon catalogue.
- [ ] Commandes, options, identifiants de règle, clés JSON et exemples de code
      sont inchangés.
- [ ] Les noms des placeholders et le sens lié à la sûreté correspondent à
      l’anglais.
- [ ] Navigation, libellés d’accessibilité, métadonnées et chemins d’erreur sont
      traduits.
- [ ] Les entrées canoniques, `hreflang` et du sitemap ne pointent que vers des
      pages réelles.
- [ ] Les liens vers les dépendances officielles sont conservés.
- [ ] Les portes du site et de Rust concernées par le changement passent.
- [ ] Une personne parlant couramment la langue a relu le rendu, pas seulement le
      diff.
