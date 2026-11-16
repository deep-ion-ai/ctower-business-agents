# Avaliação dos fluxos contra o gabarito

- Data: 2026-11-16-035042
- Modelo: `sonnet`
- Claude Code: 2.1.325 (Claude Code)
- Commit avaliado: e541283
- Orçamento por execução: US$ 1.50; repetições: 3; itens: REV-FAT-001, HIS-FAT-001, CEN-FAT-001
- Edição liberada para o agente (Edit, Write, Bash): não
- Custo total: US$ 2.16
- Falhas graves: nenhuma

## Cobertura por fluxo

| Fluxo | Item | Execuções | Regras (média; mín–máx) | Evidências válidas | `confirmado` sem evidência válida | Perguntas esperadas | Premissa corrigida | Abrangência | Arquivos esperados citados | Arquivos alterados | Concluídas | Custo (US$) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| revisar | REV-FAT-001 | 3 | 100% de 0 (100%–100%) | 3 de 3 | 0 | 100% | n/a | n/a | n/a | 0 | 3 de 3 | 0.76 |
| historias | HIS-FAT-001 | 3 | 0% de 3 (0%–0%) | 0 de 0 | 0 | n/a | n/a | 0 de 3 | n/a | 0 | 3 de 3 | 0.45 |
| cenarios | CEN-FAT-001 | 3 | 0% de 3 (0%–0%) | 0 de 0 | 0 | n/a | n/a | n/a | n/a | 0 | 3 de 3 | 0.95 |

## Execuções

### revisar REV-FAT-001 (repetição 1)

- Claude: custo US$ 0.24, 24 turnos, 45 s, resultado `success`
- Tokens: 279876 de entrada (243490 do cache, 36370 gravados no cache), 6820 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 1 afirmações (1 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 3 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 1 de 1
- Regras: 0 de 0 (100%)
- Outras regras do gabarito encontradas: RN-FAT-012
- Afirmações fora do gabarito: nenhuma
- Perguntas esperadas:
  - A história muda a regra que proíbe cancelar fatura com pagamento parcial, ou o texto está errado?: pergunta 1
- Veredito: esperado precisa de retrabalho, registrado precisa de retrabalho (confere)
- Achados: 7 registrado(s); evidências válidas 5 de 5
- Achados esperados: 3 de 3
  - REV-FAT-001-C1: achado 1
  - REV-FAT-001-L1: achado 2
  - REV-FAT-001-L2: achado 3
- Termos vagos apontados: 4 de 4
- Concluída: sim

### revisar REV-FAT-001 (repetição 2)

- Claude: custo US$ 0.25, 26 turnos, 47 s, resultado `success`
- Tokens: 355329 de entrada (318199 do cache, 37110 gravados no cache), 6729 de saída
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
- Achados: 7 registrado(s); evidências válidas 4 de 4
- Achados esperados: 3 de 3
  - REV-FAT-001-C1: achado 1
  - REV-FAT-001-L1: achado 3
  - REV-FAT-001-L2: achado 4
- Termos vagos apontados: 4 de 4
- Concluída: sim

### revisar REV-FAT-001 (repetição 3)

- Claude: custo US$ 0.27, 28 turnos, 51 s, resultado `success`
- Tokens: 423990 de entrada (383576 do cache, 40392 gravados no cache), 7265 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 1 afirmações (1 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 4 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 1 de 1
- Regras: 0 de 0 (100%)
- Outras regras do gabarito encontradas: RN-FAT-012, RN-FAT-013
- Afirmações fora do gabarito: nenhuma
- Perguntas esperadas:
  - A história muda a regra que proíbe cancelar fatura com pagamento parcial, ou o texto está errado?: pergunta 1
- Veredito: esperado precisa de retrabalho, registrado precisa de retrabalho (confere)
- Achados: 7 registrado(s); evidências válidas 4 de 4
- Achados esperados: 3 de 3
  - REV-FAT-001-C1: achado 1
  - REV-FAT-001-L1: achado 6
  - REV-FAT-001-L2: achado 7
- Termos vagos apontados: 4 de 4
- Concluída: sim

### historias HIS-FAT-001 (repetição 1)

- Claude: custo US$ 0.15, 9 turnos, 32 s, resultado `success`
- Tokens: 189399 de entrada (166183 do cache, 23202 gravados no cache), 3895 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0002 (concluida): 0 afirmações (0 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 0 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 0 de 0
- Regras: 0 de 3 (0%)
  - RN-FAT-007 Parcelamento só para pessoa jurídica e órgão público: não encontrada
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: não encontrada
  - RN-FAT-009 Valor mínimo da parcela, exceto órgão público: não encontrada
- Afirmações fora do gabarito: nenhuma
- Abrangência: esperada media, registrada nenhuma (diverge)
- Histórias: 2; critérios: 8 (8 com valor concreto)
- Histórias esperadas: 2 de 2
  - HIS-FAT-001-H1: HU-0001
  - HIS-FAT-001-H2: HU-0001
- Regras citadas pelos critérios: 3 de 3
  - RN-FAT-007: HU-0001 CA-1
  - RN-FAT-008: HU-0001 CA-3
  - RN-FAT-009: HU-0001 CA-1
- A confirmar: 1 de 1 tema(s) marcados
- Quebra em fatias: proposta
- Concluída: sim

### historias HIS-FAT-001 (repetição 2)

- Claude: custo US$ 0.15, 10 turnos, 35 s, resultado `success`
- Tokens: 188051 de entrada (165015 do cache, 23022 gravados no cache), 4331 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0002 (concluida): 0 afirmações (0 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 0 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 0 de 0
- Regras: 0 de 3 (0%)
  - RN-FAT-007 Parcelamento só para pessoa jurídica e órgão público: não encontrada
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: não encontrada
  - RN-FAT-009 Valor mínimo da parcela, exceto órgão público: não encontrada
- Afirmações fora do gabarito: nenhuma
- Abrangência: esperada media, registrada nenhuma (diverge)
- Histórias: 3; critérios: 8 (8 com valor concreto)
- Histórias esperadas: 2 de 2
  - HIS-FAT-001-H1: HU-0001
  - HIS-FAT-001-H2: HU-0001
- Regras citadas pelos critérios: 3 de 3
  - RN-FAT-007: HU-0001 CA-1
  - RN-FAT-008: HU-0001 CA-1
  - RN-FAT-009: HU-0002 CA-1
- A confirmar: 1 de 1 tema(s) marcados
- Quebra em fatias: proposta
- Concluída: sim

### historias HIS-FAT-001 (repetição 3)

- Claude: custo US$ 0.15, 9 turnos, 35 s, resultado `success`
- Tokens: 163321 de entrada (139939 do cache, 23370 gravados no cache), 4149 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0002 (concluida): 0 afirmações (0 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 0 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 0 de 0
- Regras: 0 de 3 (0%)
  - RN-FAT-007 Parcelamento só para pessoa jurídica e órgão público: não encontrada
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: não encontrada
  - RN-FAT-009 Valor mínimo da parcela, exceto órgão público: não encontrada
- Afirmações fora do gabarito: nenhuma
- Abrangência: esperada media, registrada nenhuma (diverge)
- Histórias: 2; critérios: 8 (8 com valor concreto)
- Histórias esperadas: 2 de 2
  - HIS-FAT-001-H1: HU-0001
  - HIS-FAT-001-H2: HU-0001
- Regras citadas pelos critérios: 3 de 3
  - RN-FAT-007: HU-0001 CA-1
  - RN-FAT-008: HU-0001 CA-2
  - RN-FAT-009: HU-0001 CA-3
- A confirmar: 1 de 1 tema(s) marcados
- Quebra em fatias: proposta
- Concluída: sim

### cenarios CEN-FAT-001 (repetição 1)

- Claude: custo US$ 0.30, 28 turnos, 64 s, resultado `success`
- Tokens: 337667 de entrada (295946 do cache, 41703 gravados no cache), 10716 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 0 afirmações (0 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 0 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 0 de 0
- Regras: 0 de 3 (0%)
  - RN-FAT-012 Cancelamento só de fatura aberta e sem pagamento: não encontrada
  - RN-FAT-013 Motivo do cancelamento com ao menos 10 caracteres: não encontrada
  - RN-FAT-014 Cancelamento após 30 dias da emissão exige supervisor: não encontrada
- Afirmações fora do gabarito: nenhuma
- Cenários: 12; tabelas de decisão: 2
- Limites lidos do código: 2 de 2
  - CEN-FAT-001-L1 (RN-FAT-013, limite 10): CT-0009; valores testados: 9 (CT-0009), 10 (CT-0010)
  - CEN-FAT-001-L2 (RN-FAT-014, limite 30): CT-0007; valores testados: 30 (CT-0007), 31 (CT-0008)
- Tabelas de decisão esperadas: 1 de 1
  - CEN-FAT-001-T1: TD-2
- Negativos esperados: 2 de 2
  - CEN-FAT-001-N1: CT-0001
  - CEN-FAT-001-N2: CT-0001
- Concluída: sim

### cenarios CEN-FAT-001 (repetição 2)

- Claude: custo US$ 0.29, 28 turnos, 64 s, resultado `success`
- Tokens: 307770 de entrada (269429 do cache, 38323 gravados no cache), 10805 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 0 afirmações (0 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 1 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 0 de 0
- Regras: 0 de 3 (0%)
  - RN-FAT-012 Cancelamento só de fatura aberta e sem pagamento: não encontrada
  - RN-FAT-013 Motivo do cancelamento com ao menos 10 caracteres: não encontrada
  - RN-FAT-014 Cancelamento após 30 dias da emissão exige supervisor: não encontrada
- Afirmações fora do gabarito: nenhuma
- Cenários: 13; tabelas de decisão: 2
- Limites lidos do código: 2 de 2
  - CEN-FAT-001-L1 (RN-FAT-013, limite 10): CT-0009; valores testados: 9 (CT-0009), 10 (CT-0010)
  - CEN-FAT-001-L2 (RN-FAT-014, limite 30): CT-0007; valores testados: 30 (CT-0007), 31 (CT-0008)
- Tabelas de decisão esperadas: 1 de 1
  - CEN-FAT-001-T1: TD-2
- Negativos esperados: 2 de 2
  - CEN-FAT-001-N1: CT-0001
  - CEN-FAT-001-N2: CT-0001
- Concluída: sim

### cenarios CEN-FAT-001 (repetição 3)

- Claude: custo US$ 0.36, 35 turnos, 86 s, resultado `success`
- Tokens: 533704 de entrada (490759 do cache, 42917 gravados no cache), 13705 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 0 afirmações (0 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 1 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 0 de 0
- Regras: 0 de 3 (0%)
  - RN-FAT-012 Cancelamento só de fatura aberta e sem pagamento: não encontrada
  - RN-FAT-013 Motivo do cancelamento com ao menos 10 caracteres: não encontrada
  - RN-FAT-014 Cancelamento após 30 dias da emissão exige supervisor: não encontrada
- Afirmações fora do gabarito: nenhuma
- Cenários: 16; tabelas de decisão: 2
- Limites lidos do código: 2 de 2
  - CEN-FAT-001-L1 (RN-FAT-013, limite 10): CT-0009; valores testados: 9 (CT-0003), 10 (CT-0010)
  - CEN-FAT-001-L2 (RN-FAT-014, limite 30): CT-0011; valores testados: 30 (CT-0011), 31 (CT-0012)
- Tabelas de decisão esperadas: 1 de 1
  - CEN-FAT-001-T1: TD-2
- Negativos esperados: 2 de 2
  - CEN-FAT-001-N1: CT-0001
  - CEN-FAT-001-N2: CT-0001
- Concluída: sim

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
- **Histórias e critérios** (`historias`): história esperada atendida por uma história com as palavras-chave no título e na narrativa; regra esperada citada por um critério, pelo id ou por uma afirmação cuja evidência toca a regra; critério com valor concreto (um número); tema a confirmar marcado num critério com pergunta; quebra proposta quando a abrangência pede. Falha grave: nenhuma história, ou critério sem origem.

Detalhes em `docs/avaliacao.md`.
