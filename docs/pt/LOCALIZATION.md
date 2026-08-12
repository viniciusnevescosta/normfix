# Guia de localização

Este é o contrato de quem contribui traduzindo cada superfície do normfix que
uma pessoa lê. Os primeiros idiomas publicados são inglês (`en`), português
(`pt`), espanhol (`es`) e francês (`fr`). Um idioma só está completo quando
alguém consegue instalar a ferramenta, entender seu limite de segurança, usar o
playground no navegador e seguir a documentação central sem cair no inglês.

A localização não pode mudar a interface de máquina. Traduza explicações, não
identificadores.

## O que permanece em inglês

Estes valores são tokens estáveis de API ou de código-fonte e precisam continuar
inalterados em todos os idiomas:

- o comando `normfix` e os nomes dos subcomandos;
- flags como `--check`, `--changed` e `--format json`;
- IDs de regra como `TOO_MANY_LINES` e `MAKEFILE_SOURCE_NOT_FOUND`;
- chaves JSON, valores de enum, `schema_version` e códigos de saída;
- chaves de configuração, nomes de variáveis de ambiente e nomes de arquivos;
- identificadores C, comandos de shell, caminhos, nomes de arquivos compactados e
  exemplos de código;
- mensagens de commit do Git e comentários de código em Rust/TypeScript.

Mantenha inalterados os nomes oficiais de produto — Norminette, Rust, WSL, Clang,
Vite, Monaco, Git, GitHub e Vercel. Traduza a frase ao redor deles e preserve o
link oficial.

## Superfícies atuais

| Superfície | Origem do texto traduzido | Comportamento publicado |
|---|---|---|
| Playground no navegador | `web/src/i18n.ts` e atributos `data-i18n*` em `web/index.html` | Interface completa em `en`, `pt`, `es` e `fr`; escolher um idioma apenas muda o idioma, e a escolha é lembrada até ser trocada |
| Playground instalado | Um web app manifest por idioma, gerado em `web/vite.config.ts` | Cada idioma instala com nome, identidade e URL inicial próprios, então um playground instalado abre no idioma que o leitor escolheu |
| Documentação | Árvores de idioma dentro de `docs/`, mais a navegação por idioma em `docs/.vitepress/config.ts` | Landing, instalação, playground, segurança, compatibilidade e caminhos de contribuição localizados, com o inglês como fallback explícito para páginas ainda não publicadas |
| SEO | Head/config do VitePress, `web/index.html`, sitemaps e `robots.txt` | URLs canônicas e `hreflang` apenas para páginas que realmente existem |
| CLI nativa | Catálogo em `crates/normfix-i18n`, selecionado com `--lang` ou pelo locale do processo | Anúncio, texto do relatório, prompts de segurança, artigos do `explain` e os diagnósticos deste projeto em `en`, `pt`, `es` e `fr`; achados repassados do verificador oficial ou do compilador C ficam como aquelas ferramentas os produziram; comandos, flags, JSON, IDs de regra e códigos de saída permanecem neutros de idioma |

## Traduzindo o playground

1. Adicione o código do idioma a `SUPPORTED_LOCALES` em `web/src/i18n.ts`.
2. Preencha cada `MessageKey`. Não publique um idioma que herde em silêncio um
   botão, um erro de validação, uma declaração de privacidade ou um rótulo de
   acessibilidade em inglês.
3. Coloque o texto estático do HTML atrás de `data-i18n`, `data-i18n-title`,
   `data-i18n-placeholder` ou `data-i18n-aria`. Coloque o texto dinâmico atrás de
   `translate()`; não deixe literais de texto voltado ao usuário em
   `web/src/main.ts` nem em `web/src/editor.ts`.
4. Use placeholders nomeados como `{path}` e `{count}`. Toda tradução precisa
   preservar exatamente o mesmo conjunto de placeholders do inglês.
5. Quando uma mensagem contém uma contagem, não escreva uma única frase com um
   placeholder dentro. Adicione uma entrada por categoria plural do CLDR —
   `importedOne`, `importedOther` — e renderize com `translatePlural`, para que o
   substantivo concorde com o número em vez de sair “1 arquivos adicionados”. Uma
   mensagem pluralizada precisa depender de exatamente uma contagem; uma frase
   com duas não consegue concordar em todos os idiomas, então escreva duas
   frases.
6. Formate números e datas com o idioma selecionado. Não localize o timestamp
   fixo do cabeçalho da 42 nem outro texto de protocolo.
7. Defina o valor `lang` do documento e ofereça um seletor de idioma visível.
8. Nunca injete uma tradução com `innerHTML`. Continue usando `textContent` e nós
   do DOM, para que texto traduzido ou versionado não possa virar markup.
9. Teste o fallback de textarea em tela estreita, além do Monaco. O Monaco em si
   não define a completude da localização do produto. O caminho offline usa esse
   mesmo fallback, então é também o que um leitor vê ao abrir o playground
   instalado sem rede.
10. Traduza o nome do aplicativo em `localizedPages`, em `web/vite.config.ts`. Ele
    é o rótulo embaixo do ícone de quem instalar o playground, então precisa ser
    curto e soar como um nome, não como título de página.

Os diagnósticos nativos em Rust devolvidos pelo WebAssembly continuam em inglês.
A interface precisa dizer isso claramente, em vez de apresentar um diagnóstico
parcialmente traduzido como localização completa.

## Traduzindo a documentação

Use a página em inglês como fonte da verdade. Preserve os títulos que são alvo de
link, a menos que a configuração do idioma também forneça um redirecionamento
testado. Mantenha os exemplos de comando válidos byte a byte; traduza apenas o
texto ao redor e a saída humana esperada.

Para um idioma novo:

1. crie o diretório do idioma e traduza a landing page;
2. traduza primeiros passos, o guia do playground no navegador,
   segurança/recuperação, compatibilidade e este guia de localização antes de
   anunciar o idioma;
3. adicione rótulos, navegação, sidebar, rótulos de busca, rodapé e texto do link
   de edição localizados no VitePress;
4. faça link para as páginas oficiais da Norminette, do Rust, do WSL e do Clang
   sempre que essas ferramentas forem citadas como dependências;
5. adicione metadados canônicos e de idioma alternativo apenas entre páginas
   traduzidas equivalentes;
6. inclua cada URL localizada publicada no sitemap gerado;
7. verifique cada link interno e cada bloco de código no build de produção.

Não crie uma página fina cujo único conteúdo é um redirecionamento automático
para o inglês e chame isso de tradução. Um link explícito “Esta página está
disponível em inglês” é um fallback temporário aceitável quando a rota localizada
não é anunciada como completa.

## Traduzindo a CLI nativa

A crate `crates/normfix-i18n` é dona da seleção de idioma e do catálogo. O texto
traduzido vive ali, nunca dentro do código que decide o que dizer.

A completude é garantida pelo compilador, não pela revisão. Cada idioma é um
único literal de struct `Messages`, então uma entrada nova que algum idioma não
traduza é um erro de compilação. Dois testes cobrem o que o sistema de tipos não
alcança: nenhuma entrada pode estar vazia, e cada tradução precisa carregar o
mesmo conjunto de `{placeholder}` do original em inglês. Os placeholders são
nomeados, não posicionais, então uma tradução pode reordená-los.

Para adicionar uma entrada:

1. adicione o campo a `Messages` com um comentário de documentação nomeando seus
   placeholders;
2. preencha-o nos quatro literais de idioma na mesma mudança;
3. renderize através de `messages.<campo>` e `normfix_i18n::fill`, nunca como um
   literal no ponto de chamada.

A seleção de idioma segue `--lang`, depois `NORMFIX_LANG`, `LC_ALL`,
`LC_MESSAGES` e `LANG`, e então o inglês. Só o subtag primário importa, então
`pt_BR.UTF-8` seleciona português. Um valor de `--lang` não publicado cai no
inglês com um aviso conciso; um locale de processo não publicado cai em silêncio,
porque uma dica não é uma decisão. Nenhum dos dois casos é fatal: o idioma da
saída não pode ser motivo para recusar analisar um projeto.

O JSON nunca é localizado. O evento `execution_start` e o relatório final mantêm
valores em inglês em todos os idiomas, então um script nunca precisa escolher um
idioma para continuar confiável.

### O que é traduzido, e o que nunca será

Traduzido: o anúncio da execução, o texto do próprio relatório, todos os prompts
críticos de segurança, os artigos do `explain` e os diagnósticos que este projeto
escreve.

Nunca traduzido: um achado repassado da Norminette oficial ou do compilador C.
Aquele texto é a saída daquelas ferramentas. Reescrevê-lo faria o relatório
discordar do que rodar `norminette` diretamente imprime, o que é pior do que ler
uma frase em inglês. Uma execução em outro idioma diz isso em uma linha — como um
fato sobre a origem daquelas palavras, não como desculpa por uma tradução
faltando.

Os tokens de status na tabela de arquivos (`CLEAN`, `WOULD FIX`, `REVIEW`,
`FAILED`) e as palavras de severidade continuam em inglês, junto dos IDs de regra
ao lado dos quais aparecem.

Para traduzir um diagnóstico novo, adicione um `DiagnosticKey`, preencha-o nos
quatro `match` de idioma e construa-o com `localized_text`. O inglês é sempre
produzido, porque é ele que chega ao JSON e é usado pela igualdade e pela
ordenação.

## Terminologia e tom

- Use o vocabulário que os estudantes já veem na 42.
- Prefira frases curtas e diretas em avisos e botões.
- Mantenha precisa a distinção entre **aviso**, **falha**, **inseguro**,
  **recuperável**, **informativo** e **conclusivo**.
- Não traduza “safe” como “garantidamente correto”. Significa que a prova
  documentada daquela edição passou.
- Não traduza a estimativa de pré-defesa como uma nota oficial.
- Preserve a afirmação de que a identidade no navegador é configuração local do
  dispositivo, não um segredo criptografado.

Quando um termo for disputado, atualize um pequeno glossário nas notas de quem
contribui naquele idioma e use uma grafia consistente entre playground e
documentação.

## Validação

Rode as verificações completas do site depois de qualquer mudança de localização:

```sh
npm ci
npm run typecheck
npm audit --audit-level=moderate
npm run build
```

Depois revise cada idioma em largura de desktop e em telas estreitas. Confira
acesso por teclado, rótulos de foco, estouro de texto, texto de plural e
contagem, comportamento do botão de copiar código, links quebrados, URLs
canônicas, `hreflang` e o sitemap. Alguém fluente no idioma de destino deve
aprovar sentido e tom; um build de TypeScript que passa só prova o formato do
catálogo.

Para uma mudança no catálogo da CLI, rode também os testes do workspace Rust, o
Clippy com avisos negados, o rustdoc com avisos negados e as fixtures do schema
JSON.

## Checklist do pull request

- [ ] Cada texto novo voltado a pessoas está no catálogo correto.
- [ ] Comandos, flags, IDs de regra, chaves JSON e exemplos de código estão
      inalterados.
- [ ] Os nomes dos placeholders e o significado de segurança correspondem ao
      inglês.
- [ ] Navegação, rótulos de acessibilidade, metadados e caminhos de erro estão
      traduzidos.
- [ ] Entradas canônicas, `hreflang` e do sitemap apontam apenas para páginas
      reais.
- [ ] Os links das dependências oficiais foram mantidos.
- [ ] Os portões do site e do Rust relevantes para a mudança passam.
- [ ] Alguém fluente conferiu o resultado renderizado, não apenas o diff.
