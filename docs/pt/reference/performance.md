# Desempenho

Todo número aqui foi medido, e cada um é reproduzível com um comando que você
mesmo pode rodar. Onde um número não é impressionante, o texto diz isso e diz
por quê.

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
