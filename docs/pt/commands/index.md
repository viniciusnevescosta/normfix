# Comandos

A interface sem subcomando é o caminho mais curto para formatar um projeto, e é
o que a maioria das execuções usa:

```sh
cd caminho/para/um/projeto-42
normfix
```

Os subcomandos tornam a intenção explícita, o que importa em scripts, na CI e
durante uma revisão.

| Comando | Grava | Use quando |
|---|---|---|
| [`format`](/pt/commands/format) | sim | Você quer aplicar as edições aceitas |
| [`lint`](/pt/commands/lint) | não | Você quer diagnósticos sobre os bytes em disco, sem nada proposto |
| [`check`](/pt/commands/check) | não | Você quer ver o que uma execução de correção *faria* |
| [`budget`](/pt/commands/budget) | não | Você quer a folga de linhas/variáveis/parâmetros por função |
| [`preflight`](/pt/commands/preflight) | não | Você vai defender e quer as verificações somente leitura |
| [`explain`](/pt/commands/explain) | não | Você quer uma regra explicada sem varrer nada |
| [`undo`](/pt/commands/undo) | sim | Você quer restaurar uma execução anterior |
| [`upgrade`](/pt/commands/upgrade) | sim | Você quer a versão mais nova, verificada |
| [`uninstall`](/pt/commands/uninstall) | sim | Você quer o normfix removido desta máquina |

## Todo exemplo destas páginas é real

A saída mostrada foi produzida por `normfix 1.3.0` sobre este arquivo:

```c
# include "libft.h"
# include <stdlib.h>

int add(int a,int b){
return a+b;
}

int	scale(int value, int factor)
{
	int result;
	result = value * factor;
	return result;
}
```

Ele é propositalmente bagunçado de maneiras comuns: includes fora de ordem, uma
definição de função colapsada, espaços faltando, uma declaração não separada
das instruções e valores de `return` sem parênteses.

## Códigos de saída

Todos os comandos os compartilham:

| Código | Significado |
|---:|---|
| `0` | Nada bloqueante: a execução ficou limpa, ou o modo de correção terminou |
| `1` | Restam diagnósticos manuais, ou uma pré-visualização encontrou mudanças propostas |
| `2` | Falha de descoberta, configuração, ferramenta, E/S, transação ou quarentena |
| `130` | Uma revisão interativa foi cancelada |

Avisos informativos nunca alteram o código de saída. Isso torna os códigos
usáveis diretamente na CI:

```sh
normfix --check || echo "este projeto ainda não está limpo segundo a Norm"
```

## Flags que todo comando aceita

`--format json` e `--no-color` mudam a saída; `--threads`, `--timeout`,
`--no-cache` e `--norminette PATH` mudam como a execução acontece. A tabela
completa está em [linha de comando](/pt/guide/command-line).
