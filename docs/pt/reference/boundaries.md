# Limites conhecidos

Todo limite abaixo é deliberado. Lê-los é o jeito mais rápido de entender para que serve a ferramenta.

- A compatibilidade exata é testada contra a Norminette 3.3.59; outras versões
  analisáveis rodam com um aviso destacado, a menos que o modo estrito de versão
  esteja ativado.
- Arquivos C precisam ser UTF-8 válido e não conter bytes NUL.
- Recuperação do Tree-sitter ou bytes de fita não classificados desativam as
  edições cientes de sintaxe naquele arquivo.
- A passagem estrita padrão do compilador usa um contexto de include inferido de
  forma conservadora; defines específicos do projeto, modo de linguagem,
  arquivos gerados, flags de alvo, linkagem e comportamento em execução
  continuam sendo responsabilidade do projeto.
- O `-fanalyzer` do GCC pode sugerir possíveis vazamentos, mas não pode provar a
  ausência deles.
- O formatador não infere a arquitetura do projeto, contratos ocultos de
  avaliação, intenção de API pública nem pertencimento a um alvo.
- A extração de funções longas é sugerida, nunca executada automaticamente.
- Um resultado rígido de 80 colunas só é garantido quando existe uma quebra
  segura. Literais longos, comentários, diretivas e expressões ambíguas
  permanecem como avisos.
- A transação de fonte é recuperável e ordenada, mas um sistema de arquivos não
  oferece uma única renomeação atômica abrangendo vários arquivos; o rollback é
  a estratégia de falha entre arquivos.

## Analisadores que o preflight não executa

O `--analyzer` usa o que o compilador já traz: `-fanalyzer` no GCC, o analisador
estático do Clang caso contrário. Outras ferramentas são deliberadamente
deixadas para você, porque cada uma precisa de um build ou de uma execução que o
preflight se recusa a fazer:

| Ferramenta | Por que não é executada |
|---|---|
| `valgrind`, `leaks` | Ferramentas de tempo de execução. Precisam de um binário linkado e de uma carga de trabalho. O comando separado e explícito [`normfix leaks`](/pt/commands/leaks) executa o binário indicado; o preflight nunca faz isso. |
| [AddressSanitizer](https://clang.llvm.org/docs/AddressSanitizer.html), [LeakSanitizer](https://clang.llvm.org/docs/LeakSanitizer.html), UBSan | Builds instrumentados, pelo mesmo motivo. O `preflight` dá uma receita separada de build de depuração sem alterar o Makefile entregue. |
| [clang-tidy](https://clang.llvm.org/extra/clang-tidy/index.html) | Precisa do banco de compilação real do projeto, dos caminhos de include, dos defines e das flags de alvo. O `preflight` informa se ele está disponível, mas não adivinha um comando. |
| `cppcheck`, `scan-build` | Instalações separadas com configuração própria de projeto; integrá-las significaria adivinhar o seu build. |

A regra por trás das quatro linhas é a mesma de todo o resto: um resultado que
esta ferramenta não consegue reproduzir e explicar não é um resultado que ela vai
relatar.
