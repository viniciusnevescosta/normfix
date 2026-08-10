# Cabeçalhos oficiais da 42

Como o bloco de cabeçalho, a identidade por trás dele e as guardas de inclusão de cabeçalho são tratados.

Cabeçalhos oficiais ausentes são inseridos em fontes C, cabeçalhos C e Makefiles
quando há uma identidade validada disponível. A resolução de identidade usa esta
ordem:

1. `--email`, com verificação opcional de consistência via `--login`;
2. `NORMFIX_EMAIL`, com um login opcional do ambiente ou da CLI;
3. o arquivo de configuração INI persistente por usuário;
4. o `user.email` efetivo do Git, se for um endereço 42 suportado;
5. a variável de ambiente `MAIL`;
6. configurações conhecidas de cabeçalho 42 do Vim, Neovim, VS Code, Cursor e VSCodium.

O e-mail é a fonte da verdade. O login é a parte local antes do `@`; a
ferramenta nunca inventa um endereço nem escolhe silenciosamente entre endereços
salvos ambíguos.

Quando nenhum e-mail válido é encontrado e tanto a entrada quanto a saída de erro
são terminais interativos, o modo humano pergunta:

```text
No verified 42 student email was found.
Enter your 42 email (Enter, cancel, or q to skip the header):
```

Após uma resposta válida, o `normfix` guarda o e-mail/login canônico para
execuções futuras. Enter, `cancel`, `q` ou fim de entrada pulam a inserção do
cabeçalho enquanto todas as outras correções seguras continuam. Execuções JSON e
não interativas nunca perguntam. Ctrl-C cancela o próprio comando, seguindo o
comportamento normal do terminal.

### Configuração persistente de identidade

Fornecer um `--email` válido (com um `--login` correspondente opcional) também
atualiza essa configuração automaticamente. No Unix, o diretório da aplicação
tem modo `0700` e o arquivo substituído atomicamente tem modo `0600`. O e-mail é
dado comum de configuração, não um segredo criptografado.

`NORMFIX_CONFIG` seleciona um caminho absoluto explícito. Caso contrário, o
padrão da plataforma é:

```text
$XDG_CONFIG_HOME/normfix/config.ini                    # explicit XDG base
~/Library/Application Support/normfix/config.ini       # macOS
%APPDATA%\normfix\config.ini                          # Windows
~/.config/normfix/config.ini                           # other Unix
```

O formato suportado é:

```ini
[header]
login = your_login
email = your_login@student.42.fr
```

Configuração por ambiente também é suportada:

```sh
export NORMFIX_LOGIN='your_login'
export NORMFIX_EMAIL='your_login@student.42.fr'
```

Um único carimbo de tempo é capturado para a execução completa. O
`SOURCE_DATE_EPOCH` pode fornecer um carimbo UTC reproduzível; um valor inválido
interrompe a execução em vez de usar silenciosamente o relógio do sistema.

Cabeçalhos válidos existentes mantêm os campos `By` e `Created`. O nome do
arquivo e a linha `Updated` mudam apenas quando o arquivo tem outra edição
aceita ou quando o nome de arquivo do cabeçalho está desatualizado, tornando uma
segunda execução limpa idempotente.

### Guardas de cabeçalho

Para cabeçalhos comuns, o `normfix` pode inserir uma guarda ausente derivada do
nome do arquivo, reparar um par `#ifndef`/`#define` divergente ou renomear uma
guarda simples errada. Toda operação exige uma prova fechada da árvore de
trabalho do Git. A prova varre também os arquivos ignorados, verifica que a
macro esperada não é usada, rejeita guardas duplicadas derivadas do nome do
arquivo e definições dinâmicas de build, e vincula a aprovação aos hashes do
projeto completo e do cabeçalho.

A inserção é recusada em caso de pré-processamento condicional, `#pragma once`,
`#undef` ou colisão com outra macro. Uma renomeação é recusada quando os nomes
antigos têm usos além do par canônico do arquivo inteiro. Cabeçalhos complexos,
referenciados, de inclusão repetida, fora do Git ou ambíguos permanecem
inalterados e recebem um aviso acionável.
