# Avaliação do ciclo de publicação: 2026-11-20 (sonnet)

Avaliação do cenário `publicacao` (#279, [`docs/avaliacao.md`](../avaliacao.md#ciclo-de-publicação)): analisar P-FAT-001, aprovar, publicar num repositório de docs local (PR numa forja falsa, dado como mesclado), sincronizar numa base nova e analisar P-FAT-003 nela, conferindo que o agente consultou a documentação publicada. Comando:

```sh
cargo run -p cba-avaliacao -- rodar --modelo sonnet --orcamento 1.5 --fluxos publicacao --repeticoes 3
```

Abaixo, o `relatorio.md` da rodada final, sem edição. O commit que aparece nele é o do cenário com o ajuste dos roteiros e da leitura da transcrição.

## Rodadas antes do relatório

1. **Primeira rodada (1 execução, US$ 0,44), roteiros da #278.** O ciclo inteiro funcionou (análise aprovada com 4 regras, 5 documentos publicados com PR, 6 sincronizados na base nova), mas, na segunda análise, o agente não chamou `conhecimento`: viu no contexto de `iniciar_analise` só os títulos da documentação publicada e respondeu pelo código, dizendo ao analista que não a leu. Ajuste: os roteiros de perguntar, explicar, impacto e revisar e o texto do contexto passam a pedir a consulta (`conhecimento` com `externos: "incluir"`) antes de explorar o código, porque o título não diz o que o documento afirma.
2. **Segunda rodada (3 execuções, US$ 1,27), roteiros ajustados.** Nas três, o agente chamou `conhecimento` com `externos: "incluir"` e recebeu os documentos publicados, mas a comparação deu "não usada": o Claude Code entrega ao agente o `structuredContent` (os dados, com `"fonte": "externa"`) no lugar do texto com o rótulo `[doc. publicada]`, e a leitura da transcrição só procurava o rótulo. Ajuste na comparação: os dados com `fonte: externa` também contam. Relendo as transcrições guardadas, as três execuções usaram a documentação publicada.
3. **Rodada final (3 execuções, US$ 1,37), abaixo.**

Resumo da rodada final: nenhuma falha grave; nas três execuções, a análise aprovada (5 ou 6 regras na base), 6 ou 7 documentos publicados com PR, 7 ou 8 sincronizados na base nova, `conhecimento` com `externos` (1 ou 2 vezes) devolvendo documentos publicados, a fonte citada na resposta (repositório e commit) e as regras do P-FAT-003 encontradas (2 de 2, com 27 evidências válidas no total). Custo do ciclo: US$ 0,42 a 0,47 (teto de US$ 3,00, duas execuções de US$ 1,50).

Achado lateral, fora do escopo: como o agente recebe os dados estruturados e não o texto, o lembrete "pista, não prova" do texto de `conhecimento` não chega a ele por esse caminho; chega pelos roteiros, pela descrição da ferramenta e pelo contexto de `iniciar_analise`. Nas transcrições, o agente tratou a documentação como pista e citou só código como evidência.

## Relatório gerado

- Data: 2026-11-20-135939
- Modelo: `sonnet`
- Claude Code: 2.1.329 (Claude Code)
- Commit avaliado: c32f504
- Orçamento por execução: US$ 1.50; repetições: 3; itens: PUB-FAT-001
- Edição liberada para o agente (Edit, Write, Bash): não
- Custo total: US$ 1.37
- Falhas graves: nenhuma

### Cobertura por fluxo

| Fluxo | Item | Execuções | Regras (média; mín–máx) | Evidências válidas | `confirmado` sem evidência válida | Perguntas esperadas | Premissa corrigida | Abrangência | Arquivos esperados citados | Arquivos alterados | Concluídas | Custo (US$) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| publicacao | PUB-FAT-001 | 3 | 100% de 2 (100%–100%) | 36 de 36 | 0 | n/a | n/a | n/a | n/a | 0 | 3 de 3 | 1.37 |

### Execuções

#### publicacao PUB-FAT-001 (repetição 1)

- Claude: custo US$ 0.42, 41 turnos, 87 s, resultado `success`
- Tokens: 627830 de entrada (565590 do cache, 62204 gravados no cache), 11843 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Ciclo de publicação: custo US$ 0.42 (primeira análise US$ 0.25, segunda US$ 0.18), teto US$ 3.00
  - Analisar P-FAT-001: análise A-0001; aprovada: sim; 5 regra(s) na base
  - Publicar: 6 documento(s) novos ou alterados; PR aberto (https://github.com/exemplo/faturamento-docs/pull/1)
  - Sincronizar (base nova): 7 documento(s) em `externos/`, commit c633f2e
  - Analisar P-FAT-003 de novo: `conhecimento` chamado 2 vez(es), 2 com `externos`; 1 resposta(s) com `[doc. publicada]`; 0 leitura(s) em `externos/`; fonte citada na resposta: sim; documentação publicada usada: sim
- Análise A-0001 (concluida): 5 afirmações (4 `confirmado`, 1 `inferido`, 0 `a_confirmar`), 1 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 12 de 12
- Regras: 2 de 2 (100%)
  - RN-FAT-002 Juros de mora desde o primeiro dia de atraso: encontrada (afirmações 1, 2, 5)
  - RN-FAT-003 Carência de 3 dias sem encargos: encontrada (afirmações 1, 4, 5)
- Outras regras do gabarito encontradas: INC-FAT-002, RN-FAT-001, RN-FAT-004
- Afirmações fora do gabarito: nenhuma
- Concluída: sim

#### publicacao PUB-FAT-001 (repetição 2)

- Claude: custo US$ 0.47, 43 turnos, 93 s, resultado `success`
- Tokens: 651077 de entrada (581506 do cache, 69535 gravados no cache), 13505 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Ciclo de publicação: custo US$ 0.47 (primeira análise US$ 0.25, segunda US$ 0.22), teto US$ 3.00
  - Analisar P-FAT-001: análise A-0001; aprovada: sim; 6 regra(s) na base
  - Publicar: 7 documento(s) novos ou alterados; PR aberto (https://github.com/exemplo/faturamento-docs/pull/1)
  - Sincronizar (base nova): 8 documento(s) em `externos/`, commit bfd1443
  - Analisar P-FAT-003 de novo: `conhecimento` chamado 2 vez(es), 2 com `externos`; 2 resposta(s) com `[doc. publicada]`; 0 leitura(s) em `externos/`; fonte citada na resposta: sim; documentação publicada usada: sim
- Análise A-0001 (concluida): 7 afirmações (6 `confirmado`, 1 `inferido`, 0 `a_confirmar`), 1 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 13 de 13
- Regras: 2 de 2 (100%)
  - RN-FAT-002 Juros de mora desde o primeiro dia de atraso: encontrada (afirmações 2, 4, 7)
  - RN-FAT-003 Carência de 3 dias sem encargos: encontrada (afirmações 1, 3, 7)
- Outras regras do gabarito encontradas: INC-FAT-002, RN-FAT-001, RN-FAT-004
- Afirmações fora do gabarito: nenhuma
- Concluída: sim

#### publicacao PUB-FAT-001 (repetição 3)

- Claude: custo US$ 0.47, 47 turnos, 95 s, resultado `success`
- Tokens: 726255 de entrada (658557 do cache, 67658 gravados no cache), 13607 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Ciclo de publicação: custo US$ 0.47 (primeira análise US$ 0.26, segunda US$ 0.21), teto US$ 3.00
  - Analisar P-FAT-001: análise A-0001; aprovada: sim; 6 regra(s) na base
  - Publicar: 7 documento(s) novos ou alterados; PR aberto (https://github.com/exemplo/faturamento-docs/pull/1)
  - Sincronizar (base nova): 8 documento(s) em `externos/`, commit ac2d4c6
  - Analisar P-FAT-003 de novo: `conhecimento` chamado 1 vez(es), 1 com `externos`; 1 resposta(s) com `[doc. publicada]`; 0 leitura(s) em `externos/`; fonte citada na resposta: sim; documentação publicada usada: sim
- Análise A-0001 (concluida): 6 afirmações (5 `confirmado`, 1 `inferido`, 0 `a_confirmar`), 1 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 11 de 11
- Regras: 2 de 2 (100%)
  - RN-FAT-002 Juros de mora desde o primeiro dia de atraso: encontrada (afirmações 2, 6)
  - RN-FAT-003 Carência de 3 dias sem encargos: encontrada (afirmações 1, 6)
- Outras regras do gabarito encontradas: INC-FAT-002, RN-FAT-001, RN-FAT-004, RN-FAT-005, RN-FAT-006
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
- **Ciclo de publicação** (`publicacao`): o item `analise` é analisado, aprovado (as afirmações `confirmado` com evidências válidas confirmadas) e publicado num repositório de docs local, com PR numa forja falsa dado como mesclado; numa base nova, a documentação é sincronizada e o item `reanalise` é analisado de novo e comparado com o gabarito como os outros. Documentação publicada usada quando `conhecimento` é chamado com `externos` (`incluir` ou `somente`) e devolve algum `[doc. publicada]` (ou o agente lê um arquivo de `externos/`); fonte citada quando a resposta final traz o repositório ou o commit. Falha grave: análise não aprovada, nada publicado, nada sincronizado ou documentação publicada não usada.
- **Histórias e critérios** (`historias`): história esperada atendida por uma história com as palavras-chave no título e na narrativa; regra esperada citada por um critério, pelo id ou por uma afirmação cuja evidência toca a regra; critério com valor concreto (um número); tema a confirmar marcado num critério com pergunta; quebra proposta quando a abrangência pede. Falha grave: nenhuma história, ou critério sem origem.

Detalhes em `docs/avaliacao.md`.
