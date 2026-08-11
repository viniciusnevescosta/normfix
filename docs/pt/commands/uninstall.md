# `normfix uninstall`

Remove este binário e — apenas quando pedido explicitamente — os dados que ele
criou.

```sh
normfix uninstall --dry-run   # mostra o plano, não remove nada
normfix uninstall             # remove o binário, mantém seus dados
normfix uninstall --purge     # remove também configuração, cache e backups
```

## Ele mostra o plano primeiro

Nada é removido antes de você ver exatamente o que seria:

```console
$ normfix uninstall --dry-run
normfix uninstall
  remove  /usr/local/bin/normfix
  keep    /home/student/.config/normfix (configuration)
  keep    /home/student/.cache/normfix (cache)
  keep    /home/student/.local/share/normfix (backups and quarantine)
Pass --purge to remove the kept directories as well.
```

O padrão mantém seus dados. Isso é deliberado: o diretório de backups guarda a
única cópia de qualquer coisa que uma execução anterior tenha substituído ou
movido, e desinstalar um formatador não é uma declaração de que você quer perder
o trabalho que ele salvou para você.

## `--purge`

```console
$ normfix uninstall --purge --dry-run
normfix uninstall
  remove  /usr/local/bin/normfix
  remove  /home/student/.config/normfix (configuration)
  remove  /home/student/.cache/normfix (cache)
  remove  /home/student/.local/share/normfix (backups and quarantine)
This also deletes backups and quarantined files, which is the only copy of anything a previous run replaced or moved.
```

Configuração e cache são reproduzíveis: a primeira é a sua identidade 42, que
você pode fornecer de novo, e o segundo é um cache. Backups e arquivos em
quarentena não são. Rode [`normfix undo --list`](/pt/commands/undo) antes se não
tiver certeza de que algo ainda é recuperável.

## Confirmação

Uma execução interativa pergunta antes de remover qualquer coisa:

```console
Remover os arquivos listados acima? [y/N]
```

`y` é a resposta aceita em todos os idiomas. Uma execução não interativa — um
script, a CI ou `--format json` — recusa em vez de supor, e exige `--force`:

```sh
normfix uninstall --force
normfix uninstall --purge --force
```

## Quando ele recusa

| Situação | O que ele diz |
|---|---|
| Instalado pelo Homebrew | Aponta para `brew uninstall viniciusnevescosta/normfix/normfix` |
| Sem permissão de escrita | Nomeia o caminho e manda conferir o dono; nunca pede `sudo` |
| Um diretório de dados não pode ser removido | Nomeia esse diretório e para, com o binário ainda instalado |

O Homebrew é recusado em vez de contornado: remover um arquivo que a fórmula
ainda descreve deixa o `brew` como a única coisa capaz de recolocar a máquina em
um estado consistente.

Os diretórios de dados são removidos antes do binário. Se um deles falhar, a
ferramenta que relatou a falha ainda está em disco para tentar de novo.

## Remover um binário em execução

No Unix, desvincular o executável em execução é seguro: o kernel mantém o arquivo
vivo até o processo terminar, então o comando conclui e imprime seu resultado
normalmente. O que é removido é o nome no sistema de arquivos.
