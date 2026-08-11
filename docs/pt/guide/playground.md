# Playground no navegador

O <a href="/pt/" target="_self">playground</a> executa o núcleo do `normfix` em WebAssembly. O editor
Monaco oferece números de linha, busca, múltiplos cursores, pares de colchetes e
realce para C, headers, Markdown e Makefiles. Em celulares, ele usa um editor de
texto leve porque o Monaco não oferece suporte oficial a navegadores móveis.

## Adicionando seu projeto

Arraste arquivos para a página, ou arraste a própria pasta do projeto. Uma
pasta solta mantém sua estrutura, então `libft/src/ft_strlen.c` chega nesse
caminho, e não achatado num monte de nomes.

Um diretório de projeto real tem mais do que código. Arquivos objeto, o binário
compilado, o `.git` e configurações de editor são ignorados em vez de virarem
erro, e a quantidade de ignorados é sempre exibida — a importação nunca descarta
nada em silêncio, nem recusa o drop inteiro porque um arquivo não é algo que o
normfix formata. **Escolher arquivos** faz o mesmo por um seletor.

## Cabeçalho da 42

Informe um e-mail de estudante válido no painel **Identidade 42**. A opção de
lembrar começa desmarcada. Se você ativá-la, o e-mail fica somente no
armazenamento local desse navegador e pode ser apagado com **Esquecer**. Ele é
passado ao WebAssembly na aba atual para gerar o cabeçalho oficial; nunca é
enviado ao servidor.

## Privacidade e limites

Código e identidade permanecem na aba. A única consulta externa do playground
é a contagem pública de estrelas do repositório no GitHub; se ela falhar, um
valor incluído no site é exibido. Não há upload de código, conta, analytics ou
backend de formatação.

O navegador não executa a [Norminette oficial](https://github.com/42school/norminette),
compilador, Git ou Make. Use a CLI para uma verificação oficial e para backups,
transações e undo.

## Uso offline

O playground se instala na primeira vez que você o abre. Depois disso, a
página, o formatador em WebAssembly e a interface não precisam de rede alguma:
abra o mesmo endereço num avião, no wifi da escola no seu pior dia, ou mesmo
com o site fora do ar, e a formatação roda exatamente como antes. Nada nunca
foi enviado a lugar nenhum, então o uso offline muda como você chega à
ferramenta, não o que ela faz.

O navegador também pode instalá-lo como aplicativo, pela barra de endereço ou
pelo menu. Ele passa a abrir em janela própria, com o nome no idioma que você
escolheu.

Vale saber duas coisas:

- O editor de desktop não faz parte da instalação. O Monaco é um download
  grande que dá realce de sintaxe e busca, então só é baixado quando há
  conexão, e guardado assim que houver. Abrir o playground offline antes disso
  entrega a área de texto simples, que formata de forma idêntica.
- Só o playground fica guardado. A documentação que você está lendo agora é
  outro site e continua precisando de rede.

Uma versão nova nunca substitui a página enquanto você trabalha nela. Ela é
baixada em segundo plano e o cabeçalho oferece **Nova versão pronta** com um
botão **Recarregar**. Até você apertá-lo, continua valendo a versão com que
você começou.
