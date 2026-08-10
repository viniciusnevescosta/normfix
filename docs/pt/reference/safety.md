# Segurança, recuperação e operações destrutivas

O `normfix` só aplica automaticamente uma edição quando a prova correspondente
é concluída. Um diagnóstico ou sugestão não equivale a uma correção comprovada.
Use a prévia antes de qualquer operação destrutiva:

```sh
normfix --diff --remove-unused
normfix --check --remove-unexpected
```

## Autorizações destrutivas

Comentários inválidos são apenas relatados por padrão.
`--remove-invalid-comments` remove somente o comentário na posição exata
informada pela Norminette oficial e preserva o cabeçalho 42. As opções
`--remove-unused`, `--remove-unexpected` e `--unsafe` exigem confirmação `y/N`
em uma sessão interativa. Em JSON ou outra execução não interativa, exigem
`--force`.

Uma confirmação só autoriza a capacidade solicitada. Cada candidato ainda
precisa passar pelas provas de parser, hash, escopo e transação. Ambiguidade,
arquivos ilegíveis, macros complexas ou um conjunto incompleto de fontes fazem
a operação falhar de modo conservador.

## Backups e undo

Por padrão, os bytes originais e o `journal.json` ficam fora do projeto:

```text
$XDG_DATA_HOME/normfix/backups/<run-id>/
~/.local/share/normfix/backups/<run-id>/
```

Antes de gravar, o programa rejeita alvos externos, links simbólicos, arquivos
irregulares, duplicados ou alterados desde a análise. Uma falha durante a
gravação aciona rollback. Remoções exigem armazenamento de recuperação mesmo
com `--no-backup`; arquivos inesperados são movidos para quarentena, não
apagados permanentemente.

Use `normfix undo` para restaurar a transação mais recente. Guarde o caminho do
journal exibido se um rollback não puder ser concluído automaticamente.

O arquivo `normfix.toml` e as listas de funções autorizadas complementam, mas
nunca substituem, o subject e a avaliação oficial da 42.
