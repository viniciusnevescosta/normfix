# `normfix undo`

Restaura uma execução anterior a partir do seu backup externo e se recusa a
sobrescrever qualquer coisa que tenha mudado desde então.

```sh
normfix undo --list
normfix undo
normfix undo --run run-1785950998077000000-53423
```

## Encontrar um ponto de recuperação

```console
$ normfix undo --list
normfix undo: 1 recovery point(s)
  run-1785950998077000000-53423  1 file(s)
```

Cada execução guarda os bytes originais exatos e um `journal.json` provando
quais arquivos ela gravou e o que gravou neles.

## Restaurar

Sem `--run`, o `undo` seleciona o ponto de recuperação íntegro mais recente e
pede confirmação. A restauração não interativa exige `--force`:

```sh
normfix undo --force
```

## Quando ele recusa

O `undo` falha fechado. Ele não restaura quando:

- um arquivo de destino já não corresponde aos bytes que aquela execução gravou,
  porque alguém o editou depois, e restaurar descartaria esse trabalho em
  silêncio;
- um arquivo de backup está faltando ou seu hash não bate com o journal;
- qualquer caminho no backup ou no projeto resolve através de um link
  simbólico;
- o journal está ilegível ou seu schema é desconhecido.

Uma recusa nomeia o arquivo e o motivo. Isso é deliberado: uma ferramenta de
recuperação que adivinha é pior do que uma que para.

## O que não está coberto

Execuções com `--no-backup` não deixam nada para restaurar, e esse é o custo de
pular os backups. Operações destrutivas sempre mantêm armazenamento de
recuperação de qualquer forma, então um arquivo em quarentena ou um comentário
removido pode ser recuperado mesmo quando `--no-backup` foi passado.
