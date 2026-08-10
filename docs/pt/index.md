---
layout: home
title: normfix em português
description: Correções automáticas seguras e diagnósticos úteis para a Norma da 42.
hero:
  name: normfix
  text: Formate com prova. Revise o restante.
  tagline: Uma ferramenta conservadora para projetos C da 42, com a Norminette oficial como autoridade.
  actions:
    - theme: brand
      text: Primeiros passos
      link: /pt/guide/getting-started
    - theme: alt
      text: Conhecer o playground
      link: /pt/guide/playground
---

## O que o normfix faz

O `normfix` corrige automaticamente apenas transformações que consegue provar
como seguras. O restante aparece como um diagnóstico localizado e acionável,
sem fingir que uma sugestão é uma correção comprovada.

- processa projetos completos ou arquivos específicos `.c`, `.h`, `Makefile` e README;
- inclui o cabeçalho oficial quando uma identidade 42 válida está disponível;
- usa a [Norminette oficial](https://github.com/42school/norminette) para a verificação de compatibilidade;
- oferece prévia, diff, escopos Git, orçamento de funções, backup e undo na CLI;
- oferece um playground privado em WebAssembly, sem enviar o código a um servidor.

A documentação de referência completa ainda está em inglês. As páginas
essenciais de instalação e do playground estão disponíveis em português.
