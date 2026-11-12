# Avaliação dos fluxos contra o gabarito

Roteiros, limites e modelos mudam com o tempo. A avaliação mostra se um ajuste piorou a análise (spec, critério de aceite 2): roda os três fluxos com o Claude Code real no sistema fictício **Faturamento Exemplo** e compara o que o agente registrou com o gabarito `fixtures/gabaritos/faturamento-exemplo.yaml`.

Ela fica na crate `crates/cba-avaliacao` e tem duas partes:

| Parte | O que faz | Onde roda |
|---|---|---|
| Comparação (`cba-avaliacao comparar` e a lib) | Lê o estado de uma análise (`A-NNNN.estado.json`) e mede a cobertura do gabarito. Determinística, sem modelo. | CI comum (`cargo test --workspace`), com estados de exemplo |
| Execução (`cba-avaliacao rodar`) | Roda os fluxos com o Claude Code numa cópia da fixture e grava o relatório. | Manual, com a assinatura de quem roda. Fora do CI comum |

## Quando rodar

Antes de abrir um PR que mude:

- um roteiro (`crates/cba-tools/textos/roteiros/*.md`) ou as dicas das seções;
- os limites (`crates/cba-session/prompts/limites.md`);
- as descrições ou os schemas das ferramentas MCP;
- a fixture ou o gabarito.

Rode também quando sair uma versão nova do Claude Code ou do modelo padrão. Rode antes e depois da mudança, com os mesmos parâmetros, e compare os dois relatórios. Ponha no PR o resumo do relatório e o custo.

## Como rodar

Pré-requisitos: o Claude Code instalado e logado (`claude auth status`) e o executável `cba` compilado.

```sh
(cd ui && npm ci && npm run build)   # o cba-app embute ui/dist
cargo build -p cba-app               # gera target/debug/cba
cargo run -p cba-avaliacao -- rodar --modelo sonnet --orcamento 1.5
```

O relatório sai em `target/avaliacao/<data-hora>/relatorio.md` e `relatorio.json`, com uma pasta por execução contendo a saída do `claude` (`claude.json`), o estado da análise (`estado.json`) e o documento gerado (`documento.md`).

Opções principais:

| Opção | Padrão | O que é |
|---|---|---|
| `--modelo` | `sonnet` | modelo do `claude` |
| `--orcamento` | `1.5` | `--max-budget-usd` de **cada** execução, em US$ |
| `--fluxos` | `perguntar,explicar,impacto` | fluxos a rodar |
| `--repeticoes` | `1` | execuções de cada fluxo (o modelo varia de uma para outra; use 3 ou mais para comparar versões) |
| `--pergunta`, `--funcionalidade`, `--demanda` | `P-FAT-001`, `F-FAT-002`, `D-FAT-001` | item do gabarito de cada fluxo |
| `--saida` | `target/avaliacao/<data-hora>` | pasta do relatório |
| `--permitir-edicao` | desligado | libera Edit, Write e Bash, para testar o limite "não altera o código" |
| `--tempo-limite` | `900` | segundos por execução |
| `--manter` | desligado | mantém as pastas temporárias, para depurar |

O custo máximo é cerca de `orçamento x fluxos x repetições`; o `claude` confere o orçamento entre os turnos e pode passar um pouco dele. O programa mostra a estimativa antes de começar. Sai com 0 sem falhas graves, 1 com falha grave e 2 em erro de uso.

Para comparar um estado já gravado (de uma execução anterior ou de uma análise feita à mão na fixture), sem chamar o Claude:

```sh
cargo run -p cba-avaliacao -- comparar --estado <pasta>/estado.json --item D-FAT-001
```

### Isolamento

Cada execução trabalha numa pasta temporária, sem tocar na configuração do Claude Code de quem roda:

- a fixture é copiada e vira um repositório git novo; dados do app (`CBA_DATA_DIR`) e base de conhecimento também são temporários;
- o projeto é cadastrado e a análise é criada como o app faz (spec, 4.2), e o pedido é o mesmo do app: `Inicie a análise A-0001 pelo ctower`;
- o servidor MCP e os hooks são gerados pelo mesmo preparo do app (`cba_session::preparo`), em arquivos temporários, e passados com `--mcp-config <arquivo> --strict-mcp-config --settings <arquivo> --setting-sources local --no-session-persistence`: nenhum MCP, hook ou permissão do usuário entra, e a sessão não fica no histórico;
- o agente pode usar Read, Grep, Glob e as ferramentas do `cba`; Edit, Write e Bash ficam negados, a não ser com `--permitir-edicao`.

O `claude` usa o login de quem roda. O programa não lê nem grava credenciais.

## Critérios de comparação

Para cada execução, o estado da análise é comparado com o item do gabarito:

- **Regras encontradas.** Uma regra (`RN-*`) é encontrada quando uma evidência válida de alguma afirmação está no mesmo arquivo de uma evidência esperada da regra e os intervalos de linhas se sobrepõem. Uma inconsistência entre camadas (`INC-*`) exige que **uma mesma afirmação** toque duas evidências esperadas dela, isto é, os dois lados da divergência. Evidências com mais de 30 linhas não contam, para um intervalo largo não "achar" todas as regras de um arquivo. Afirmações marcadas como incorretas não contam.
- **Evidências válidas.** Cada evidência passa de novo pela validação do `afirmar` (`cba_tools::evidencias::validar`) contra a fixture, e o hash do intervalo precisa ser o gravado.
- **Nenhuma afirmação `confirmado` sem evidência válida.** Sem evidência, ou com alguma evidência inválida, é falha grave.
- **Perguntas esperadas.** No gabarito, cada item pode ter `perguntas_esperadas`, com grupos de palavras-chave. Uma pergunta registrada atende quando cada grupo tem ao menos um termo no texto. A comparação ignora maiúsculas e acentos; o termo casa com a palavra inteira (`12` não casa com `120`) ou, terminando em `*`, com o começo de uma palavra (`parcel*` casa com `parcelamento`). Palavras-chave, e não semelhança de texto, para o critério ser explicável e estável.
- **Premissa corrigida** (impacto). No gabarito, `verificacao_da_premissa` dá o veredito esperado e as palavras-chave da suposição. Vale uma afirmação da seção `demanda_como_entendida`, com evidência válida, no formato do roteiro ("Premissa do solicitante: ... Verificação: não confere | confere em parte"), com o veredito esperado e as palavras-chave. Quando o esperado é `confere_em_parte`, `não confere` também vale.
- **Abrangência** (impacto). A afirmação mais recente da seção `abrangencia` começa pela estimativa esperada.
- **Arquivos esperados citados.** Arquivos da funcionalidade ou da demanda que aparecem em evidências válidas.
- **Nenhum arquivo do projeto alterado.** A cópia é comparada byte a byte com a fixture (menos `.git`).

Falha grave: afirmação `confirmado` sem evidência válida, nenhuma afirmação, estado ausente, erro da execução ou arquivo do projeto alterado. As demais medidas são cobertura: não reprovam a execução, mas uma queda entre dois relatórios indica que a mudança piorou a análise.

O relatório também lista, como informação, as regras de outros itens que a análise encontrou e as afirmações com evidência válida que não tocam nenhuma evidência do gabarito.

## Ao mudar o gabarito ou a fixture

- O teste da crate `cba-fixtures` confere o gabarito, inclusive o formato de `perguntas_esperadas` e `verificacao_da_premissa`.
- Os testes da crate `cba-avaliacao` usam estados de exemplo montados no teste e estados gravados por execuções reais (`crates/cba-avaliacao/tests/estados/<item>.estado.json`). Se uma mudança na fixture deslocar linhas, as evidências desses estados deixam de conferir: rode a avaliação de novo e troque os estados gravados.

## Exemplo

O relatório da primeira execução, com os três fluxos, está em [`docs/avaliacao/2026-11-12-sonnet.md`](avaliacao/2026-11-12-sonnet.md) (e o JSON ao lado).
