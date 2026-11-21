# Avaliação dos roteiros da #291: 2026-11-21 (sonnet)

Conferência dos fluxos perguntar, explicar, impacto e revisar depois do ajuste da #291 nos roteiros (consultar a documentação publicada um conceito por chamada; [ciclo de publicação](2026-11-21-publicacao-legivel-sonnet.md)). Uma execução por fluxo, como na conferência da #279 ([relatório](2026-11-20-roteiros-278-sonnet.md)). Comando:

```sh
cargo run -p cba-avaliacao -- rodar --modelo sonnet --orcamento 1.5 --fluxos perguntar,explicar,impacto,revisar
```

Resumo: nenhuma falha grave, US$ 1,28. Regras: perguntar 7/7, explicar 6/7, impacto 3/3; revisar com 3/3 achados, 4/4 termos vagos e o veredito. Nesses fluxos não há documentação publicada (o contexto não traz `[doc. publicada]`), então o trecho alterado do roteiro não se aplica e nenhum agente chamou `conhecimento` com `externos`. A regra que faltou em explicar é a inconsistência `INC-FAT-003` (na #279, encontrada): o agente fez a pergunta esperada sobre ela ("A tela confere a parcela mínima sem os encargos; qual valor deve valer?"), mas nenhuma afirmação tocou as duas evidências, que é o que a comparação exige. Perguntas esperadas em impacto: 1 de 2, a mesma variação registrada na #279. Com uma execução por fluxo, não é possível separar a variação do modelo de um efeito do texto, que nesses fluxos não se aplica.

## Relatório gerado

- Data: 2026-11-21-022612
- Modelo: `sonnet`
- Claude Code: 2.1.330 (Claude Code)
- Commit avaliado: bb381ef
- Orçamento por execução: US$ 1.50; repetições: 1; itens: P-FAT-001, F-FAT-002, D-FAT-001, REV-FAT-001
- Edição liberada para o agente (Edit, Write, Bash): não
- Custo total: US$ 1.28
- Falhas graves: nenhuma

### Cobertura por fluxo

| Fluxo | Item | Execuções | Regras (média; mín–máx) | Evidências válidas | `confirmado` sem evidência válida | Perguntas esperadas | Premissa corrigida | Abrangência | Arquivos esperados citados | Arquivos alterados | Concluídas | Custo (US$) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| perguntar | P-FAT-001 | 1 | 100% de 7 (100%–100%) | 16 de 16 | 0 | 100% | n/a | n/a | n/a | 0 | 1 de 1 | 0.26 |
| explicar | F-FAT-002 | 1 | 86% de 7 (86%–86%) | 27 de 27 | 0 | 100% | n/a | n/a | 60% | 0 | 1 de 1 | 0.42 |
| impacto | D-FAT-001 | 1 | 100% de 3 (100%–100%) | 21 de 21 | 0 | 50% | 1 de 1 | 1 de 1 | 100% | 0 | 1 de 1 | 0.33 |
| revisar | REV-FAT-001 | 1 | 100% de 0 (100%–100%) | 2 de 2 | 0 | 100% | n/a | n/a | n/a | 0 | 1 de 1 | 0.28 |

### Execuções

#### perguntar P-FAT-001 (repetição 1)

- Claude: custo US$ 0.26, 23 turnos, 50 s, resultado `success`
- Tokens: 328443 de entrada (290679 do cache, 37746 gravados no cache), 7493 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 8 afirmações (7 `confirmado`, 1 `inferido`, 0 `a_confirmar`), 2 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 16 de 16
- Regras: 7 de 7 (100%)
  - RN-FAT-001 Multa por atraso vem de parâmetro: encontrada (afirmações 1, 2, 8)
  - RN-FAT-002 Juros de mora desde o primeiro dia de atraso: encontrada (afirmações 3)
  - RN-FAT-003 Carência de 3 dias sem encargos: encontrada (afirmações 3, 8)
  - RN-FAT-004 Cliente com isenção cadastrada não paga encargos: encontrada (afirmações 4, 7)
  - RN-FAT-005 Órgão público não paga multa: encontrada (afirmações 1, 5, 8)
  - RN-FAT-006 Teto de encargos por tipo de cliente: encontrada (afirmações 6, 8)
  - INC-FAT-002 Lote COBOL usa valores fixos e não aplica exceções do backend: encontrada (afirmações 7)
- Afirmações fora do gabarito: nenhuma
- Perguntas esperadas:
  - O lote de cobrança deveria seguir o cálculo do projeto (parâmetros, exceção do órgão público, teto)?: pergunta 2
- Concluída: sim

#### explicar F-FAT-002 (repetição 1)

- Claude: custo US$ 0.42, 39 turnos, 90 s, resultado `success`
- Tokens: 572539 de entrada (517074 do cache, 55439 gravados no cache), 14460 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 16 afirmações (16 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 3 perguntas, 0 sugestões, 1 termos
- Evidências válidas: 27 de 27
- Regras: 6 de 7 (86%)
  - RN-FAT-007 Parcelamento só para pessoa jurídica e órgão público: encontrada (afirmações 4, 9)
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: encontrada (afirmações 4, 11)
  - RN-FAT-009 Valor mínimo da parcela, exceto órgão público: encontrada (afirmações 12, 15, 16)
  - RN-FAT-010 Só fatura aberta pode ser parcelada: encontrada (afirmações 4, 10)
  - RN-FAT-011 Composição e vencimentos das parcelas: encontrada (afirmações 5, 6, 7)
  - INC-FAT-001 Tela oferece até 12 parcelas; backend aceita até 6: encontrada (afirmações 11)
  - INC-FAT-003 Tela calcula a parcela mínima sobre o valor original: não encontrada
- Outras regras do gabarito encontradas: RN-FAT-005, RN-FAT-006
- Afirmações fora do gabarito: 1, 2, 3, 8, 14
- Arquivos esperados citados: 3 de 5 (faltam: backend/src/main/java/exemplo/faturamento/ParametroRepository.java, db/01_tabelas.sql)
- Perguntas esperadas:
  - A tela oferece até 12 parcelas e o backend aceita até 6; qual é o limite certo?: pergunta 1
  - A tela confere a parcela mínima sem os encargos; qual valor deve valer?: pergunta 3
- Concluída: sim

#### impacto D-FAT-001 (repetição 1)

- Claude: custo US$ 0.33, 32 turnos, 67 s, resultado `success`
- Tokens: 449387 de entrada (401578 do cache, 47787 gravados no cache), 10012 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 11 afirmações (6 `confirmado`, 5 `inferido`, 0 `a_confirmar`), 1 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 21 de 21
- Regras: 3 de 3 (100%)
  - RN-FAT-007 Parcelamento só para pessoa jurídica e órgão público: encontrada (afirmações 1, 2)
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: encontrada (afirmações 3, 6, 7, 8, 10, 11)
  - INC-FAT-001 Tela oferece até 12 parcelas; backend aceita até 6: encontrada (afirmações 6, 7, 11)
- Outras regras do gabarito encontradas: RN-FAT-009, RN-FAT-010, RN-FAT-011
- Afirmações fora do gabarito: 9
- Arquivos esperados citados: 3 de 3
- Perguntas esperadas:
  - O limite de 3 parcelas vale também para órgão público, que usa o mesmo parâmetro?: pergunta 1
  - A tela oferece até 12 parcelas; deve ser alinhada ao novo limite?: não feita
- Premissa corrigida: sim (afirmação 1)
- Abrangência: esperada pequena, registrada pequena (confere)
- Concluída: sim

#### revisar REV-FAT-001 (repetição 1)

- Claude: custo US$ 0.28, 28 turnos, 58 s, resultado `success`
- Tokens: 463814 de entrada (424162 do cache, 39628 gravados no cache), 7917 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 2 afirmações (2 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 3 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 2 de 2
- Regras: 0 de 0 (100%)
- Outras regras do gabarito encontradas: RN-FAT-012, RN-FAT-013
- Afirmações fora do gabarito: nenhuma
- Perguntas esperadas:
  - A história muda a regra que proíbe cancelar fatura com pagamento parcial, ou o texto está errado?: pergunta 1
- Veredito: esperado precisa de retrabalho, registrado precisa de retrabalho (confere)
- Achados: 7 registrado(s); evidências válidas 4 de 4
- Achados esperados: 3 de 3
  - REV-FAT-001-C1: achado 1
  - REV-FAT-001-L1: achado 3
  - REV-FAT-001-L2: achado 4
- Termos vagos apontados: 4 de 4
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
