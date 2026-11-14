# Avaliação dos fluxos contra o gabarito

Roteiros, limites e modelos mudam com o tempo. A avaliação mostra se um ajuste piorou a análise (spec, critério de aceite 2): roda os fluxos com o Claude Code real no sistema fictício **Faturamento Exemplo** e compara o que o agente registrou com o gabarito `fixtures/gabaritos/faturamento-exemplo.yaml`. Além dos três fluxos de análise (perguntar, explicar, impacto), avalia o fluxo **Analisar reunião** (`reuniao`) com os transcripts fictícios de `fixtures/reunioes/` e mede o custo de uma reunião longa, de cerca de 2 h ([seção própria](#fluxo-reuniao)), o fluxo **Revisar requisito** (`revisar`) com uma história fictícia com problemas plantados ([seção própria](#fluxo-revisar)) o fluxo **Histórias e critérios** (`historias`) a partir da análise de impacto de uma demanda ([seção própria](#fluxo-historias)) e o fluxo **Cenários de teste** (`cenarios`) a partir das regras ([seção própria](#fluxo-cenarios)).

Ela fica na crate `crates/cba-avaliacao` e tem duas partes:

| Parte | O que faz | Onde roda |
|---|---|---|
| Comparação (`cba-avaliacao comparar` e a lib) | Lê o estado de uma análise (`A-NNNN.estado.json`) ou de uma ata (`R-NNNN.estado.json`) e mede a cobertura do gabarito. Determinística, sem modelo. | CI comum (`cargo test --workspace`), com estados de exemplo |
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
| `--fluxos` | `perguntar,explicar,impacto` | fluxos a rodar; também `revisar`, `historias`, `cenarios`, `reuniao` e `reuniao-longa` |
| `--repeticoes` | `1` | execuções de cada fluxo (o modelo varia de uma para outra; use 3 ou mais para comparar versões) |
| `--pergunta`, `--funcionalidade`, `--demanda`, `--revisao`, `--historias`, `--cenarios` | `P-FAT-001`, `F-FAT-002`, `D-FAT-001`, `REV-FAT-001`, `HIS-FAT-001`, `CEN-FAT-001` | item do gabarito de cada fluxo |
| `--reuniao` | `REU-FAT-001` | reunião do gabarito dos fluxos `reuniao` e `reuniao-longa` |
| `--orcamento-longa` | `3` | `--max-budget-usd` da reunião longa, em US$ |
| `--longa-minutos`, `--longa-palavras` | `120`, `25000` | tamanho da reunião longa sintética |
| `--saida` | `target/avaliacao/<data-hora>` | pasta do relatório |
| `--permitir-edicao` | desligado | libera Edit, Write e Bash, para testar o limite "não altera o código" |
| `--tempo-limite` | `900` | segundos por execução |
| `--manter` | desligado | mantém as pastas temporárias, para depurar |

O custo máximo é cerca de `orçamento x fluxos x repetições` (a reunião longa usa `--orcamento-longa`); o `claude` confere o orçamento entre os turnos e pode passar um pouco dele. O programa mostra a estimativa antes de começar. Sai com 0 sem falhas graves, 1 com falha grave e 2 em erro de uso.

Para comparar um estado já gravado (de uma execução anterior ou de uma análise feita à mão na fixture), sem chamar o Claude:

```sh
cargo run -p cba-avaliacao -- comparar --estado <pasta>/estado.json --item D-FAT-001
cargo run -p cba-avaliacao -- comparar --ata <pasta>/ata.estado.json --reuniao REU-FAT-001
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

## Fluxo reuniao

O fluxo **Analisar reunião** monta a ata a partir do transcript (roteiro `crates/cba-tools/textos/roteiros/reuniao.md`). O gabarito tem, em `reunioes`, a ata esperada de cada transcript de `fixtures/reunioes/`: o papel de cada participante e os itens esperados por seção (decisões, necessidades, regras citadas, pendências com responsável e prazo, dúvidas e próximos passos), com palavras-chave e o início das falas que os sustentam. `REU-FAT-001` é a reunião 1 (o `.vtt`); `REU-FAT-002` é a reunião 2, com participantes em comum e o nome da Maria em outra grafia.

```sh
cargo run -p cba-avaliacao -- rodar --fluxos reuniao,reuniao-longa --orcamento 1.5 --orcamento-longa 3
```

### Preparo

Como nos outros fluxos, numa pasta temporária: a fixture copiada, os dados do app e a base. Depois, como o app faz ao receber o transcript: cria uma demanda (com o título do gabarito), cria a reunião nela (`R-0001`, ata em rascunho) e guarda o transcript, lido pelo mesmo parser do app, no diretório de dados temporário (`cba_reunioes::Guarda`). O pedido ao `claude` é `Analise a reunião R-0001 da demanda D-0001 pelo ctower`, e o agente cria a análise com `iniciar_analise` (fluxo `reuniao`, `reuniao` e `demanda`). No fim, a saída de cada execução tem o estado da ata (`ata.estado.json`) e a ata montada (`ata.md`), que só têm os trechos curtos das citações; o transcript fica só na pasta temporária, apagada no fim (menos com `--manter`).

### Reunião longa

`reuniao-longa` mede o custo de uma reunião de cerca de 2 h (questão 4 da #93). O transcript é gerado a partir da reunião escolhida, sem arquivo novo na fixture: a reunião é repetida em blocos, com os horários deslocados, e, a partir do segundo bloco, cada fala ganha antes frases de enchimento sem conteúdo de negócio (retomadas, problemas de áudio), até chegar perto de `--longa-palavras` (25 mil) em `--longa-minutos` (120). O conteúdo de negócio é o da reunião original, então a ata esperada é a mesma. A geração é determinística (`cba_avaliacao::longa`); para comparar de novo uma ata dela, use `comparar --ata ... --reuniao REU-FAT-001 --longa`.

### Critérios

- **Papel acertado.** A proposta do agente (`propor_papel`, campo `proposta` do participante no estado) é o papel esperado ou um dos aceitos (`tambem_aceitos`), sem acentos nem maiúsculas. O participante casa pelo nome normalizado, como no catálogo de pessoas ("Silva, Maria" e "Maria Silva" são o mesmo nome).
- **Item atendido.** Um item registrado da seção do item esperado, ou de uma das seções aceitas (`tambem_em`: uma decisão que também vale registrada como regra), com um termo de cada grupo de palavras-chave no texto (o mesmo critério das perguntas esperadas). Um item registrado pode atender a dois esperados, quando o agente os juntou. Nas pendências, o responsável e o prazo registrados são conferidos à parte, pelos grupos `responsavel` e `prazo`.
- **Fala esperada citada.** O item que atende cita uma das falas do gabarito (`falas`, pelo início). A fala citada é achada pelo falante e pelo início no transcript analisado e comparada com a original pelo texto, o que vale também para a reunião longa.
- **Citações.** O status gravado (`conferida`, `conferida_em`, `nao_confere`) e a conferência de novo de cada citação, dos itens e das propostas de papel, contra o transcript analisado (`cba_reunioes::conferir_citacao`, pelo falante e pelo início).
- **Seções vazias.** As seções sem item e, delas, as que têm itens no gabarito.

Falha grave (reunião): estado da ata ausente, nenhum item na ata, item sem citação, citação gravada como `nao_confere` ou que não confere de novo, erro da execução ou arquivo do projeto alterado. Papéis e itens são cobertura. O relatório traz também o custo em US$, os tokens (`usage` da saída do `claude`: entrada, cache e saída) e o tempo de cada execução, e o tamanho do transcript (falas, palavras e duração).

### Privacidade

Nenhum texto de fala vai para o relatório: ele mostra só identificadores, contagens, os papéis, as descrições do gabarito e os motivos das recusas de citação (que não trazem texto de fala). Os trechos curtos das citações ficam no `ata.estado.json` e no `ata.md` da pasta de cada execução, como na ata do app. O teste `relatorio_da_reuniao_sem_texto_de_fala` confere que nenhuma fala nem trecho citado aparece no relatório.

## Fluxo revisar

O fluxo **Revisar requisito** (`revisar`, #116) confere um requisito ou uma história contra a base e o código e monta o parecer de revisão (roteiro `crates/cba-tools/textos/roteiros/revisar.md`, ferramentas `verificar_termos_vagos`, `registrar_achado`, `registrar_invest` e `registrar_veredito`). O gabarito tem, em `revisoes`, textos fictícios com problemas plantados e o parecer esperado. `REV-FAT-001` é uma história de cancelamento de fatura que contradiz a regra RN-FAT-012 (permite cancelar fatura com pagamento parcial), omite o motivo mínimo (RN-FAT-013) e o supervisor depois de 30 dias (RN-FAT-014) e usa termos vagos da lista de referência ("rápido", "adequada", "e/ou", "etc.").

```sh
cargo run -p cba-avaliacao -- rodar --fluxos revisar --orcamento 1.5
```

A execução é a mesma dos três primeiros fluxos: a análise é criada com o texto do gabarito como entrada, e o pedido é `Inicie a análise A-0001 pelo ctower`. Na base temporária não há regras confirmadas, então a contradição precisa ser achada no código.

### Critérios

- **Achado esperado.** Um achado ativo (não descartado) atende quando é da característica esperada (ou de uma das `tambem_em`), tem um termo de cada grupo de palavras-chave no trecho, no problema ou na reescrita e, se o esperado cita regras, tem evidência válida que toca uma evidência esperada de uma delas (o mesmo critério das regras encontradas) ou cita a regra pelo id. A contradição plantada é o achado esperado de correção.
- **Termo vago apontado.** Cada termo de `termos_vagos` tem um achado de ambiguidade ou de verificabilidade cujo trecho contém o termo, pela mesma lista de referência das ferramentas (`cba_core::qualidade`).
- **Veredito.** O registrado é o esperado.
- **Perguntas esperadas**, como nos outros fluxos (perguntas para o autor).

Falha grave (revisão): achado de correção sem evidência válida, nenhum achado quando o veredito esperado não é `pronta`, estado ausente, erro da execução ou arquivo do projeto alterado. Achados, termos e veredito são cobertura. Para comparar um estado gravado: `cargo run -p cba-avaliacao -- comparar --estado <pasta>/estado.json --item REV-FAT-001`.

O roteiro de revisão é provisório até a primeira execução com o modelo real: por isso ainda não é copiado para a base nem aparece no editor de textos (veja `roteiro::revisado`).

## Fluxo historias

O fluxo **Histórias e critérios** (`historias`, #115) parte de uma análise de origem (de preferência aprovada) e registra histórias de usuário ("Como / quero / para") com critérios de aceite em Dado / Quando / Então, cada critério ligado às afirmações e às regras que o sustentam (roteiro `crates/cba-tools/textos/roteiros/historias.md`, ferramentas `registrar_historia` e `registrar_criterio`). O gabarito tem, em `historias`, o que se espera a partir da análise de impacto de uma demanda. `HIS-FAT-001` parte da `D-FAT-003` (parcelamento para pessoa física, abrangência média).

```sh
cargo run -p cba-avaliacao -- rodar --fluxos historias --orcamento 1.5
```

### Preparo

Antes do `claude`, o preparo cria a análise de origem como o analista a teria deixado: a análise de impacto da demanda, com uma afirmação confirmada por regra da demanda (o enunciado e as evidências do gabarito, conferidas como no `afirmar`), a abrangência e uma pergunta em aberto por pergunta esperada da demanda, concluída e aprovada; e grava as regras da demanda na base (`regras/RN-FAT-007.md`...), como a aprovação de análises anteriores as teria deixado. Depois cria a análise de histórias ligada a ela (`cba_tools::ciclo::criar_analise_de_historias`, a mesma da tela de nova análise), e o pedido é `Inicie a análise A-0002 pelo ctower`.

### Critérios

- **História esperada.** Uma história registrada (não descartada nem quebrada) atende quando o título e a narrativa têm um termo de cada grupo de palavras-chave.
- **Regra citada.** Cada regra esperada é citada por algum critério, pelo identificador (`regras`) ou por uma afirmação cuja evidência toca uma evidência esperada da regra.
- **Valor concreto.** Critérios com ao menos um número no Dado, no Quando ou no Então.
- **A confirmar.** Cada tema de `a_confirmar` tem um critério com pergunta cujo texto tem as palavras-chave.
- **Quebra.** Quando o gabarito pede (`quebra: true`), há proposta de quebra, fatia ou mais de uma história.

Falha grave (histórias): nenhuma história, critério sem origem (a ferramenta já recusa; pegaria uma regressão), estado ausente, erro da execução ou arquivo do projeto alterado. Os demais são cobertura. Para comparar um estado gravado: `cargo run -p cba-avaliacao -- comparar --estado <pasta>/estado.json --item HIS-FAT-001`.

O roteiro de histórias também é provisório até a primeira execução com o modelo real.

## Fluxo cenarios

O fluxo **Cenários de teste** (`cenarios`, #117) deriva das regras os cenários de teste: tabela de decisão para regra com duas ou mais condições (`registrar_tabela_decisao`, completa e sem sobreposição, com as combinações impossíveis justificadas), partição de equivalência e valores-limite com o limite lido do código e a evidência, cenários negativos e de exceção (`registrar_cenario`, roteiro `crates/cba-tools/textos/roteiros/cenarios.md`). O valor do limite precisa aparecer no trecho citado; o limite que vem de tabela de parâmetros fica "a confirmar", com a pergunta. O gabarito tem, em `cenarios`, `CEN-FAT-001` (cancelamento: 10 caracteres no PL/SQL, 30 dias no Delphi, tabela de decisão de RN-FAT-014 e os negativos de RN-FAT-012) e `CEN-FAT-002` (mínimo de 2 parcelas no backend; o máximo, de `MAX_PARCELAS`, a confirmar).

```sh
cargo run -p cba-avaliacao -- rodar --fluxos cenarios --orcamento 1.5
cargo run -p cba-avaliacao -- rodar --fluxos cenarios --cenarios CEN-FAT-002 --orcamento 1.5
```

### Preparo

Antes do `claude`, o preparo grava na base as regras do item (`regras/RN-FAT-012.md`...), com as evidências do gabarito conferidas como no `afirmar`, e cria a análise de cenários com o pedido do gabarito, sem análise de origem (`cba_tools::ciclo::criar_analise_de_cenarios`, a mesma da tela de nova análise).

### Critérios

- **Limite lido do código.** Cada limite esperado é atendido por um cenário da regra cujo limite tem o mesmo valor, não vem de configuração e cita uma evidência que toca a evidência esperada. Limite de outro valor na mesma regra é listado como divergente.
- **Valores testados.** Cada valor de `valores_testados` (o limite e o vizinho do outro lado da comparação) aparece no valor ou nos passos de um cenário da regra.
- **Tabela de decisão.** Cada tabela esperada é atendida por uma tabela com ao menos as condições esperadas, ligada à regra (pelo id ou por evidência) e com as palavras-chave nas condições.
- **Negativos.** Cada negativo esperado é atendido por um cenário da regra com as palavras-chave.
- **A confirmar.** Cada tema de `a_confirmar` tem um cenário com pergunta e as palavras-chave.

Falha grave (cenários): nenhum cenário, cenário sem origem (a ferramenta já recusa; pegaria uma regressão), estado ausente, erro da execução ou arquivo do projeto alterado. Os demais são cobertura. Para comparar um estado gravado: `cargo run -p cba-avaliacao -- comparar --estado <pasta>/estado.json --item CEN-FAT-001`.

O roteiro de cenários também é provisório até a primeira execução com o modelo real. O teste da crate `cba-fixtures` confere que o valor de cada limite esperado aparece no trecho citado da fixture e que a evidência toca a da regra.

## Ao mudar o gabarito ou a fixture

- O teste da crate `cba-fixtures` confere o gabarito, inclusive o formato de `perguntas_esperadas` e `verificacao_da_premissa`, nas revisões, que os termos vagos esperados são exatamente os que a lista de referência acha no texto e que as regras dos achados existem, e, nas reuniões, que o transcript existe, que os participantes são os falantes dele e que as falas citadas pelos itens existem.
- Os testes de `crates/cba-avaliacao/tests/ata.rs` montam uma ata boa e uma ruim a partir do gabarito e do transcript; uma mudança no transcript ou nos itens esperados aparece ali.
- Os testes da crate `cba-avaliacao` usam estados de exemplo montados no teste e estados gravados por execuções reais (`crates/cba-avaliacao/tests/estados/<item>.estado.json`). Se uma mudança na fixture deslocar linhas, as evidências desses estados deixam de conferir: rode a avaliação de novo e troque os estados gravados.

## Exemplo

O relatório da primeira execução, com os três fluxos, está em [`docs/avaliacao/2026-11-12-sonnet.md`](avaliacao/2026-11-12-sonnet.md) (e o JSON ao lado).

O da primeira execução do fluxo `reuniao`, com a reunião 1 e a reunião longa (custo de uma reunião de 2 h), está em [`docs/avaliacao/2026-11-14-reuniao-sonnet.md`](avaliacao/2026-11-14-reuniao-sonnet.md).
