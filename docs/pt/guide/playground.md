# Playground no navegador

O <a href="/pt/" target="_self">playground</a> executa o núcleo do `normfix` em WebAssembly. O editor
Monaco oferece números de linha, busca, múltiplos cursores, pares de colchetes e
realce para C, headers, Markdown e Makefiles. Em celulares, ele usa um editor de
texto leve porque o Monaco não oferece suporte oficial a navegadores móveis.

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
