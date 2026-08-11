# Guia de localização

As interfaces publicadas são inglês (`en`), português (`pt`), espanhol (`es`)
e francês (`fr`). Uma tradução precisa permitir instalar a ferramenta,
entender seu limite de segurança e usar o playground sem textos residuais de
interface em inglês.

## O que não deve ser traduzido

Mantenha `normfix`, subcomandos, flags, IDs como `TOO_MANY_LINES`, chaves JSON,
`schema_version`, códigos de saída, nomes de configuração, caminhos e exemplos
de código exatamente como estão. Preserve os nomes oficiais Norminette, Rust,
WSL, Clang, Vite, Monaco, Git, GitHub e Vercel, com seus links oficiais.

## Playground

Adicione o idioma a `SUPPORTED_LOCALES` em `web/src/i18n.ts` e traduza cada
`MessageKey`, inclusive validações, privacidade, títulos e rótulos acessíveis.
Texto estático deve usar `data-i18n`, `data-i18n-title`,
`data-i18n-placeholder` ou `data-i18n-aria`; texto dinâmico deve usar
`translate()`. Preserve placeholders como `{path}` e `{count}` e nunca injete
traduções com `innerHTML`.

Os diagnósticos que o próprio normfix escreve são traduzidos. Um achado
repassado da Norminette oficial ou do compilador continua no idioma em que
aquela ferramenta o produziu, para que o relatório não discorde do que
`norminette` imprime. A interface deve dizer de onde vêm essas palavras, e não
prometer uma tradução futura.

Cada idioma publica seu próprio web app manifest, gerado em
`web/vite.config.ts`. Traduza o nome do aplicativo junto com a página: é o
rótulo que aparece embaixo do ícone de quem instalar o playground.

## Documentação e validação

Antes de anunciar um idioma, traduza a landing page, primeiros passos,
playground, segurança/recuperação, compatibilidade e este guia. Adicione a
navegação, sitemap, canonical e `hreflang` somente para rotas reais.

```sh
npm ci
npm run typecheck
npm audit --audit-level=moderate
npm run build
```

Revise desktop e mobile, teclado, foco, overflow, links, metadados e sitemap.
Uma pessoa fluente deve aprovar significado e tom; o build só comprova a forma
do catálogo.
