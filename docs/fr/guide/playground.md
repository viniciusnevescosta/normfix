# Playground dans le navigateur

Le <a href="/fr/" target="_self">playground</a> exécute le cœur de `normfix` en WebAssembly. Monaco offre
numéros de ligne, recherche, curseurs multiples, paires de crochets et
coloration pour C, headers, Markdown et Makefiles. Sur mobile, un éditeur léger
est utilisé car Monaco ne prend pas officiellement en charge les navigateurs
mobiles.

## En-tête 42

Saisissez une adresse étudiante valide dans **Identité 42**. Si vous choisissez
de la mémoriser, elle reste uniquement dans le stockage local de ce navigateur
et peut être supprimée avec **Oublier**. Elle est transmise au WebAssembly de
l’onglet pour générer l’en-tête officiel, jamais à un serveur.

## Confidentialité et limites

Le code et l’identité restent dans l’onglet. La seule requête externe récupère
le nombre public d’étoiles GitHub ; une valeur intégrée est affichée en cas
d’échec. Aucun téléversement, compte, analytics ou backend de formatage.

Le navigateur n’exécute pas la [Norminette officielle](https://github.com/42school/norminette),
le compilateur, Git ou Make. Utilisez la CLI pour la vérification officielle,
les sauvegardes, les transactions et undo.
