# `normfix upgrade`

Remplace le binaire en cours d'exécution par la version publiée la plus récente
de son canal de mise à jour.

```sh
normfix upgrade          # télécharge, vérifie et installe
normfix upgrade --check  # signale seulement
```

```console
$ normfix upgrade --check
normfix 1.4.0 is already the newest release.
```

## Ce qu'il fait, dans l'ordre

1. Choisit le canal de mise à jour d'après la version en cours. Une version
   stable interroge le point d'accès `/releases/latest` de GitHub pour la version
   stable la plus récente. Une préversion suit le flux complet des versions, et
   peut donc avancer vers une nouvelle version candidate ou vers la stable
   finale.
2. S'arrête si vous l'exécutez déjà.
3. Refuse si le binaire est géré par Homebrew, et indique la commande qui fait ce
   qu'il faut dans ce cas.
4. Télécharge l'archive de votre plateforme et le `SHA256SUMS` publié.
5. **Vérifie l'empreinte.** Une différence interrompt et affiche les deux
   valeurs ; rien n'est écrit.
6. Extrait dans un répertoire de préparation *à l'intérieur* de la destination,
   pour que l'étape finale soit un renommage sur le même système de fichiers : le
   binaire est soit remplacé, soit laissé exactement tel quel.

Remplacer un exécutable en cours est sûr sous Unix, car le processus en cours
conserve l'ancien fichier jusqu'à sa fin.

La frontière entre canaux est délibérée : ni `upgrade` ni l'avis quotidien de
version ne déplacent une installation stable vers une bêta ou une version
candidate. Choisir une préversion reste un choix explicite au moment de
l'installation.

## Quand il refuse

| Situation | Ce qu'il dit |
|---|---|
| Installé par Homebrew | Vous renvoie à `brew upgrade viniciusnevescosta/normfix/normfix` |
| Pas de droit d'écriture | Nomme le chemin et dit de vérifier le propriétaire ; il ne demande jamais `sudo` |
| Somme de contrôle différente | Affiche les deux empreintes et n'installe rien |
| Ni `curl` ni `wget` | Indique quel outil manque |
| Plateforme non prise en charge | Suggère de compiler depuis les sources ou d'utiliser le playground |

## L'avis de version

Une exécution normale affiche une ligne quand une version plus récente existe :

```text
normfix 1.0.0 is available; this is 1.0.0-rc.1. Run `normfix upgrade`.
```

C'est le seul accès réseau en dehors d'`upgrade` lui-même, il est donc
délibérément étroit :

- au plus **une fois par jour**, l'horodatage étant mis en cache dans
  `$XDG_CACHE_HOME/normfix/last-update-check` ;
- uniquement pour la **sortie humaine interactive**, jamais pour
  `--format json` et jamais quand stderr n'est pas un terminal, si bien que les
  scripts et la CI ne sont pas affectés ;
- **silencieux en cas d'échec**, car un formateur qui n'atteint pas le réseau
  n'a rien d'anormal ;
- la tentative est enregistrée *avant* la requête, pour qu'un réseau
  inaccessible ne fasse pas payer la même recherche à chaque exécution.

Désactivez-le complètement :

```sh
export NORMFIX_NO_UPDATE_CHECK=1
```

::: tip Rien de votre code ne quitte la machine
La vérification demande à GitHub des métadonnées publiques de version. Elle
n'envoie aucun chemin, aucun code source et aucun identifiant d'aucune sorte.
:::
