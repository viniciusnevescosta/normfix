# Politique de compatibilité

## Norminette officielle

`normfix` est testé avec la
[Norminette officielle](https://github.com/42school/norminette) `3.3.59`. Une
autre version continue avec l’avertissement `NORMINETTE_VERSION_UNTESTED` ;
utilisez `--strict-norminette-version` pour la refuser en CI. Le release inclut
le binaire `normfix`, pas Python ni la Norminette.

## Plateformes

Les binaires publiés couvrent Linux x86-64/ARM64 et macOS Intel/Apple Silicon.
Windows n’a pas de binaire natif : utilisez
[WSL](https://learn.microsoft.com/windows/wsl/install) pour la CLI complète.
Le playground fonctionne directement dans les navigateurs modernes avec
WebAssembly et les modules ES.

[Rust](https://www.rust-lang.org/tools/install) n’est nécessaire que pour
compiler les sources. Le MSRV est `1.85` ; le dépôt fixe `1.97.1`.

## Limites de la promesse

La Norminette est l’autorité de style. Le compilateur fournit uniquement des
diagnostics de syntaxe et warnings ; `normfix preflight` n’exécute pas les
recettes Make, ne lie pas, ne lance pas les tests et ne prouve pas l’absence de
leaks. Le playground n’exécute pas non plus Norminette, compilateur, Git ou les
transactions de fichiers.

Pour l’automatisation, utilisez `--format json` et vérifiez `schema_version`.
Commandes, flags, IDs de règles, codes de sortie et clés JSON restent en anglais
et font partie de l’interface stable.
