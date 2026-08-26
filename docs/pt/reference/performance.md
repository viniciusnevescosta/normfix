# Desempenho

Todo número de benchmark aqui foi medido, e os comandos repetíveis são
mostrados. O registro de aceitação também descreve um corpus de campo
deliberadamente temporário, em vez de fingir que ele é um benchmark estável.

::: tip Não existe `normfix bench`
Benchmarks são ferramenta de desenvolvimento, não parte da superfície de
comandos. Eles rodam via `cargo bench` a partir de um clone do repositório.
:::

## Quanto uma execução custa de verdade

Em um projeto real, o `libft` com 44 fontes e cabeçalhos:

| Execução | Tempo |
|---|---:|
| Cache frio, tudo ativado | 1,82 s |
| Cache quente, tudo ativado | 0,19 s |
| Cache quente, sem o preflight do compilador | 0,17 s |

O cache vale cerca de **dez vezes**, o que importa porque o caso comum é rodar a
ferramenta repetidamente em um projeto no qual você está trabalhando, não uma
vez em um projeto que você nunca viu.

### Por que uma execução fria custa o que custa

Uma invocação da Norminette oficial custa **107 ms** nesta máquina, e isso é um
interpretador Python iniciando, não algo que este projeto controle. Para 44
arquivos isso é aproximadamente 4,7 s de trabalho serial, que o paralelismo
reduz para 1,82 s.

Então o resumo honesto de uma execução fria é: ela é dominada por um subprocesso
por arquivo. Otimizar o Rust deste repositório move esse número em
porcentagens de um dígito. O cache existe exatamente porque a solução para o
custo dominante é não fazer o trabalho duas vezes.

## Resultado de aceitação: uma Libft propositalmente bagunçada

O candidato da versão 1.9.1 também foi executado contra uma Libft adversarial
temporária: 11 arquivos analisados, um `normfix.toml` e um arquivo de texto
inesperado. Ela misturava guarda de header errada, cabeçalhos oficiais ausentes,
fonte inexistente no Makefile, espaços onde tabs eram exigidos, instruções
compactadas, linhas longas, comentários inválidos, um laço `for`, um ternário,
declarações desalinhadas e funções acima dos orçamentos da Norma.

| Operação | Resultado | Tempo |
|---|---|---:|
| Passagem somente leitura, cache desativado | 351 correções seguras propostas em 10 arquivos | 1,06 s |
| Passagem de escrita autorizada, cache desativado | 356 correções gravadas em 10 arquivos; 1 arquivo inesperado colocado em quarentena | 1,30 s |
| Checagem com cache novo depois da formatação | 0 mudanças; 7 achados manuais | 0,472 s |
| Mesma checagem, cache quente | mediana de cinco execuções | 0,121 s |

O cache quente foi **3,9 vezes mais rápido** nesse corpus pequeno. Mais
importante que o tempo, todos os limites do resultado foram preservados:

- o `make` construiu `libft.a` com `cc -Wall -Wextra -Werror` e `ar`;
- o mesmo driver de asserções passou antes e depois da formatação;
- os oito objetos C otimizados ficaram byte a byte idênticos antes e depois;
- todas as linhas C, de header e do Makefile couberam em 80 colunas visuais com
  tabs de quatro colunas;
- a Norminette oficial reportou então apenas os seis problemas estruturais
  intencionais: dois locais com argumentos demais, dois com funções demais, uma
  função longa e uma função com variáveis demais;
- o normfix acrescentou um aviso da allowlist do projeto para a chamada
  deliberada a `puts`, totalizando sete achados manuais;
- uma segunda passagem propôs zero mudanças, e `normfix undo` restaurou
  exatamente os dez arquivos gravados enquanto a nota inesperada continuou
  recuperável na quarentena.

Medido em 2026-08-26 em um MacBook Pro Apple M1 com 8 núcleos e 8 GB de RAM,
macOS 26.5.2, Norminette 3.3.59 e o MSRV Rust 1.85. Tempos de relógio variam com
armazenamento, início do Python, carga da CPU e formato do projeto; as checagens
de correção acima são os critérios de aceitação, não um limite de tempo.

## Quanto custa o código deste próprio projeto

Estes números excluem toda ferramenta externa, então medem o que uma mudança
neste repositório pode de fato regredir:

| Caso | Tempo |
|---|---:|
| Arquivo de 50 linhas já correto | 0,95 ms |
| Arquivo bagunçado de 40 linhas, todas as ações de layout | 1,89 ms |
| Arquivo bagunçado de 800 linhas | 38,2 ms |
| Construir um analisador | 0,34 µs |

Medido em um Apple M1, 8 núcleos, macOS 26.5, com a toolchain fixada em
`rust-toolchain.toml`.

```sh
cargo bench -p normfix-c-actions
```

A CI roda os mesmos benchmarks a cada push como um job informativo. Um runner
compartilhado é ruidoso demais para servir de portão, mas um benchmark que nunca
roda é um benchmark que silenciosamente para de compilar.

## O que os benchmarks descobriram

Os benchmarks foram adicionados depois de semanas cronometrando à mão, e a
primeira execução contradisse duas suposições em poucos minutos.

Um arquivo de 50 linhas já correto levava **4,5 ms** para decidir que nada
precisava ser feito. A causa suspeita era a construção do analisador; medi-la
mostrou **340 nanossegundos**, então não era isso. A causa real era que a fonte
era reanalisada uma vez por fase de formatação, sendo que ela não pode mudar
enquanto o laço de fases roda: aceitar um lote é a única coisa que a reescreve,
e isso sai do laço imediatamente.

Analisando uma vez por passagem, em vez disso:

| Caso | Antes | Depois |
|---|---:|---:|
| Arquivo de 50 linhas já correto | 4,49 ms | 0,95 ms |
| Arquivo bagunçado de 800 linhas | 108 ms | 38,2 ms |

De ponta a ponta em um projeto real isso é uma melhora de 29 por cento a quente
e 5 por cento a frio, pelo motivo acima: uma execução fria está esperando o
Python.

A lição vale mais do que os números. Duas explicações plausíveis estavam
erradas, e só a medição disse isso.

## O que não está otimizado

- **O subprocesso por arquivo.** A Norminette aceita vários arquivos em uma
  invocação, o que substituiria 44 inicializações de processo por uma. Fazer
  isso significa que o pipeline não pode mais verificar os bytes propostos de um
  arquivo por vez,
  que é como a prova antes/depois está estruturada hoje. É o maior ganho
  restante e o de maior custo arquitetural.
- **Arquivos únicos muito grandes.** Acima de alguns milhares de linhas o custo
  é dominado por outra coisa que não o índice de linhas, e isso não foi
  perseguido. Fontes reais da 42 estão muito abaixo disso.
- **Alocação de tokens.** Cada análise copia o texto de cada token para uma
  string própria. Emprestar da fonte em vez disso é uma mudança contida que
  ainda não foi medida.
