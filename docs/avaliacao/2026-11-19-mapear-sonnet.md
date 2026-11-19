# Avaliação dos fluxos contra o gabarito

- Data: 2026-11-19-174614
- Modelo: `sonnet`
- Claude Code: 2.1.328 (Claude Code)
- Commit avaliado: 8392685
- Orçamento por execução: US$ 1.50; repetições: 3; itens: 
- Edição liberada para o agente (Edit, Write, Bash): não
- Custo total: US$ 0.82
- Falhas graves: nenhuma

## Cobertura por fluxo

| Fluxo | Item | Execuções | Regras (média; mín–máx) | Evidências válidas | `confirmado` sem evidência válida | Perguntas esperadas | Premissa corrigida | Abrangência | Arquivos esperados citados | Arquivos alterados | Concluídas | Custo (US$) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| mapear | MAP-FAT-001 | 3 | 0% de 0 (0%–0%) | 0 de 0 | 0 | n/a | n/a | n/a | n/a | 0 | 0 de 3 | 0.82 |

## Execuções

### mapear MAP-FAT-001 (repetição 1)

- Claude: custo US$ 0.28, 21 turnos, 50 s, resultado `success`
- Tokens: 329545 de entrada (287365 do cache, 42162 gravados no cache), 7984 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Mapa de funcionalidades: 7 registradas; funcionalidades esperadas 100%, pontos de entrada bem tratados 100%
  - F-FAT-003 (F-FAT-003): encontrada como F-COB-003, em outra área
  - F-FAT-005 (Registro de pagamento): encontrada como F-PAG-001, em outra área

### mapear MAP-FAT-001 (repetição 2)

- Claude: custo US$ 0.27, 21 turnos, 51 s, resultado `success`
- Tokens: 327626 de entrada (285954 do cache, 41654 gravados no cache), 7924 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Mapa de funcionalidades: 7 registradas; funcionalidades esperadas 100%, pontos de entrada bem tratados 100%
  - F-FAT-003 (F-FAT-003): encontrada como F-COB-001, em outra área
  - F-FAT-005 (Registro de pagamento): encontrada como F-PAG-001, em outra área

### mapear MAP-FAT-001 (repetição 3)

- Claude: custo US$ 0.27, 21 turnos, 49 s, resultado `success`
- Tokens: 320387 de entrada (280355 do cache, 40014 gravados no cache), 7690 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Mapa de funcionalidades: 7 registradas; funcionalidades esperadas 100%, pontos de entrada bem tratados 100%
  - F-FAT-003 (F-FAT-003): encontrada como F-COB-003, em outra área
  - F-FAT-005 (Registro de pagamento): encontrada como F-PAG-001, em outra área

## Critérios

- **Regra encontrada:** uma evidência válida de uma afirmação no mesmo arquivo de uma evidência esperada da regra, com intervalos de linhas que se sobrepõem. Inconsistência (`INC-*`): uma mesma afirmação toca duas evidências esperadas dela. Evidências com mais de 30 linhas não contam.
- **Evidência válida:** passa de novo pela validação do `afirmar` contra a fixture e o hash do intervalo é o gravado.
- **Pergunta esperada:** uma pergunta registrada tem ao menos um termo de cada grupo de palavras-chave do gabarito (sem acentos nem maiúsculas; palavra inteira ou prefixo com `*`).
- **Premissa corrigida:** afirmação da seção "Demanda como entendida", com evidência válida, com o veredito esperado ("Verificação: não confere" ou "confere em parte") e as palavras-chave da premissa.
- **Falha grave:** afirmação `confirmado` sem evidência válida, nenhuma afirmação, estado ausente ou arquivo do projeto alterado.
- **Revisão de requisito** (`revisar`): achado esperado atendido por um achado da característica esperada, com as palavras-chave e, se o esperado cita regras, com evidência válida que toca a regra; termo vago apontado por um achado de ambiguidade ou verificabilidade cujo trecho o contém; veredito igual ao esperado. Falha grave: achado de correção sem evidência válida, ou nenhum achado quando o esperado não é `pronta`.
- **Ciclo de vida** (`ciclo-vida`, análise de explicar com diagrama de estados): estado esperado encontrado quando um nó tem o valor do código como chave; inicial e finais conferem pela marca de início e fim; transição esperada sustentada quando uma aresta liga os mesmos estados e cita evidência que toca a esperada; achado esperado (inalcançável, sem saída) apontado pelo app no diagrama. Falha grave: nenhum diagrama de estados.
- **Cenários de teste** (`cenarios`): limite esperado lido por um cenário da regra cujo limite tem o mesmo valor, não vem de configuração e cita evidência que toca a esperada (limite de outro valor é listado como divergente); cada valor testado esperado (o limite e o vizinho) presente num cenário da regra; tabela de decisão esperada atendida por uma tabela com as condições mínimas, ligada à regra e com as palavras-chave nas condições; cenário negativo esperado atendido por um cenário da regra com as palavras-chave; tema a confirmar marcado num cenário com pergunta. Falha grave: nenhum cenário, ou cenário sem origem.
- **Fluxo do processo e modelo de dados** (`explicar`): raia esperada atendida pela raia de algum passo com as palavras-chave; passo do fluxo principal atendido por um nó com as palavras-chave no rótulo ou na descrição, ou com evidência que toca a esperada; decisão esperada, do mesmo jeito, por um nó de decisão; fluxograma coerente com início, fim, decisões com dois caminhos e todos os passos alcançáveis. Entidade esperada achada pela tabela, com o nome de negócio e a ligação ao glossário; relação esperada achada pelas tabelas das pontas, com a origem (chave estrangeira ou join). Falha grave: relação registrada com a origem errada (o documento não distingue a relação inferida).
- **Mapa de funcionalidades** (`mapear`, sem análise: o agente começa por `iniciar_mapeamento` e o mapa do projeto é comparado): funcionalidade esperada encontrada quando uma funcionalidade registrada (não descartada nem juntada) tem as palavras-chave no nome ou na descrição e ao menos um ponto de entrada esperado dela; área confere pelo slug. Ponto de entrada esperado bem tratado quando está numa funcionalidade que atende uma das esperadas para ele (cada parte de uma funcionalidade dividida conta), ou justificado quando o gabarito o aceita como utilitário. Falha grave: nenhuma funcionalidade, ponto de entrada do gabarito sem tratamento ou evidência de ponto de entrada que não confere.
- **Histórias e critérios** (`historias`): história esperada atendida por uma história com as palavras-chave no título e na narrativa; regra esperada citada por um critério, pelo id ou por uma afirmação cuja evidência toca a regra; critério com valor concreto (um número); tema a confirmar marcado num critério com pergunta; quebra proposta quando a abrangência pede. Falha grave: nenhuma história, ou critério sem origem.

Detalhes em `docs/avaliacao.md`.
