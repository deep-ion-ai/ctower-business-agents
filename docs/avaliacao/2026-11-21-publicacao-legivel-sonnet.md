# Avaliação do ciclo de publicação com a documentação legível: 2026-11-21 (sonnet)

Avaliação do cenário `publicacao` (#291, [`docs/avaliacao.md`](../avaliacao.md#ciclo-de-publicação)) depois da documentação legível (#287 a #290): analisar P-FAT-001, aprovar, publicar as páginas legíveis num repositório de docs local (PR numa forja falsa, dado como mesclado), sincronizar numa base nova (recortando as páginas pelos marcadores) e analisar P-FAT-003 nela, conferindo que o agente recebeu da documentação publicada uma **regra recortada de página, com a âncora**, e não só "algum documento externo". Comando:

```sh
cargo run -p cba-avaliacao -- rodar --modelo sonnet --orcamento 1.5 --fluxos publicacao --repeticoes 3
```

Abaixo, o `relatorio.md` da rodada final, sem edição.

## Rodadas antes do relatório

1. **Primeira rodada (3 execuções, US$ 1,38), roteiros da #279, critério novo.** O ciclo inteiro funcionou nas três: análise aprovada (5 ou 6 regras), 4 páginas publicadas (`README.md`, `regras/geral.md`, `analises/README.md`, `analises/A-0001.md`, mais o manifesto), 9 ou 10 documentos externos na base nova (um por bloco, mais as páginas sem marcador e o leia-me). Em duas, o agente recebeu 3 regras recortadas (`regras/geral.md#rn-ger-003` a `#rn-ger-005`). Na terceira, **falha grave pelo critério novo**: o agente chamou `conhecimento` uma vez só, com `"juros multa atraso carência"`; a busca exige todas as palavras e só o bloco da análise publicada (`analises/A-0001.md#a-0001`) tinha as quatro, então nenhuma regra recortada chegou. Ajuste nos roteiros de perguntar, explicar, impacto e revisar: consultar a documentação publicada **um conceito por chamada** (`juros`, depois `carência`), porque uma análise que cita todos os conceitos esconde as regras de cada um.
2. **Rodada final (3 execuções, US$ 1,40), abaixo.**

Resumo da rodada final: nenhuma falha grave. Nas três, o agente chamou `conhecimento` duas vezes (`juros` e `carência`, com `externos: "incluir"`) e recebeu 6 blocos recortados, 5 deles regras (entre `regras/geral.md#rn-ger-001` e `#rn-ger-006`), cada um com o link do trecho na forja no commit sincronizado (`https://github.com/exemplo/faturamento-docs/blob/<commit>/regras/geral.md#rn-ger-002`); citou a fonte na resposta (repositório e commit, "só como pista") e registrou só código como evidência. Regras do P-FAT-003 encontradas nas três (2 de 2; 32 evidências válidas de 32). Custo do ciclo: US$ 0,44 a 0,48 (teto de US$ 3,00).

Trecho da transcrição (repetição 1, segunda análise), o que o agente recebeu de `conhecimento` com `termo: "carência"`:

| Fonte | Tipo | Id | Trecho (página#âncora) |
|---|---|---|---|
| externa | regra | RN-GER-002 | `regras/geral.md#rn-ger-002` (Carência: não há multa enquanto os dias de atraso forem menores…) |
| externa | pergunta | A-0001 | `analises/A-0001.md#a-0001` |
| externa | página sem marcador | | `analises/README.md` |

Na resposta final: "**Fontes externas:** consultei a documentação publicada em `exemplo/faturamento-docs` (commit 4fbbef0) só como pista. Todas as evidências registradas são trechos do código."

O Faturamento Exemplo da avaliação não tem mapa de funcionalidades, então as regras vão para a página da área (`regras/geral.md`); a página por funcionalidade é coberta pelos testes do renderizador (#288) e pelo roteiro de uso no OpenCBS ([`projeto-de-referencia.md`](../projeto-de-referencia.md#roteiro-publicação-legível-273)).

## Relatório gerado

- Data: 2026-11-21-022130
- Modelo: `sonnet`
- Claude Code: 2.1.330 (Claude Code)
- Commit avaliado: bb381ef
- Orçamento por execução: US$ 1.50; repetições: 3; itens: PUB-FAT-001
- Edição liberada para o agente (Edit, Write, Bash): não
- Custo total: US$ 1.40
- Falhas graves: nenhuma

### Cobertura por fluxo

| Fluxo | Item | Execuções | Regras (média; mín–máx) | Evidências válidas | `confirmado` sem evidência válida | Perguntas esperadas | Premissa corrigida | Abrangência | Arquivos esperados citados | Arquivos alterados | Concluídas | Custo (US$) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| publicacao | PUB-FAT-001 | 3 | 100% de 2 (100%–100%) | 32 de 32 | 0 | n/a | n/a | n/a | n/a | 0 | 3 de 3 | 1.40 |

### Execuções

#### publicacao PUB-FAT-001 (repetição 1)

- Claude: custo US$ 0.47, 46 turnos, 90 s, resultado `success`
- Tokens: 662448 de entrada (592436 do cache, 69976 gravados no cache), 13490 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Ciclo de publicação: custo US$ 0.47 (primeira análise US$ 0.25, segunda US$ 0.23), teto US$ 3.00
  - Analisar P-FAT-001: análise A-0001; aprovada: sim; 5 regra(s) na base
  - Publicar: 4 documento(s) novos ou alterados; PR aberto (https://github.com/exemplo/faturamento-docs/pull/1)
  - Sincronizar (base nova): 9 documento(s) em `externos/`, commit 4fbbef0
  - Analisar P-FAT-003 de novo: `conhecimento` chamado 2 vez(es), 2 com `externos`; 2 resposta(s) com `[doc. publicada]`; 0 leitura(s) em `externos/`; 6 bloco(s) recortado(s) recebido(s), 5 regra(s) (`regras/geral.md#rn-ger-001`, `regras/geral.md#rn-ger-002`, `regras/geral.md#rn-ger-003`, `regras/geral.md#rn-ger-004`, `regras/geral.md#rn-ger-005`); fonte citada na resposta: sim; regra recortada da documentação publicada usada: sim
- Análise A-0001 (concluida): 6 afirmações (4 `confirmado`, 2 `inferido`, 0 `a_confirmar`), 1 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 10 de 10
- Regras: 2 de 2 (100%)
  - RN-FAT-002 Juros de mora desde o primeiro dia de atraso: encontrada (afirmações 2, 3, 4, 6)
  - RN-FAT-003 Carência de 3 dias sem encargos: encontrada (afirmações 1, 3, 4, 6)
- Outras regras do gabarito encontradas: INC-FAT-002, RN-FAT-001, RN-FAT-004
- Afirmações fora do gabarito: nenhuma
- Concluída: sim

#### publicacao PUB-FAT-001 (repetição 2)

- Claude: custo US$ 0.48, 43 turnos, 90 s, resultado `success`
- Tokens: 708433 de entrada (637150 do cache, 71245 gravados no cache), 13118 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Ciclo de publicação: custo US$ 0.48 (primeira análise US$ 0.26, segunda US$ 0.22), teto US$ 3.00
  - Analisar P-FAT-001: análise A-0001; aprovada: sim; 6 regra(s) na base
  - Publicar: 4 documento(s) novos ou alterados; PR aberto (https://github.com/exemplo/faturamento-docs/pull/1)
  - Sincronizar (base nova): 10 documento(s) em `externos/`, commit df4e4bd
  - Analisar P-FAT-003 de novo: `conhecimento` chamado 2 vez(es), 2 com `externos`; 2 resposta(s) com `[doc. publicada]`; 0 leitura(s) em `externos/`; 6 bloco(s) recortado(s) recebido(s), 5 regra(s) (`regras/geral.md#rn-ger-002`, `regras/geral.md#rn-ger-003`, `regras/geral.md#rn-ger-004`, `regras/geral.md#rn-ger-005`, `regras/geral.md#rn-ger-006`); fonte citada na resposta: sim; regra recortada da documentação publicada usada: sim
- Análise A-0001 (concluida): 6 afirmações (5 `confirmado`, 1 `inferido`, 0 `a_confirmar`), 1 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 10 de 10
- Regras: 2 de 2 (100%)
  - RN-FAT-002 Juros de mora desde o primeiro dia de atraso: encontrada (afirmações 2, 6)
  - RN-FAT-003 Carência de 3 dias sem encargos: encontrada (afirmações 1, 6)
- Outras regras do gabarito encontradas: INC-FAT-002, RN-FAT-001, RN-FAT-004, RN-FAT-005, RN-FAT-006
- Afirmações fora do gabarito: nenhuma
- Concluída: sim

#### publicacao PUB-FAT-001 (repetição 3)

- Claude: custo US$ 0.44, 42 turnos, 81 s, resultado `success`
- Tokens: 619647 de entrada (553369 do cache, 66244 gravados no cache), 12151 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Ciclo de publicação: custo US$ 0.44 (primeira análise US$ 0.24, segunda US$ 0.21), teto US$ 3.00
  - Analisar P-FAT-001: análise A-0001; aprovada: sim; 5 regra(s) na base
  - Publicar: 4 documento(s) novos ou alterados; PR aberto (https://github.com/exemplo/faturamento-docs/pull/1)
  - Sincronizar (base nova): 9 documento(s) em `externos/`, commit b37c92e
  - Analisar P-FAT-003 de novo: `conhecimento` chamado 2 vez(es), 2 com `externos`; 2 resposta(s) com `[doc. publicada]`; 0 leitura(s) em `externos/`; 6 bloco(s) recortado(s) recebido(s), 5 regra(s) (`regras/geral.md#rn-ger-001`, `regras/geral.md#rn-ger-002`, `regras/geral.md#rn-ger-003`, `regras/geral.md#rn-ger-004`, `regras/geral.md#rn-ger-005`); fonte citada na resposta: sim; regra recortada da documentação publicada usada: sim
- Análise A-0001 (concluida): 7 afirmações (6 `confirmado`, 1 `inferido`, 0 `a_confirmar`), 2 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 12 de 12
- Regras: 2 de 2 (100%)
  - RN-FAT-002 Juros de mora desde o primeiro dia de atraso: encontrada (afirmações 2, 7)
  - RN-FAT-003 Carência de 3 dias sem encargos: encontrada (afirmações 1, 6, 7)
- Outras regras do gabarito encontradas: INC-FAT-002, RN-FAT-001, RN-FAT-004, RN-FAT-006
- Afirmações fora do gabarito: nenhuma
- Concluída: sim

### Critérios

- **Regra encontrada:** uma evidência válida de uma afirmação no mesmo arquivo de uma evidência esperada da regra, com intervalos de linhas que se sobrepõem. Inconsistência (`INC-*`): uma mesma afirmação toca duas evidências esperadas dela. Evidências com mais de 30 linhas não contam.
- **Evidência válida:** passa de novo pela validação do `afirmar` contra a fixture e o hash do intervalo é o gravado.
- **Pergunta esperada:** uma pergunta registrada tem ao menos um termo de cada grupo de palavras-chave do gabarito (sem acentos nem maiúsculas; palavra inteira ou prefixo com `*`).
- **Premissa corrigida:** afirmação da seção "Demanda como entendida", com evidência válida, com o veredito esperado ("Verificação: não confere" ou "confere em parte") e as palavras-chave da premissa.
- **Falha grave:** afirmação `confirmado` sem evidência válida, nenhuma afirmação, estado ausente ou arquivo do projeto alterado.
- **Dados pessoais e requisitos não funcionais** (impacto): campo pessoal esperado apontado por uma afirmação da seção com as palavras-chave do campo e evidência válida que toca a esperada (definição ou conteúdo da coluna); aspecto não funcional (auditoria, desempenho, acesso) tratado por uma afirmação da seção ou por uma pergunta com as palavras-chave; possível falso positivo quando uma afirmação da seção cita uma coluna não pessoal do gabarito sem apontar nenhum campo esperado (para revisão, não é falha grave).
- **Revisão de requisito** (`revisar`): achado esperado atendido por um achado da característica esperada, com as palavras-chave e, se o esperado cita regras, com evidência válida que toca a regra; termo vago apontado por um achado de ambiguidade ou verificabilidade cujo trecho o contém; veredito igual ao esperado. Falha grave: achado de correção sem evidência válida, ou nenhum achado quando o esperado não é `pronta`.
- **Ciclo de vida** (`ciclo-vida`, análise de explicar com diagrama de estados): estado esperado encontrado quando um nó tem o valor do código como chave; inicial e finais conferem pela marca de início e fim; transição esperada sustentada quando uma aresta liga os mesmos estados e cita evidência que toca a esperada; achado esperado (inalcançável, sem saída) apontado pelo app no diagrama. Falha grave: nenhum diagrama de estados.
- **Cenários de teste** (`cenarios`): limite esperado lido por um cenário da regra cujo limite tem o mesmo valor, não vem de configuração e cita evidência que toca a esperada (limite de outro valor é listado como divergente); cada valor testado esperado (o limite e o vizinho) presente num cenário da regra; tabela de decisão esperada atendida por uma tabela com as condições mínimas, ligada à regra e com as palavras-chave nas condições; cenário negativo esperado atendido por um cenário da regra com as palavras-chave; tema a confirmar marcado num cenário com pergunta. Falha grave: nenhum cenário, ou cenário sem origem.
- **Fluxo do processo e modelo de dados** (`explicar`): raia esperada atendida pela raia de algum passo com as palavras-chave; passo do fluxo principal atendido por um nó com as palavras-chave no rótulo ou na descrição, ou com evidência que toca a esperada; decisão esperada, do mesmo jeito, por um nó de decisão; fluxograma coerente com início, fim, decisões com dois caminhos e todos os passos alcançáveis. Entidade esperada achada pela tabela, com o nome de negócio e a ligação ao glossário; relação esperada achada pelas tabelas das pontas, com a origem (chave estrangeira ou join). Falha grave: relação registrada com a origem errada (o documento não distingue a relação inferida).
- **Mapa de funcionalidades** (`mapear`, sem análise: o agente começa por `iniciar_mapeamento` e o mapa do projeto é comparado): funcionalidade esperada encontrada quando uma funcionalidade registrada (não descartada nem juntada) tem as palavras-chave no nome ou na descrição e ao menos um ponto de entrada esperado dela; área confere pelo slug. Ponto de entrada esperado bem tratado quando está numa funcionalidade que atende uma das esperadas para ele (cada parte de uma funcionalidade dividida conta), ou justificado quando o gabarito o aceita como utilitário. Falha grave: nenhuma funcionalidade, ponto de entrada do gabarito sem tratamento ou evidência de ponto de entrada que não confere.
- **Ciclo de publicação** (`publicacao`): o item `analise` é analisado, aprovado (as afirmações `confirmado` com evidências válidas confirmadas) e publicado num repositório de docs local, com PR numa forja falsa dado como mesclado; numa base nova, a documentação é sincronizada e o item `reanalise` é analisado de novo e comparado com o gabarito como os outros. Regra recortada usada quando `conhecimento` é chamado com `externos` (`incluir` ou `somente`) e devolve uma regra recortada de página publicada, com a âncora do trecho (ou o agente lê a cópia dela em `externos/`); fonte citada quando a resposta final traz o repositório ou o commit. Falha grave: análise não aprovada, nada publicado, nada sincronizado, documentação publicada não consultada ou consultada sem nenhuma regra recortada.
- **Histórias e critérios** (`historias`): história esperada atendida por uma história com as palavras-chave no título e na narrativa; regra esperada citada por um critério, pelo id ou por uma afirmação cuja evidência toca a regra; critério com valor concreto (um número); tema a confirmar marcado num critério com pergunta; quebra proposta quando a abrangência pede. Falha grave: nenhuma história, ou critério sem origem.

Detalhes em `docs/avaliacao.md`.
