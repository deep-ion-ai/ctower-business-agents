# Avaliação dos fluxos contra o gabarito

- Data: 2026-11-25-163526
- Modelo: `sonnet`
- Claude Code: 2.1.332 (Claude Code)
- Commit avaliado: c4cc478
- Orçamento por execução: US$ 2.00; repetições: 1; itens: REV-FAT-001
- Edição liberada para o agente (Edit, Write, Bash): não
- Custo total: US$ 1.47
- Falhas graves: nenhuma

## Cobertura por fluxo

| Fluxo | Item | Execuções | Regras (média; mín–máx) | Evidências válidas | `confirmado` sem evidência válida | Perguntas esperadas | Premissa corrigida | Abrangência | Arquivos esperados citados | Arquivos alterados | Concluídas | Custo (US$) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| revisar | REV-FAT-001 [simples] | 1 | 100% de 0 (100%–100%) | 11 de 11 | 0 | 100% | n/a | n/a | n/a | 0 | 1 de 1 | 0.36 |
| revisar | REV-FAT-001 [workflow] | 1 | 100% de 0 (100%–100%) | 2 de 2 | 0 | 100% | n/a | n/a | n/a | 0 | 1 de 1 | 1.11 |

## Condução simples e workflow

| Item | Condução (pedida → efetiva) | Achados esperados | Termos vagos apontados | Veredito (esperado) | Perguntas esperadas | Achados | Contestados | Agentes | Tokens (todos os modelos) | Custo (US$) | Duração (relógio) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| REV-FAT-001 [simples] | simples → simples | 3 de 3 | 4 de 4 | precisa de retrabalho (precisa de retrabalho) | 1 de 1 | 7 | 0 | - | 525 mil | 0.36 | 1 min 20 s |
| REV-FAT-001 [workflow] | workflow → workflow | 3 de 3 | 4 de 4 | precisa de retrabalho (precisa de retrabalho) | 1 de 1 | 13 | 0 | 12 | 1.5 mi | 1.11 | 2 min 23 s |


### Tokens por fase: REV-FAT-001 [workflow] (situação do workflow: concluido)

| Fase | Agentes | Tokens | Por agente (mín–máx) | Saída |
|---|---|---|---|---|
| Separar | 1 | 121 mil | 121 mil–121 mil | 3 mil |
| Avaliar | 5 | 1.0 mi | 134 mil–258 mil | 24 mil |
| Verificar | 5 | 146 mil | 24 mil–40 mil | 2 mil |
| Consolidar | 1 | 81 mil | 81 mil–81 mil | 2 mil |

## Execuções

### revisar REV-FAT-001 [simples] (repetição 1)

- Claude: custo US$ 0.36, 34 turnos, 80 s, resultado `success`
- Tokens: 513348 de entrada (463103 do cache, 50221 gravados no cache), 11537 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 5 afirmações (5 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 5 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 11 de 11
- Regras: 0 de 0 (100%)
- Outras regras do gabarito encontradas: RN-FAT-012, RN-FAT-013, RN-FAT-014
- Afirmações fora do gabarito: nenhuma
- Perguntas esperadas:
  - A história muda a regra que proíbe cancelar fatura com pagamento parcial, ou o texto está errado?: pergunta 1
- Veredito: esperado precisa de retrabalho, registrado precisa de retrabalho (confere)
- Achados: 7 registrado(s); evidências válidas 7 de 7
- Achados esperados: 3 de 3
  - REV-FAT-001-C1: achado 1
  - REV-FAT-001-L1: achado 3
  - REV-FAT-001-L2: achado 4
- Termos vagos apontados: 4 de 4
- Concluída: sim

### revisar REV-FAT-001 [workflow] (repetição 1)

- Claude: custo US$ 1.11, 3 turnos, 143 s, resultado `success`
- Tokens: 82432 de entrada (78995 do cache, 3431 gravados no cache), 1068 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 1 afirmações (1 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 5 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 2 de 2
- Regras: 0 de 0 (100%)
- Outras regras do gabarito encontradas: RN-FAT-013
- Afirmações fora do gabarito: nenhuma
- Perguntas esperadas:
  - A história muda a regra que proíbe cancelar fatura com pagamento parcial, ou o texto está errado?: pergunta 2
- Veredito: esperado precisa de retrabalho, registrado precisa de retrabalho (confere)
- Achados: 13 registrado(s); evidências válidas 23 de 23
- Achados esperados: 3 de 3
  - REV-FAT-001-C1: achado 4
  - REV-FAT-001-L1: achado 3
  - REV-FAT-001-L2: achado 1
- Termos vagos apontados: 4 de 4
- Concluída: sim

## Critérios

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
