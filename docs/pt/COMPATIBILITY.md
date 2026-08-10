# Política de compatibilidade

## Norminette oficial

O `normfix` é testado com a
[Norminette oficial](https://github.com/42school/norminette) `3.3.59`. Uma
versão diferente continua com o aviso `NORMINETTE_VERSION_UNTESTED`; use
`--strict-norminette-version` para recusá-la em CI. O release inclui o binário
do `normfix`, não Python nem a Norminette.

## Plataformas

Os binários publicados cobrem Linux x86-64/ARM64 e macOS Intel/Apple Silicon.
O Windows não tem binário nativo: use
[WSL](https://learn.microsoft.com/windows/wsl/install) para a CLI completa. O
playground funciona diretamente em navegadores modernos com WebAssembly e
módulos ES.

O [Rust](https://www.rust-lang.org/tools/install) só é necessário para compilar
as fontes. O MSRV é `1.85`; o repositório fixa `1.97.1`.

## Limites da promessa

A Norminette é a autoridade de estilo. O compilador executa apenas diagnósticos
de sintaxe e warnings; `normfix preflight` não executa receitas Make, não linka,
não roda testes e não prova ausência de leaks. O playground também não executa
Norminette, compilador, Git ou transações de arquivos.

Para automação, use `--format json` e valide `schema_version`. Comandos, flags,
IDs de regras, códigos de saída e chaves JSON permanecem em inglês e fazem
parte da interface estável.
