# Avaliação do fluxo reuniao: exemplo de 2026-11-14 (sonnet)

Primeira execução da avaliação do fluxo **Analisar reunião** ([`docs/avaliacao.md`](../avaliacao.md#fluxo-reuniao)), com o modelo `sonnet`, na reunião 1 da fixture (`REU-FAT-001`, 5 min 35 s) e na reunião longa sintética gerada a partir dela (cerca de 2 h e 25 mil palavras), para medir o custo (questão 4 da #93). Comando:

```sh
cargo run -p cba-avaliacao -- rodar --modelo sonnet --fluxos reuniao,reuniao-longa \
  --repeticoes 1 --orcamento 1.5 --orcamento-longa 3
```

Abaixo, o `relatorio.md` gerado. A ata da reunião 1 está em `crates/cba-avaliacao/tests/atas/REU-FAT-001.estado.json` e é conferida pelos testes da crate.

## Resumo

| Execução | Papéis | Itens da ata | Citações conferidas de novo | Custo | Tempo | Tokens |
|---|---|---|---|---|---|---|
| Reunião 1 (29 falas, 418 palavras) | 3 de 4 | 16 de 16 | 27 de 27 | US$ 0,22 | 42 s (28 turnos) | 243 mil de entrada (211 mil do cache), 7 mil de saída |
| Reunião longa (609 falas, 25 mil palavras, 2 h) | 3 de 4 | 16 de 16 | 32 de 32 | US$ 0,76 | 76 s (43 turnos) | 1,71 milhão de entrada (1,59 milhão do cache), 11 mil de saída |

- **Custo de uma reunião de 2 h (questão 4 da #93):** US$ 0,76 e pouco mais de 1 min com o `sonnet`, cerca de 3,5 vezes o da reunião de 5 min. Quase toda a entrada vem do cache de prompt (93%): cada página de `ler_transcript` (até 10 mil caracteres de fala) entra uma vez e é relida do cache nos turnos seguintes. O transcript longo é repetitivo (a reunião 1 repetida em 21 blocos); uma reunião real de 2 h, com mais assuntos, deve gerar mais itens e mais turnos de registro, e o custo deve ficar acima disso, mas na mesma ordem de grandeza.
- **Itens:** as duas atas atendem a todos os 16 itens esperados, nas seções certas, com a fala esperada citada; as pendências com o responsável e o prazo ditos. O agente juntou necessidades (o relatório mensal e os campos dele num item só) e registrou o mínimo de 10 caracteres como regra, o que o gabarito aceita. Na reunião longa, não repetiu itens por causa das repetições.
- **Papéis:** nas duas execuções o agente propôs "desenvolvedor" para João Pereira, que o gabarito tem como usuário-chave (justificativa do agente: explica como o projeto funciona hoje e sugere a solução técnica). Na reunião 1, só a fala dele sobre a tela de retaguarda indica o papel; ele se apresenta como usuário da retaguarda só na reunião 2. O gabarito ficou como está: é o papel certo, e o erro mostra o valor do histórico de papéis do catálogo de pessoas (a sugestão do catálogo, quando houver reunião anterior confirmada). A Maria foi proposta como PO, que o gabarito aceita junto com gerente/patrocinador.
- **Citações e privacidade:** todas as citações conferem de novo contra o transcript (27 e 32), nenhuma falha grave e nenhum arquivo do projeto alterado. O relatório não traz texto de fala; as descrições dos itens esperados são do gabarito, escritas sem copiar as falas (conferido pelo teste da crate `cba-fixtures`).

Ajustes depois desta execução, sem mudar o resultado (recomparando as duas atas com `cba-avaliacao comparar --ata`): a coluna "Concluídas" das reuniões passou a contar a análise da reunião concluída (a primeira versão do relatório mostrava 0 de 1; as duas análises foram concluídas), e as descrições dos itens esperados foram reescritas para não repetir as falas. O relatório abaixo já está com os dois ajustes.

## Relatório gerado

- Data: 2026-11-14-012512
- Modelo: `sonnet`
- Claude Code: 2.1.325 (Claude Code)
- Commit avaliado: 881a056 (primeiro commit da branch da #137, antes do rebase)
- Orçamento por execução: US$ 1.50; repetições: 1; itens: REU-FAT-001, REU-FAT-001 (longa)
- Edição liberada para o agente (Edit, Write, Bash): não
- Reunião longa sintética: cerca de 120 min e 25000 palavras; orçamento de US$ 3.00
- Custo total: US$ 0.98
- Falhas graves: nenhuma

## Reuniões

| Fluxo | Reunião | Execuções | Papéis (média) | Itens da ata (média; mín–máx) | Citações conferidas de novo | Seção vazia com itens esperados | Concluídas | Custo (US$) | Tempo | Tokens |
|---|---|---|---|---|---|---|---|---|---|---|
| reuniao | REU-FAT-001 | 1 | 75% de 4 | 100% de 16 (100%–100%) | 27 de 27 | 0 | 1 de 1 | 0.22 | 42 s | 242619 de entrada (211005 do cache, 31598 gravados no cache), 7242 de saída |
| reuniao-longa | REU-FAT-001 | 1 | 75% de 4 | 100% de 16 (100%–100%) | 32 de 32 | 0 | 1 de 1 | 0.76 | 1 min 16 s | 1711953 de entrada (1587968 do cache, 123941 gravados no cache), 10536 de saída |

Itens atendidos por seção (soma das execuções):

| Fluxo | Reunião | Seção | Atendidos |
|---|---|---|---|
| reuniao | REU-FAT-001 | decisoes | 3 de 3 |
| reuniao | REU-FAT-001 | necessidades | 3 de 3 |
| reuniao | REU-FAT-001 | regras | 2 de 2 |
| reuniao | REU-FAT-001 | pendencias | 3 de 3 |
| reuniao | REU-FAT-001 | duvidas | 3 de 3 |
| reuniao | REU-FAT-001 | proximos_passos | 2 de 2 |
| reuniao-longa | REU-FAT-001 | decisoes | 3 de 3 |
| reuniao-longa | REU-FAT-001 | necessidades | 3 de 3 |
| reuniao-longa | REU-FAT-001 | regras | 2 de 2 |
| reuniao-longa | REU-FAT-001 | pendencias | 3 de 3 |
| reuniao-longa | REU-FAT-001 | duvidas | 3 de 3 |
| reuniao-longa | REU-FAT-001 | proximos_passos | 2 de 2 |

## Execuções

### reuniao REU-FAT-001 (repetição 1)

- Claude: custo US$ 0.22, 28 turnos, 42 s, resultado `success`
- Tokens: 242619 de entrada (211005 do cache, 31598 gravados no cache), 7242 de saída
- Transcript: 29 falas, 418 palavras, 5 min 35 s
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Ata R-0001 (rascunho), análise A-0001 concluída: 15 itens registrados; objetivo: sim; resumo: sim
- Papéis: 3 de 4 (75%)
  - Silva, Maria: esperado gerente/patrocinador, proposto PO (acertou)
  - João Pereira: esperado usuário-chave, proposto desenvolvedor
  - Ana Souza: esperado analista de negócio, proposto analista de negócio (acertou)
  - carlos.lima@exemplo.com: esperado desenvolvedor, proposto desenvolvedor (acertou)
- Itens esperados: 16 de 16 (100%)
  - REU-FAT-001-D1 (decisao) Decidido levar a checagem dos 30 dias com supervisor para a procedure do banco.: atendido por D-1 (cita a fala esperada)
  - REU-FAT-001-D2 (decisao) Fica mantida a exigência de motivo com no mínimo 10 caracteres.: atendido por R-3 (cita a fala esperada)
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
- Itens registrados por seção: decisoes 2, necessidades 2, regras 3, pendencias 3, duvidas 3, proximos_passos 2
- Seções vazias: nenhuma
- Citações: 27 (27 `conferida`, 0 `conferida em`, 0 `não confere`); conferidas de novo: 27 de 27

### reuniao-longa REU-FAT-001 (repetição 1)

- Claude: custo US$ 0.76, 43 turnos, 76 s, resultado `success`
- Tokens: 1711953 de entrada (1587968 do cache, 123941 gravados no cache), 10536 de saída
- Transcript: 609 falas, 25009 palavras, 120 min 47 s
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Ata R-0001 (rascunho), análise A-0001 concluída: 16 itens registrados; objetivo: sim; resumo: sim
- Papéis: 3 de 4 (75%)
  - Silva, Maria: esperado gerente/patrocinador, proposto PO (acertou)
  - João Pereira: esperado usuário-chave, proposto desenvolvedor
  - Ana Souza: esperado analista de negócio, proposto analista de negócio (acertou)
  - carlos.lima@exemplo.com: esperado desenvolvedor, proposto desenvolvedor (acertou)
- Itens esperados: 16 de 16 (100%)
  - REU-FAT-001-D1 (decisao) Decidido levar a checagem dos 30 dias com supervisor para a procedure do banco.: atendido por D-1 (cita a fala esperada)
  - REU-FAT-001-D2 (decisao) Fica mantida a exigência de motivo com no mínimo 10 caracteres.: atendido por R-3 (cita a fala esperada)
  - REU-FAT-001-D3 (decisao) O relatório de cancelamentos vira uma demanda separada, feita depois da trava.: atendido por D-2 (cita a fala esperada)
  - REU-FAT-001-N1 (necessidade) Exigir o supervisor para faturas antigas em todos os canais, não só na tela Delphi.: atendido por N-1 (cita a fala esperada)
  - REU-FAT-001-N2 (necessidade) Relatório por mês dos cancelamentos de cada usuário, para acompanhamento da auditoria.: atendido por N-2 (cita a fala esperada)
  - REU-FAT-001-N3 (necessidade) Campos do relatório: motivo, data, usuário e valor da fatura.: atendido por N-2 (cita a fala esperada)
  - REU-FAT-001-R1 (regra) Nenhum perfil cancela fatura que já recebeu pagamento parcial (RN-FAT-012).: atendido por R-2 (cita a fala esperada)
  - REU-FAT-001-R2 (regra) Cancelar fatura com mais de 30 dias desde a emissão exige perfil SUPERVISOR (RN-FAT-014).: atendido por R-1 (cita a fala esperada)
  - REU-FAT-001-P1 (pendencia) Carlos levanta onde o cancelamento acontece hoje (banco, backend e lote); prazo sexta, dia 20.: atendido por P-1 (responsável confere, prazo confere, cita a fala esperada)
  - REU-FAT-001-P2 (pendencia) Ana redige a demanda, com critérios de aceite, para revisão da Maria; prazo quarta, dia 18.: atendido por P-2 (responsável confere, prazo confere, cita a fala esperada)
  - REU-FAT-001-P3 (pendencia) Maria consulta a auditoria sobre o marco dos 30 dias; prazo dia 25.: atendido por P-3 (responsável confere, prazo confere, cita a fala esperada)
  - REU-FAT-001-DV1 (duvida) O prazo de 30 dias conta a partir da emissão ou do vencimento?: atendido por DV-1 (cita a fala esperada)
  - REU-FAT-001-DV2 (duvida) Cancelar fatura de órgão público exige alguma aprovação adicional?: atendido por DV-2 (cita a fala esperada)
  - REU-FAT-001-DV3 (duvida) Qual perfil de usuário vale quando o cancelamento chega pelo lote?: atendido por DV-3 (cita a fala esperada)
  - REU-FAT-001-PP1 (proximo_passo) Abrir a análise de impacto sobre a trava.: atendido por PP-1 (cita a fala esperada)
  - REU-FAT-001-PP2 (proximo_passo) Nova reunião em 27 para concluir o escopo.: atendido por PP-2 (cita a fala esperada)
- Itens registrados por seção: decisoes 2, necessidades 2, regras 3, pendencias 3, duvidas 3, proximos_passos 3
- Seções vazias: nenhuma
- Citações: 32 (32 `conferida`, 0 `conferida em`, 0 `não confere`); conferidas de novo: 32 de 32

## Critérios

- **Papel acertado:** a proposta do agente (`propor_papel`) para o participante é o papel esperado ou um dos aceitos no gabarito (sem acentos nem maiúsculas; o nome casa como no catálogo de pessoas).
- **Item da ata atendido:** um item registrado na seção do item esperado (ou em uma das seções aceitas) tem um termo de cada grupo de palavras-chave do gabarito. Nas pendências, o responsável e o prazo registrados são conferidos à parte.
- **Citação conferida de novo:** o trecho aparece na fala citada (falante e início) do transcript analisado.
- **Falha grave (reunião):** estado da ata ausente, nenhum item, item sem citação ou citação que não confere.

Detalhes em `docs/avaliacao.md`.
