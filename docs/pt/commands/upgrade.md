# `normfix upgrade`

Substitui o binário em execução pela versão publicada mais nova do seu canal de
atualização.

```sh
normfix upgrade          # baixa, verifica e instala
normfix upgrade --check  # apenas informa
```

```console
$ normfix upgrade --check
normfix 1.6.3 is already the newest release.
```

## O que ele faz, em ordem

1. Seleciona o canal de atualização a partir da versão em execução. Uma build
   estável consulta o endpoint `/releases/latest` do GitHub buscando a versão
   estável mais nova. Uma pré-release acompanha o feed completo de releases,
   podendo avançar para um novo candidato a versão ou para a estável final.
2. Para se você já a estiver executando.
3. Recusa se o binário for gerenciado pelo Homebrew, e informa o comando que faz
   a coisa certa nesse caso.
4. Baixa o arquivo da sua plataforma e o `SHA256SUMS` publicado.
5. **Verifica o digest.** Uma divergência aborta e imprime os dois valores; nada
   é gravado.
6. Extrai em um diretório de preparo *dentro* do destino, para que o passo final
   seja uma renomeação no mesmo sistema de arquivos: o binário ou é substituído,
   ou fica exatamente como estava.

Substituir um executável em execução é seguro no Unix, porque o processo em
execução mantém o arquivo antigo até terminar.

A fronteira entre canais é deliberada: uma instalação estável nunca é movida
para uma beta ou candidata a versão pelo `upgrade` nem pelo aviso diário de
release. Optar por uma pré-release continua sendo uma escolha explícita no
momento da instalação.

## Quando ele recusa

| Situação | O que ele diz |
|---|---|
| Instalado pelo Homebrew | Aponta para `brew upgrade viniciusnevescosta/normfix/normfix` |
| Sem permissão de escrita | Nomeia o caminho e manda conferir o dono; nunca pede `sudo` |
| Checksum divergente | Imprime os dois digests e não instala nada |
| Sem `curl` nem `wget` | Diz qual ferramenta está faltando |
| Plataforma não suportada | Sugere compilar do código-fonte ou usar o playground |

## O aviso de release

Uma execução normal imprime uma linha quando existe uma versão mais nova:

```text
normfix 1.0.0 is available; this is 1.0.0-rc.1. Run `normfix upgrade`.
```

Esse é o único acesso à rede fora do próprio `upgrade`, então ele é
deliberadamente estreito:

- no máximo **uma vez por dia**, com o horário em cache em
  `$XDG_CACHE_HOME/normfix/last-update-check`;
- apenas para **saída humana interativa**, nunca para `--format json` e nunca
  quando o stderr não é um terminal, então scripts e CI não são afetados;
- **silencioso em qualquer falha**, porque um formatador que não alcança a rede
  não tem nada de errado;
- a tentativa é registrada *antes* da requisição, então uma rede inacessível não
  faz toda execução pagar pela mesma consulta.

Desative por completo:

```sh
export NORMFIX_NO_UPDATE_CHECK=1
```

::: tip Nada sobre o seu código sai da máquina
A verificação pede ao GitHub metadados públicos de release. Ela não envia nenhum
caminho, nenhum código-fonte e nenhum identificador de qualquer tipo.
:::
