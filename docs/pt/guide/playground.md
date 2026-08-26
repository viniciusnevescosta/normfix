# Playground no navegador

O <a href="/pt/" target="_self">playground</a> é o normfix rodando dentro da aba
do seu navegador. Cole ou arraste um projeto, aperte Executar, e você recebe o
código formatado, os achados que ele conseguiu provar e o diff — sem nada sair
da sua máquina.

É o mesmo código que a linha de comando roda, então o que ele conserta aqui, ele
conserta lá. O que ele não consegue fazer aqui é conferir seu trabalho contra a
Norminette oficial ou um compilador, porque nenhum dos dois existe num
navegador. Todo resultado diz isso.

Em navegadores de desktop, o editor é o Monaco, com números de linha, busca,
múltiplos cursores, pares de colchetes e realce para todos os tipos de arquivo
suportados. Celulares e dispositivos de ponteiro grosso usam uma área de texto
leve, porque o Monaco não oferece suporte oficial a navegadores móveis.

## Adicionando seu projeto

Arraste arquivos para a página, ou arraste a própria pasta do projeto. Uma pasta
solta mantém sua estrutura, então `libft/src/ft_strlen.c` chega nesse caminho, e
não achatado num monte de nomes.

Um diretório de projeto real tem mais do que código. Arquivos objeto, o binário
compilado, o `.git` e configurações de editor são ignorados em vez de virarem
erro, e a quantidade de ignorados é sempre exibida — a importação nunca descarta
nada em silêncio, nem recusa o drop inteiro porque um arquivo não é algo que o
normfix formata. **Escolher arquivos** faz o mesmo por um seletor.

O botão **+** cria um arquivo. Escolha o tipo — `.c`, `.h`, `Makefile` ou `.md` —
em vez de digitar a extensão e descobrir depois que não era uma das quatro. Um
caminho como `src/utils.c` cria a pasta junto, e as pastas aninham quanto você
precisar. O **Baixar tudo (.zip)** mantém essa estrutura.

## Achados sublinhados onde eles estão

Erros e avisos aparecem sublinhados no editor do mesmo jeito que o seu editor
sublinha, para você parar de cruzar um número de linha numa lista com uma linha
no arquivo. Passando o mouse, aparece a regra e a explicação.

Um achado sem posição — um cabeçalho 42 inválido pertence ao arquivo, não a uma
linha — fica de fora dos sublinhados em vez de ser desenhado em algum lugar
qualquer. Ele continua no painel de diagnósticos.

## Aparência

**Sistema**, **Claro** ou **Escuro**, ao lado do seletor de idioma. Segue o seu
sistema operacional a menos que você diga o contrário, e a escolha fica salva
neste dispositivo até você trocá-la — como o idioma, ela muda a aparência da
página e nada mais: nenhuma execução, nenhuma requisição, nenhum recarregamento.

## Cabeçalho oficial da 42

Informe um e-mail de estudante válido no painel **Identidade 42**. A opção
**Lembrar neste dispositivo** começa desmarcada. Quando você a ativa
explicitamente, o endereço fica guardado apenas no armazenamento local de mesma
origem deste navegador e pode ser removido a qualquer momento com **Esquecer**.
Caso contrário, ele vale só para a aba atual.

O endereço é passado ao WebAssembly dentro da aba para gerar o cabeçalho oficial
da 42. Ele nunca é enviado a um servidor de formatação. Sem uma identidade
válida, o código continua sem cabeçalho gerado e o resultado inclui um
diagnóstico dizendo isso.

## Aproveitando o resultado

Uma execução sempre cobre o projeto inteiro, porque um header e o arquivo que o
inclui só são julgados corretamente juntos. A escolha é o que fazer com a
resposta: aplicar de uma vez tudo que foi provado, ou apenas o que está à sua
frente. Em ambos os casos, uma correção deixa de valer se o arquivo foi editado
depois da execução, já que ela foi provada contra o código que o normfix leu, e
não contra o que está no editor agora.

- **Corrigir todos os arquivos** grava de uma vez, no projeto, todo resultado
  provado.
- **Corrigir este arquivo** faz o mesmo para o arquivo que você está vendo.
- **Copiar arquivo** copia o resultado estável selecionado. Se o acesso à área
  de transferência for negado, o navegador seleciona o texto para você copiar
  pelo teclado.
- **Baixar arquivo** salva o resultado selecionado.
- **Baixar tudo (.zip)** salva todos os resultados estáveis num único arquivo
  que qualquer sistema de desktop abre sem instalar nada.
- **Usar como nova entrada** devolve um resultado ao editor para outra execução.

## Privacidade e comportamento de rede

Código e identidade permanecem na aba. Não há upload de código, conta,
dependência de analytics nem backend de formatação. Abrir o playground não faz
requisições a terceiros; o GitHub e as ferramentas oficiais são links comuns,
acessados somente quando você escolhe.

## Uso offline

O playground se instala na primeira vez que você o abre. Depois disso, a página,
o formatador em WebAssembly e a interface não precisam de rede alguma: abra o
mesmo endereço num avião, no wifi da escola no seu pior dia, ou mesmo com o site
fora do ar, e a formatação roda exatamente como antes. Nada nunca foi enviado a
lugar nenhum, então o uso offline muda como você chega à ferramenta, não o que
ela faz.

O navegador também pode instalá-lo como aplicativo, pela barra de endereço ou
pelo menu. Ele passa a abrir em janela própria, com o nome no idioma que você
escolheu.

Vale saber duas coisas:

- O editor de desktop não faz parte da instalação. O Monaco é um download grande
  que dá realce de sintaxe e busca, então só é baixado quando há conexão, e
  guardado assim que houver. Abrir o playground offline antes disso entrega a
  área de texto simples, que formata de forma idêntica.
- Só o playground fica guardado. A documentação que você está lendo agora é
  outro site e continua precisando de rede.

Uma versão nova nunca substitui a página enquanto você trabalha nela. Ela é
baixada em segundo plano e o cabeçalho oferece **Nova versão pronta** com um
botão **Recarregar**. Até você apertá-lo, continua valendo a versão com que você
começou.

## Limites entre a CLI e o playground

| Recurso | CLI | Playground |
|---|---:|---:|
| Formatação segura de C e headers | sim | sim |
| Formatação segura de Makefile e Markdown | sim | sim |
| Cabeçalho oficial da 42 a partir de uma identidade informada | sim | sim |
| Diagnósticos estruturais e orçamentos de função | sim | sim |
| Diffs unificados | sim | sim |
| Verificação pela Norminette oficial | sim | não |
| Preflight estrito do compilador e analisador | sim | não |
| Descoberta automática de identidade | sim | não |
| Escopos do Git | sim | não |
| Backups, transações e undo | sim | não |

O sandbox do navegador não executa o binário da
[Norminette oficial](https://github.com/42school/norminette), um compilador, o
Git nem o Make. Use a [linha de comando](/pt/guide/command-line) para a verificação oficial e para o fluxo completo de preparação
para a defesa.

## Limites e portabilidade

O playground aceita no máximo 128 arquivos, 1 MiB por arquivo e 4 MiB no total.
Os caminhos precisam ser relativos, portáteis e normalizados em NFC, com no
máximo 240 bytes UTF-8. Ele rejeita duplicatas que colidem em sistemas
insensíveis a maiúsculas, nomes reservados de plataforma, UTF-8 inválido e
caminhos inseguros para arquivamento antes mesmo de executar o formatador. Um
BOM UTF-8 no início é consumido de forma consistente. Qualquer resultado do
formatador que não chegue a um ponto fixo é descartado, em vez de ser exposto
como uma edição parcial aparentemente utilizável.

## Executando localmente

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
npm ci
npm run dev
```

A compilação também exige uma instalação do Clang com o alvo WebAssembly
funcionando. No macOS, o build procura os caminhos do LLVM do Homebrew e explica
como instalar o LLVM quando o compilador do sistema não consegue gerar código
para `wasm32`.
