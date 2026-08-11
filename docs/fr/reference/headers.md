# En-têtes officiels 42

Comment sont traités le bloc d'en-tête, l'identité qui le sous-tend et les gardes d'inclusion d'en-tête.

Les en-têtes officiels manquants sont insérés dans les sources C, les en-têtes C
et les Makefiles quand une identité validée est disponible. La résolution
d'identité suit cet ordre :

1. `--email`, avec vérification de cohérence facultative via `--login` ;
2. `NORMFIX_EMAIL`, avec un login facultatif de l'environnement ou de la CLI ;
3. le fichier de configuration INI persistant par utilisateur ;
4. le `user.email` effectif de Git, s'il s'agit d'une adresse 42 prise en
   charge ;
5. la variable d'environnement `MAIL` ;
6. les réglages connus d'en-tête 42 de Vim, Neovim, VS Code, Cursor et VSCodium.

L'adresse est la source de vérité. Le login est la partie locale avant `@` ;
l'outil n'invente jamais une adresse et ne choisit jamais en silence entre des
adresses enregistrées ambiguës.

Quand aucune adresse valide n'est trouvée et que l'entrée et la sortie d'erreur
sont toutes deux des terminaux interactifs, le mode humain demande :

```text
No verified 42 student email was found.
Enter your 42 email (Enter, cancel, or q to skip the header):
```

Après une réponse valide, `normfix` enregistre l'adresse/le login canoniques pour
les exécutions suivantes. Entrée, `cancel`, `q` ou fin d'entrée sautent
l'insertion de l'en-tête tandis que toutes les autres corrections sûres
continuent. Les exécutions JSON et non interactives ne demandent jamais rien.
Ctrl-C annule la commande elle-même, suivant le comportement normal du terminal.

### Configuration persistante de l'identité

Fournir un `--email` valide (avec un `--login` correspondant facultatif) met aussi
à jour cette configuration automatiquement. Sous Unix, le répertoire de
l'application est en mode `0700` et le fichier remplacé atomiquement en mode
`0600`. L'adresse est une donnée de configuration ordinaire, pas un secret
chiffré.

`NORMFIX_CONFIG` choisit un chemin absolu explicite. Sinon, la valeur par défaut
de la plateforme est :

```text
$XDG_CONFIG_HOME/normfix/config.ini                    # explicit XDG base
~/Library/Application Support/normfix/config.ini       # macOS
%APPDATA%\normfix\config.ini                          # Windows
~/.config/normfix/config.ini                           # other Unix
```

Le format pris en charge est :

```ini
[header]
login = your_login
email = your_login@student.42.fr
```

La configuration par environnement est également prise en charge :

```sh
export NORMFIX_LOGIN='your_login'
export NORMFIX_EMAIL='your_login@student.42.fr'
```

Un seul horodatage est capturé pour l'exécution complète. `SOURCE_DATE_EPOCH`
peut fournir un horodatage UTC reproductible ; une valeur invalide arrête
l'exécution au lieu d'utiliser silencieusement l'horloge du système.

Les en-têtes valides existants conservent les champs `By` et `Created`. Le nom de
fichier et la ligne `Updated` ne changent que lorsque le fichier reçoit une autre
modification acceptée ou que le nom de fichier de son en-tête est périmé, ce qui
rend idempotente une seconde exécution propre.

### Gardes d'en-tête

Pour les en-têtes ordinaires, `normfix` peut insérer une garde manquante dérivée
du nom de fichier, réparer une paire `#ifndef`/`#define` incohérente ou renommer
une garde simple erronée. Chaque opération exige une preuve fermée de l'arbre de
travail Git. La preuve balaie aussi les fichiers ignorés, vérifie que la macro
attendue n'est pas utilisée, rejette les gardes dupliquées dérivées du nom de
fichier et les définitions dynamiques de compilation, et lie l'approbation aux
empreintes du projet complet et de l'en-tête.

L'insertion est refusée en cas de prétraitement conditionnel, de `#pragma once`,
de `#undef` ou de collision avec une autre macro. Un renommage est refusé quand
les anciens noms ont des usages au-delà de la paire canonique du fichier entier.
Les en-têtes complexes, référencés, à inclusion répétée, hors Git ou ambigus
restent inchangés et reçoivent un avertissement actionnable.
