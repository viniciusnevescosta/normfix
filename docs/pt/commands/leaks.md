# `normfix leaks`

Executa um programa que você já construiu sob um verificador de vazamentos e
relata o que ele observou.

```sh
normfix leaks ./libft_test
normfix leaks ./push_swap -- 3 1 2
```

Todo o resto que o normfix faz lê o seu código. Este comando o executa, então
ele pergunta antes:

```console
$ normfix leaks ./push_swap
O normfix vai executar ./push_swap sob o verificador de vazamentos. Isso roda o seu programa. Continuar? [y/N] y
Perdidos 1024 bytes de vez, e mais 96 alcançáveis só por eles.

error[LEAK_DEFINITELY_LOST]: 1024 bytes alocados aqui nunca foram liberados
 --> stack.c:23:2
   |
23 |     stack = malloc(sizeof(int) * size);
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: Aqui é onde a memória foi alocada, não onde deveria ter sido liberada. Siga daqui até o caminho que perde o ponteiro.

error[MEMORY_ERROR]: Invalid read of size 4, em sort_stack
 --> sort.c:41:2
   |
41 |     return (stack[size]);
   |     ^^^^^^^^^^^^^^^^^^^^
   |
   = help: O programa mexeu em memória que não é dele. Isso é um bug, independentemente do que a Norm diga sobre o arquivo.

Isto é o que uma execução observou com os argumentos que recebeu. Não é prova de que o programa nunca vaza.
```

Aparecem dois tipos de achado aqui, e eles respondem perguntas diferentes. Um
achado `LEAK_` aponta onde a memória foi alocada e depois perdida — a linha que
alocou, que é o que o verificador consegue ver, não o lugar onde ela deveria ter
sido liberada. Um `MEMORY_ERROR` aponta a linha que leu, escreveu ou liberou
algo que o programa não tinha direito de tocar; esse é o bug em si.

Os argumentos depois de `--` vão para o seu programa, não para o verificador,
então você consegue exercitar o caminho que importa:

```sh
normfix leaks ./push_swap -- 5 2 9 1
```

Um binário compilado sem `-g` não carrega números de linha: nesse caso o
relatório nomeia só a função e explica o porquê.

## O que ele não faz

O `normfix` nunca constrói o seu programa. Construir significa executar as
receitas do seu Makefile, que é uma segunda categoria — bem maior — de executar
código que você escreveu; e *"você construiu, eu executei"* é uma promessa bem
menor que *"eu construí e executei"*. Construa do jeito que você já faz e aponte
este comando para o resultado.

## Um resultado limpo não é prova

O verificador vê o único caminho que o seu programa percorreu com os argumentos
que você deu. Uma execução que não perde nada diz que aquele caminho está limpo;
não diz nada sobre os caminhos que você não percorreu. Essa linha é impressa com
todo resultado pelo mesmo motivo que o resto da ferramenta relata o que não
consegue provar em vez de afirmar.

Memória ainda alcançável no encerramento não conta como perdida. A 42 avalia
memória que ninguém consegue mais alcançar, e uma arena que o seu programa
segura até sair não é isso.

Se o verificador produzir uma saída que o normfix não consegue ler como sumário
de vazamentos, isso é um erro, não um resultado limpo. Um verificador que foi
morto e um que não achou nada produzem o mesmo silêncio, e a diferença importa
demais para ser adivinhada.

## Códigos de saída

| Código | Significado |
|---|---:|
| `0` | Nada foi perdido no caminho desta execução |
| `1` | Algo foi perdido |
| `2` | O verificador está indisponível, foi recusado, ou não pôde ser lido |

Fora de um terminal interativo — em CI, ou com `--format json` — a confirmação
não pode ser respondida, então o `--force` é obrigatório:

```sh
normfix leaks --force ./libft_test
```

## Instalando um verificador

| Sistema | Como |
|---|---|
| Linux, FreeBSD | Valgrind, pelo gerenciador de pacotes |
| macOS | [`LouisBrunner/valgrind-macos`](https://github.com/LouisBrunner/valgrind-macos), já que o Valgrind oficial não compila no macOS. O suporte dele a Apple Silicon é limitado |
| Windows | Rode o normfix dentro do [WSL](https://learn.microsoft.com/windows/wsl/install), onde o verificador de Linux funciona normalmente |

O normfix localiza o `valgrind` no `PATH` e o verifica pelo próprio `--version`,
então qualquer build funcional serve. Quando nenhum é encontrado, ele diz isso e
nomeia o caminho para o sistema em que você está.
