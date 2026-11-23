# Avaliação dos roteiros depois da #278 e da #279: 2026-11-20 (sonnet)

Os roteiros de perguntar, explicar, impacto, revisar e reunião mudaram na #278 (consultar a documentação publicada como pista, citar a fonte) e os quatro primeiros de novo na #279 (consultar antes de explorar o código). Sem destino da documentação, como nos itens do gabarito, o trecho novo não se aplica; esta rodada confere que nada piorou. Uma execução por fluxo:

```sh
cargo run -p cba-avaliacao -- rodar --modelo sonnet --orcamento 1.5 --fluxos perguntar,explicar,impacto,revisar,reuniao
```

Nenhuma falha grave e todas concluídas (US$ 1,53). Comparando com as referências anteriores ([2026-11-12](2026-11-12-sonnet.md), [#168](2026-11-16-revisar-historias-cenarios-sonnet.md), [reunião](2026-11-14-reuniao-sonnet.md)):

- **Regras:** perguntar 7 de 7, explicar 7 de 7 (antes 6 de 7), impacto 3 de 3, como antes; revisar com os 3 achados esperados, os 4 termos vagos e o veredito esperado.
- **Perguntas esperadas:** perguntar 1 de 1; explicar e impacto 1 de 2 cada (antes 2 de 2): a pergunta da parcela mínima sem encargos (explicar) e a do limite de 12 parcelas da tela (impacto) não foram feitas nesta execução.
- **Reunião:** 16 de 16 itens e 30 de 30 citações; papéis 2 de 4 (antes 3 de 4): João Pereira proposto como desenvolvedor.

As diferenças estão nas perguntas e nos papéis, em fluxos em que o texto novo não tem efeito (não há documentação publicada), com uma execução só: tratadas como variação do modelo, não como regressão. Para comparar versões, use três repetições.

## Relatório gerado

- Data: 2026-11-20-140445
- Modelo: `sonnet`
- Claude Code: 2.1.329 (Claude Code)
- Commit avaliado: c32f504
- Orçamento por execução: US$ 1.50; repetições: 1; itens: P-FAT-001, F-FAT-002, D-FAT-001, REV-FAT-001, REU-FAT-001
- Edição liberada para o agente (Edit, Write, Bash): não
- Custo total: US$ 1.53
- Falhas graves: nenhuma

### Cobertura por fluxo

| Fluxo | Item | Execuções | Regras (média; mín–máx) | Evidências válidas | `confirmado` sem evidência válida | Perguntas esperadas | Premissa corrigida | Abrangência | Arquivos esperados citados | Arquivos alterados | Concluídas | Custo (US$) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| perguntar | P-FAT-001 | 1 | 100% de 7 (100%–100%) | 14 de 14 | 0 | 100% | n/a | n/a | n/a | 0 | 1 de 1 | 0.27 |
| explicar | F-FAT-002 | 1 | 100% de 7 (100%–100%) | 22 de 22 | 0 | 50% | n/a | n/a | 60% | 0 | 1 de 1 | 0.47 |
| impacto | D-FAT-001 | 1 | 100% de 3 (100%–100%) | 18 de 18 | 0 | 50% | 1 de 1 | 1 de 1 | 100% | 0 | 1 de 1 | 0.28 |
| revisar | REV-FAT-001 | 1 | 100% de 0 (100%–100%) | 1 de 1 | 0 | 100% | n/a | n/a | n/a | 0 | 1 de 1 | 0.26 |

### Reuniões

| Fluxo | Reunião | Execuções | Papéis (média) | Itens da ata (média; mín–máx) | Citações conferidas de novo | Seção vazia com itens esperados | Concluídas | Custo (US$) | Tempo | Tokens |
|---|---|---|---|---|---|---|---|---|---|---|
| reuniao | REU-FAT-001 | 1 | 50% de 4 | 100% de 16 (100%–100%) | 30 de 30 | 0 | 1 de 1 | 0.25 | 53 s | 300710 de entrada (265896 do cache, 34796 gravados no cache), 8241 de saída |

Itens atendidos por seção (soma das execuções):

| Fluxo | Reunião | Seção | Atendidos |
|---|---|---|---|
| reuniao | REU-FAT-001 | decisoes | 3 de 3 |
| reuniao | REU-FAT-001 | necessidades | 3 de 3 |
| reuniao | REU-FAT-001 | regras | 2 de 2 |
| reuniao | REU-FAT-001 | pendencias | 3 de 3 |
| reuniao | REU-FAT-001 | duvidas | 3 de 3 |
| reuniao | REU-FAT-001 | proximos_passos | 2 de 2 |

### Execuções

#### perguntar P-FAT-001 (repetição 1)

- Claude: custo US$ 0.27, 25 turnos, 49 s, resultado `success`
- Tokens: 375319 de entrada (333319 do cache, 41980 gravados no cache), 6859 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 7 afirmações (6 `confirmado`, 1 `inferido`, 0 `a_confirmar`), 2 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 14 de 14
- Regras: 7 de 7 (100%)
  - RN-FAT-001 Multa por atraso vem de parâmetro: encontrada (afirmações 1, 7)
  - RN-FAT-002 Juros de mora desde o primeiro dia de atraso: encontrada (afirmações 2, 7)
  - RN-FAT-003 Carência de 3 dias sem encargos: encontrada (afirmações 2)
  - RN-FAT-004 Cliente com isenção cadastrada não paga encargos: encontrada (afirmações 3, 6)
  - RN-FAT-005 Órgão público não paga multa: encontrada (afirmações 4)
  - RN-FAT-006 Teto de encargos por tipo de cliente: encontrada (afirmações 5, 7)
  - INC-FAT-002 Lote COBOL usa valores fixos e não aplica exceções do backend: encontrada (afirmações 6)
- Afirmações fora do gabarito: nenhuma
- Perguntas esperadas:
  - O lote de cobrança deveria seguir o cálculo do projeto (parâmetros, exceção do órgão público, teto)?: pergunta 2
- Concluída: sim

#### explicar F-FAT-002 (repetição 1)

- Claude: custo US$ 0.47, 34 turnos, 94 s, resultado `success`
- Tokens: 867470 de entrada (807028 do cache, 60408 gravados no cache), 14695 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 9 afirmações (9 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 2 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 22 de 22
- Regras: 7 de 7 (100%)
  - RN-FAT-007 Parcelamento só para pessoa jurídica e órgão público: encontrada (afirmações 3)
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: encontrada (afirmações 4)
  - RN-FAT-009 Valor mínimo da parcela, exceto órgão público: encontrada (afirmações 5, 6)
  - RN-FAT-010 Só fatura aberta pode ser parcelada: encontrada (afirmações 2)
  - RN-FAT-011 Composição e vencimentos das parcelas: encontrada (afirmações 5, 8)
  - INC-FAT-001 Tela oferece até 12 parcelas; backend aceita até 6: encontrada (afirmações 4)
  - INC-FAT-003 Tela calcula a parcela mínima sobre o valor original: encontrada (afirmações 5)
- Outras regras do gabarito encontradas: RN-FAT-001, RN-FAT-002, RN-FAT-004, RN-FAT-005, RN-FAT-006
- Afirmações fora do gabarito: 1, 9
- Arquivos esperados citados: 3 de 5 (faltam: backend/src/main/java/exemplo/faturamento/ParametroRepository.java, db/01_tabelas.sql)
- Perguntas esperadas:
  - A tela oferece até 12 parcelas e o backend aceita até 6; qual é o limite certo?: pergunta 2
  - A tela confere a parcela mínima sem os encargos; qual valor deve valer?: não feita
- Concluída: sim

#### impacto D-FAT-001 (repetição 1)

- Claude: custo US$ 0.28, 26 turnos, 53 s, resultado `success`
- Tokens: 346551 de entrada (303727 do cache, 42806 gravados no cache), 7890 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 9 afirmações (4 `confirmado`, 5 `inferido`, 0 `a_confirmar`), 3 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 18 de 18
- Regras: 3 de 3 (100%)
  - RN-FAT-007 Parcelamento só para pessoa jurídica e órgão público: encontrada (afirmações 2)
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: encontrada (afirmações 3, 4, 6, 7, 8, 9)
  - INC-FAT-001 Tela oferece até 12 parcelas; backend aceita até 6: encontrada (afirmações 3, 6)
- Outras regras do gabarito encontradas: RN-FAT-009, RN-FAT-010, RN-FAT-011
- Afirmações fora do gabarito: nenhuma
- Arquivos esperados citados: 3 de 3
- Perguntas esperadas:
  - O limite de 3 parcelas vale também para órgão público, que usa o mesmo parâmetro?: pergunta 1
  - A tela oferece até 12 parcelas; deve ser alinhada ao novo limite?: não feita
- Premissa corrigida: sim (afirmação 1)
- Abrangência: esperada pequena, registrada pequena (confere)
- Concluída: sim

#### revisar REV-FAT-001 (repetição 1)

- Claude: custo US$ 0.26, 27 turnos, 50 s, resultado `success`
- Tokens: 370207 de entrada (330590 do cache, 39597 gravados no cache), 7012 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 1 afirmações (1 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 3 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 1 de 1
- Regras: 0 de 0 (100%)
- Outras regras do gabarito encontradas: RN-FAT-012, RN-FAT-013
- Afirmações fora do gabarito: nenhuma
- Perguntas esperadas:
  - A história muda a regra que proíbe cancelar fatura com pagamento parcial, ou o texto está errado?: pergunta 1
- Veredito: esperado precisa de retrabalho, registrado precisa de retrabalho (confere)
- Achados: 7 registrado(s); evidências válidas 5 de 5
- Achados esperados: 3 de 3
  - REV-FAT-001-C1: achado 1
  - REV-FAT-001-L1: achado 3
  - REV-FAT-001-L2: achado 4
- Termos vagos apontados: 4 de 4
- Concluída: sim

#### reuniao REU-FAT-001 (repetição 1)

- Claude: custo US$ 0.25, 30 turnos, 53 s, resultado `success`
- Tokens: 300710 de entrada (265896 do cache, 34796 gravados no cache), 8241 de saída
- Transcript: 29 falas, 418 palavras, 5 min 35 s
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Ata R-0001 (rascunho), análise A-0001 concluída: 17 itens registrados; objetivo: sim; resumo: sim
- Papéis: 2 de 4 (50%)
  - Silva, Maria: esperado gerente/patrocinador, proposto PO (acertou)
  - João Pereira: esperado usuário-chave, proposto desenvolvedor
  - Ana Souza: esperado analista de negócio, proposto analista de negócio (acertou)
  - carlos.lima@exemplo.com: esperado desenvolvedor, proposto Desenvolvedor de banco
- Itens esperados: 16 de 16 (100%)
  - REU-FAT-001-D1 (decisao) Decidido levar a checagem dos 30 dias com supervisor para a procedure do banco.: atendido por D-1 (cita a fala esperada)
  - REU-FAT-001-D2 (decisao) Fica mantida a exigência de motivo com no mínimo 10 caracteres.: atendido por D-3 (cita a fala esperada)
  - REU-FAT-001-D3 (decisao) O relatório de cancelamentos vira uma demanda separada, feita depois da trava.: atendido por D-2 (cita a fala esperada)
  - REU-FAT-001-N1 (necessidade) Exigir o supervisor para faturas antigas em todos os canais, não só na tela Delphi.: atendido por N-1 (cita a fala esperada)
  - REU-FAT-001-N2 (necessidade) Relatório por mês dos cancelamentos de cada usuário, para acompanhamento da auditoria.: atendido por N-2 (cita a fala esperada)
  - REU-FAT-001-N3 (necessidade) Campos do relatório: motivo, data, usuário e valor da fatura.: atendido por N-2 (cita a fala esperada)
  - REU-FAT-001-R1 (regra) Nenhum perfil cancela fatura que já recebeu pagamento parcial (RN-FAT-012).: atendido por R-2 (cita a fala esperada)
  - REU-FAT-001-R2 (regra) Cancelar fatura com mais de 30 dias desde a emissão exige perfil SUPERVISOR (RN-FAT-014).: atendido por R-1 (cita a fala esperada)
  - REU-FAT-001-P1 (pendencia) Carlos levanta onde o cancelamento acontece hoje (banco, backend e lote); prazo sexta, dia 20.: atendido por P-2 (responsável confere, prazo confere, cita a fala esperada)
  - REU-FAT-001-P2 (pendencia) Ana redige a demanda, com critérios de aceite, para revisão da Maria; prazo quarta, dia 18.: atendido por P-3 (responsável confere, prazo confere, cita a fala esperada)
  - REU-FAT-001-P3 (pendencia) Maria consulta a auditoria sobre o marco dos 30 dias; prazo dia 25.: atendido por P-1 (responsável confere, prazo confere, cita a fala esperada)
  - REU-FAT-001-DV1 (duvida) O prazo de 30 dias conta a partir da emissão ou do vencimento?: atendido por DV-1 (cita a fala esperada)
  - REU-FAT-001-DV2 (duvida) Cancelar fatura de órgão público exige alguma aprovação adicional?: atendido por DV-2 (cita a fala esperada)
  - REU-FAT-001-DV3 (duvida) Qual perfil de usuário vale quando o cancelamento chega pelo lote?: atendido por DV-3 (cita a fala esperada)
  - REU-FAT-001-PP1 (proximo_passo) Abrir a análise de impacto sobre a trava.: atendido por PP-1 (cita a fala esperada)
  - REU-FAT-001-PP2 (proximo_passo) Nova reunião em 27 para concluir o escopo.: atendido por PP-2 (cita a fala esperada)
- Itens registrados por seção: decisoes 3, necessidades 2, regras 3, pendencias 3, duvidas 3, proximos_passos 3
- Seções vazias: nenhuma
- Citações: 30 (30 `conferida`, 0 `conferida em`, 0 `não confere`); conferidas de novo: 30 de 30

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
- **Papel acertado:** a proposta do agente (`propor_papel`) para o participante é o papel esperado ou um dos aceitos no gabarito (sem acentos nem maiúsculas; o nome casa como no catálogo de pessoas).
- **Item da ata atendido:** um item registrado na seção do item esperado (ou em uma das seções aceitas) tem um termo de cada grupo de palavras-chave do gabarito. Nas pendências, o responsável e o prazo registrados são conferidos à parte.
- **Citação conferida de novo:** o trecho aparece na fala citada (falante e início) do transcript analisado.
- **Falha grave (reunião):** estado da ata ausente, nenhum item, item sem citação ou citação que não confere.

Detalhes em `docs/avaliacao.md`.
