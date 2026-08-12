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
  - title: Só edições provadas
    details: >-
      As regras propõem substituições em faixas estreitas de bytes contra
      buffers-sombra imutáveis. Uma prova que falha não consegue alterar um
      arquivo pela metade, e tudo que é ambíguo é reportado em vez de reescrito.
  - title: Quem decide é o verificador oficial
    details: >-
      A Norminette oficial instalada é a autoridade. A versão 3.3.59 é a base
      verificada; outra versão continua utilizável, com um aviso visível de
      compatibilidade.
  - title: Recuperável por construção
    details: >-
      As escritas passam por uma única transação auditável, com backups
      externos, journal, commits ordenados, rollback e um undo que falha fechado
      diante de qualquer alvo modificado.
  - title: Privado no navegador
    details: >-
      O playground em WebAssembly reaproveita o mesmo parser nativo e as mesmas
      ações de C dentro da sua aba. Sem upload, conta, analytics ou backend.
---

## O que o normfix é

O `normfix` formata e diagnostica códigos C, headers, Makefiles e documentos
README de projetos da 42. Ele não é um reescritor genérico de C: opera sob as
regras de layout físico da Norma, mantém a
[Norminette oficial](https://github.com/42School/norminette) como autoridade de
compatibilidade, e se recusa a adivinhar onde a sintaxe de C sozinha não prova
que uma mudança é segura.

## O que ele não vai fazer

Cada limite abaixo é deliberado e está documentado na
[política de compatibilidade](/pt/COMPATIBILITY) e
no [registro de arquitetura](/pt/ARCHITECTURE):

- ele não afirma compatibilidade testada das regras nativas para uma versão da
  Norminette diferente da 3.3.59; ele identifica essa versão antes de
  prosseguir;
- ele não extrai funções longas por você, porque escolher onde uma função
  termina muda a estrutura do programa;
- ele não prova ausência de vazamentos, e a saída do analisador permanece
  informativa;
- ele não garante um resultado rígido de 80 colunas quando não existe quebra
  segura;
- ele não apaga nada sem uma concessão explícita de capacidade e sem
  armazenamento externo recuperável.
