---
layout: home
title: normfix em português
description: Correções automáticas seguras e diagnósticos úteis para a Norma da 42.
hero:
  name: normfix
  text: Correções seguras para a Norma da 42
  tagline: >-
    Um comando corrige os erros mecânicos de um projeto inteiro da 42, e explica
    os que valem o seu tempo. Suas horas é que são o recurso escasso.
  actions:
    - theme: brand
      text: Por que o normfix
      link: /pt/why
    - theme: alt
      text: Primeiros passos
      link: /pt/guide/getting-started
    - theme: alt
      text: Experimentar no navegador
      link: /pt/guide/playground

features:
  - title: Ele só muda o que consegue provar
    details: >-
      Uma edição toca exatamente o pedaço sobre o qual ele provou alguma coisa, e
      mais nada. O que ele não consegue provar, ele avisa e deixa quieto — então
      dá para rodar no meio do trabalho e ainda ler o diff.
  - title: Quem dá a última palavra é a Norminette
    details: >-
      O normfix nunca discute com a ferramenta que te avalia. Ele roda o
      verificador oficial antes e depois das edições, e joga fora qualquer lote
      que tenha piorado as coisas.
  - title: Nada se perde
    details: >-
      Todo arquivo que ele reescreve é copiado para fora do seu projeto antes. O
      `normfix undo` desfaz uma execução, e se recusa se você mexeu nesses
      arquivos depois.
  - title: Experimente sem instalar
    details: >-
      O playground roda na aba do seu navegador. Nada é enviado, não tem conta, e
      não tem ninguém olhando o que você cola ali.
---

## O que o normfix é

O `normfix` formata e verifica os arquivos C, headers, Makefiles e READMEs de um
projeto da 42. Ele não serve para reescrever C em geral: trabalha dentro das
regras de layout da Norma, trata a
[Norminette oficial](https://github.com/42School/norminette) como a autoridade
sobre o que essas regras significam, e se recusa a adivinhar sempre que a
sintaxe de C sozinha não mostra que uma mudança é segura.

## O que ele não vai fazer

Cada limite abaixo é deliberado e está documentado na
[política de compatibilidade](/pt/COMPATIBILITY) e
no [registro de arquitetura](/pt/ARCHITECTURE):

- ele não vai chamar de suportada uma versão da Norminette que não foi testada —
  ele diz qual encontrou e segue com um aviso;
- ele não vai quebrar uma função longa por você, porque escolher onde cortar
  muda como o seu programa é montado;
- ele não vai provar que o seu programa não vaza memória; o analisador aponta um
  vazamento provável, nunca a ausência de um;
- ele não vai forçar 80 colunas quando não existe um lugar seguro para quebrar a
  linha;
- ele não vai apagar nada sem você pedir, e nunca sem uma cópia de onde dá para
  restaurar.
