# Playground dans le navigateur

Le <a href="/fr/" target="_self">playground</a> exécute le cœur de `normfix` en WebAssembly. Monaco offre
numéros de ligne, recherche, curseurs multiples, paires de crochets et
coloration pour C, headers, Markdown et Makefiles. Sur mobile, un éditeur léger
est utilisé car Monaco ne prend pas officiellement en charge les navigateurs
mobiles.

## En-tête 42

Saisissez une adresse étudiante valide dans **Identité 42**. L’option de
mémorisation est désactivée par défaut. Si vous l’activez, l’adresse reste
uniquement dans le stockage local de ce navigateur et peut être supprimée avec
**Oublier**. Elle est transmise au WebAssembly de l’onglet pour générer
l’en-tête officiel, jamais à un serveur.

## Confidentialité et limites

Le code et l’identité restent dans l’onglet. La seule requête externe récupère
le nombre public d’étoiles GitHub ; une valeur intégrée est affichée en cas
d’échec. Aucun téléversement, compte, analytics ou backend de formatage.

Le navigateur n’exécute pas la [Norminette officielle](https://github.com/42school/norminette),
le compilateur, Git ou Make. Utilisez la CLI pour la vérification officielle,
les sauvegardes, les transactions et undo.

## Utilisation hors ligne

Le playground s’installe dès la première ouverture. Ensuite, la page, le
formateur WebAssembly et l’interface n’ont plus besoin de réseau : ouvrez la
même adresse dans un avion, sur le wifi de l’école au pire moment, ou même
pendant que le site est indisponible, et le formatage s’exécute comme avant.
Rien n’a jamais été envoyé nulle part : travailler hors ligne change la façon
d’atteindre l’outil, pas ce qu’il fait.

Le navigateur peut aussi l’installer comme application, depuis la barre
d’adresse ou le menu. Il s’ouvre alors dans sa propre fenêtre, sous le nom de
la langue que vous avez choisie.

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
prête** avec un bouton **Recharger**. Tant que vous ne l’avez pas actionné,
vous gardez la version avec laquelle vous avez commencé.
