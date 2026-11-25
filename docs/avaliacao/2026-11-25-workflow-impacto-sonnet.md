# Avaliação dos fluxos contra o gabarito

- Data: 2026-11-25-160257
- Modelo: `sonnet`
- Claude Code: 2.1.332 (Claude Code)
- Commit avaliado: 0ef1a84
- Orçamento por execução: US$ 2.00; repetições: 1; itens: D-FAT-001, D-FAT-002, D-FAT-003, D-FAT-004
- Edição liberada para o agente (Edit, Write, Bash): não
- Custo total: US$ 8.14
- Falhas graves: nenhuma

## Cobertura por fluxo

| Fluxo | Item | Execuções | Regras (média; mín–máx) | Evidências válidas | `confirmado` sem evidência válida | Perguntas esperadas | Premissa corrigida | Abrangência | Arquivos esperados citados | Arquivos alterados | Concluídas | Custo (US$) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| impacto | D-FAT-001 [simples] | 1 | 100% de 3 (100%–100%) | 26 de 26 | 0 | 100% | 1 de 1 | 1 de 1 | 100% | 0 | 1 de 1 | 0.42 |
| impacto | D-FAT-001 [workflow] | 1 | 100% de 3 (100%–100%) | 92 de 92 | 0 | 100% | 1 de 1 | 0 de 1 | 100% | 0 | 1 de 1 | 1.47 |
| impacto | D-FAT-002 [simples] | 1 | 100% de 2 (100%–100%) | 29 de 29 | 0 | 100% | 1 de 1 | 1 de 1 | 100% | 0 | 1 de 1 | 0.39 |
| impacto | D-FAT-002 [workflow] | 1 | 100% de 2 (100%–100%) | 55 de 55 | 0 | 100% | 1 de 1 | 1 de 1 | 100% | 0 | 1 de 1 | 1.21 |
| impacto | D-FAT-003 [simples] | 1 | 100% de 3 (100%–100%) | 26 de 26 | 0 | 100% | n/a | 1 de 1 | 100% | 0 | 1 de 1 | 0.37 |
| impacto | D-FAT-003 [workflow] | 1 | 100% de 3 (100%–100%) | 106 de 106 | 0 | 100% | n/a | 1 de 1 | 100% | 0 | 1 de 1 | 2.09 |
| impacto | D-FAT-004 [simples] | 1 | 100% de 1 (100%–100%) | 32 de 32 | 0 | 100% | 1 de 1 | 1 de 1 | 100% | 0 | 1 de 1 | 0.40 |
| impacto | D-FAT-004 [workflow] | 1 | 100% de 1 (100%–100%) | 95 de 95 | 0 | 100% | 1 de 1 | 1 de 1 | 100% | 0 | 1 de 1 | 1.79 |

## Condução simples e workflow

| Item | Condução (pedida → efetiva) | Regras | Perguntas esperadas | Premissa corrigida | Abrangência | Afirmações | Contestadas | Agentes | Tokens (todos os modelos) | Custo (US$) | Duração (relógio) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| D-FAT-001 [simples] | simples → simples | 3 de 3 | 2 de 2 | sim | sim (pequena) | 13 | 0 | - | 621 mil | 0.42 | 1 min 22 s |
| D-FAT-001 [workflow] | workflow → workflow | 3 de 3 | 2 de 2 | sim | não (media) | 30 | 0 | 8 | 2.1 mi | 1.47 | 2 min 53 s |
| D-FAT-002 [simples] | simples → simples | 2 de 2 | 1 de 1 | sim | sim (media) | 14 | 0 | - | 611 mil | 0.39 | 1 min 30 s |
| D-FAT-002 [workflow] | workflow → workflow | 2 de 2 | 1 de 1 | sim | sim (media) | 22 | 0 | 6 | 2.5 mi | 1.21 | 3 min 07 s |
| D-FAT-003 [simples] | simples → simples | 3 de 3 | 2 de 2 | n/a | sim (media) | 12 | 0 | - | 629 mil | 0.37 | 1 min 30 s |
| D-FAT-003 [workflow] | workflow → workflow | 3 de 3 | 2 de 2 | n/a | sim (media) | 42 | 0 | 10 | 3.7 mi | 2.09 | 3 min 35 s |
| D-FAT-004 [simples] | simples → simples | 1 de 1 | 4 de 4 | sim | sim (grande) | 16 | 0 | - | 514 mil | 0.40 | 1 min 29 s |
| D-FAT-004 [workflow] | workflow → workflow | 1 de 1 | 4 de 4 | sim | sim (grande) | 36 | 1 | 10 | 2.8 mi | 1.79 | 2 min 52 s |


### Tokens por fase: D-FAT-001 [workflow] (situação do workflow: concluido)

| Fase | Agentes | Tokens | Por agente (mín–máx) | Saída |
|---|---|---|---|---|
| Preparar | 1 | 142 mil | 142 mil–142 mil | 3 mil |
| Investigar | 3 | 1.4 mi | 407 mil–533 mil | 42 mil |
| Verificar | 3 | 182 mil | 49 mil–84 mil | 3 mil |
| Consolidar | 1 | 193 mil | 193 mil–193 mil | 3 mil |

### Tokens por fase: D-FAT-002 [workflow] (situação do workflow: concluido)

| Fase | Agentes | Tokens | Por agente (mín–máx) | Saída |
|---|---|---|---|---|
| Preparar | 1 | 131 mil | 131 mil–131 mil | 2 mil |
| Investigar | 2 | 2.0 mi | 801 mil–1.2 mi | 32 mil |
| Verificar | 2 | 85 mil | 30 mil–55 mil | 1 mil |
| Consolidar | 1 | 100 mil | 100 mil–100 mil | 3 mil |

### Tokens por fase: D-FAT-003 [workflow] (situação do workflow: concluido)

| Fase | Agentes | Tokens | Por agente (mín–máx) | Saída |
|---|---|---|---|---|
| Preparar | 1 | 116 mil | 116 mil–116 mil | 4 mil |
| Investigar | 4 | 2.9 mi | 439 mil–1.3 mi | 64 mil |
| Verificar | 4 | 207 mil | 32 mil–72 mil | 3 mil |
| Consolidar | 1 | 267 mil | 267 mil–267 mil | 4 mil |

### Tokens por fase: D-FAT-004 [workflow] (situação do workflow: concluido)

| Fase | Agentes | Tokens | Por agente (mín–máx) | Saída |
|---|---|---|---|---|
| Preparar | 1 | 170 mil | 170 mil–170 mil | 4 mil |
| Investigar | 4 | 2.1 mi | 371 mil–677 mil | 53 mil |
| Verificar | 4 | 216 mil | 30 mil–72 mil | 3 mil |
| Consolidar | 1 | 148 mil | 148 mil–148 mil | 4 mil |

## Execuções

### impacto D-FAT-001 [simples] (repetição 1)

- Claude: custo US$ 0.42, 37 turnos, 82 s, resultado `success`
- Tokens: 607949 de entrada (549501 do cache, 58422 gravados no cache), 12652 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 13 afirmações (10 `confirmado`, 3 `inferido`, 0 `a_confirmar`), 4 perguntas, 0 sugestões, 1 termos
- Evidências válidas: 26 de 26
- Regras: 3 de 3 (100%)
  - RN-FAT-007 Parcelamento só para pessoa jurídica e órgão público: encontrada (afirmações 2, 10)
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: encontrada (afirmações 3, 4, 8, 10, 11, 13)
  - INC-FAT-001 Tela oferece até 12 parcelas; backend aceita até 6: encontrada (afirmações 3, 8, 13)
- Outras regras do gabarito encontradas: RN-FAT-009, RN-FAT-010, RN-FAT-011
- Afirmações fora do gabarito: 7, 9, 12
- Arquivos esperados citados: 3 de 3
- Perguntas esperadas:
  - O limite de 3 parcelas vale também para órgão público, que usa o mesmo parâmetro?: pergunta 1
  - A tela oferece até 12 parcelas; deve ser alinhada ao novo limite?: pergunta 4
- Premissa corrigida: sim (afirmação 1)
- Abrangência: esperada pequena, registrada pequena (confere)
- Concluída: sim

### impacto D-FAT-001 [workflow] (repetição 1)

- Claude: custo US$ 1.47, 3 turnos, 173 s, resultado `success`
- Tokens: 82627 de entrada (79075 do cache, 3546 gravados no cache), 1485 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 30 afirmações (18 `confirmado`, 9 `inferido`, 3 `a_confirmar`), 7 perguntas, 0 sugestões, 4 termos
- Evidências válidas: 92 de 92
- Regras: 3 de 3 (100%)
  - RN-FAT-007 Parcelamento só para pessoa jurídica e órgão público: encontrada (afirmações 2, 5, 10, 17, 21, 29)
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: encontrada (afirmações 1, 3, 4, 5, 6, 10, 11, 17, 19, 21, 22, 23, 29, 30)
  - INC-FAT-001 Tela oferece até 12 parcelas; backend aceita até 6: encontrada (afirmações 3, 4, 11, 29)
- Outras regras do gabarito encontradas: INC-FAT-002, INC-FAT-003, RN-FAT-001, RN-FAT-002, RN-FAT-003, RN-FAT-004, RN-FAT-005, RN-FAT-006, RN-FAT-009, RN-FAT-010, RN-FAT-011, RN-FAT-012, RN-FAT-015, RN-FAT-016
- Afirmações fora do gabarito: 9, 12, 15, 18, 20, 25, 28
- Arquivos esperados citados: 3 de 3
- Perguntas esperadas:
  - O limite de 3 parcelas vale também para órgão público, que usa o mesmo parâmetro?: pergunta 1
  - A tela oferece até 12 parcelas; deve ser alinhada ao novo limite?: pergunta 6
- Premissa corrigida: sim (afirmação 1)
- Abrangência: esperada pequena, registrada media (diverge)
- Concluída: sim

### impacto D-FAT-002 [simples] (repetição 1)

- Claude: custo US$ 0.39, 35 turnos, 90 s, resultado `success`
- Tokens: 599438 de entrada (545043 do cache, 54369 gravados no cache), 11428 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 14 afirmações (11 `confirmado`, 3 `inferido`, 0 `a_confirmar`), 3 perguntas, 0 sugestões, 1 termos
- Evidências válidas: 29 de 29
- Regras: 2 de 2 (100%)
  - RN-FAT-001 Multa por atraso vem de parâmetro: encontrada (afirmações 1, 2, 3, 4, 9, 11, 14)
  - INC-FAT-002 Lote COBOL usa valores fixos e não aplica exceções do backend: encontrada (afirmações 2, 6, 12)
- Outras regras do gabarito encontradas: RN-FAT-003, RN-FAT-004, RN-FAT-005, RN-FAT-006, RN-FAT-011, RN-FAT-013, RN-FAT-015, RN-FAT-016
- Afirmações fora do gabarito: nenhuma
- Arquivos esperados citados: 2 de 2
- Perguntas esperadas:
  - O lote de cobrança, que usa 2% fixo, deve ser alterado junto?: pergunta 2
- Premissa corrigida: sim (afirmação 2)
- Abrangência: esperada media, registrada media (confere)
- Concluída: sim

### impacto D-FAT-002 [workflow] (repetição 1)

- Claude: custo US$ 1.21, 3 turnos, 187 s, resultado `success`
- Tokens: 82018 de entrada (78662 do cache, 3350 gravados no cache), 1252 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 22 afirmações (15 `confirmado`, 6 `inferido`, 1 `a_confirmar`), 6 perguntas, 0 sugestões, 5 termos
- Evidências válidas: 55 de 55
- Regras: 2 de 2 (100%)
  - RN-FAT-001 Multa por atraso vem de parâmetro: encontrada (afirmações 1, 2, 3, 4, 8, 10, 15, 19, 20, 21, 22)
  - INC-FAT-002 Lote COBOL usa valores fixos e não aplica exceções do backend: encontrada (afirmações 3, 7, 11)
- Outras regras do gabarito encontradas: RN-FAT-002, RN-FAT-003, RN-FAT-004, RN-FAT-005, RN-FAT-006, RN-FAT-011, RN-FAT-014, RN-FAT-015, RN-FAT-016
- Afirmações fora do gabarito: 12, 16, 18
- Arquivos esperados citados: 2 de 2
- Perguntas esperadas:
  - O lote de cobrança, que usa 2% fixo, deve ser alterado junto?: pergunta 2
- Premissa corrigida: sim (afirmação 2)
- Abrangência: esperada media, registrada media (confere)
- Concluída: sim

### impacto D-FAT-003 [simples] (repetição 1)

- Claude: custo US$ 0.37, 30 turnos, 90 s, resultado `success`
- Tokens: 617907 de entrada (566798 do cache, 51081 gravados no cache), 10754 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 12 afirmações (6 `confirmado`, 6 `inferido`, 0 `a_confirmar`), 3 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 26 de 26
- Regras: 3 de 3 (100%)
  - RN-FAT-007 Parcelamento só para pessoa jurídica e órgão público: encontrada (afirmações 1, 8, 12)
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: encontrada (afirmações 2, 3, 5, 8, 11)
  - RN-FAT-009 Valor mínimo da parcela, exceto órgão público: encontrada (afirmações 2, 3, 4, 5, 6, 11)
- Outras regras do gabarito encontradas: RN-FAT-010, RN-FAT-011
- Afirmações fora do gabarito: 7, 9, 10
- Arquivos esperados citados: 5 de 5
- Perguntas esperadas:
  - A parcela mínima de R$ 100,00 vale só para pessoa física?: pergunta 1
  - O limite de 3 parcelas vale só para pessoa física?: pergunta 1
- Abrangência: esperada media, registrada media (confere)
- Concluída: sim

### impacto D-FAT-003 [workflow] (repetição 1)

- Claude: custo US$ 2.09, 3 turnos, 215 s, resultado `success`
- Tokens: 83996 de entrada (79548 do cache, 4442 gravados no cache), 1682 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 42 afirmações (24 `confirmado`, 13 `inferido`, 5 `a_confirmar`), 13 perguntas, 0 sugestões, 6 termos
- Evidências válidas: 106 de 106
- Regras: 3 de 3 (100%)
  - RN-FAT-007 Parcelamento só para pessoa jurídica e órgão público: encontrada (afirmações 1, 6, 12, 15, 19, 20, 25, 26, 32, 34, 41, 42)
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: encontrada (afirmações 3, 10, 20, 25, 26, 29, 32, 41)
  - RN-FAT-009 Valor mínimo da parcela, exceto órgão público: encontrada (afirmações 4, 16, 25, 26, 29, 30, 41)
- Outras regras do gabarito encontradas: INC-FAT-001, INC-FAT-002, INC-FAT-003, RN-FAT-001, RN-FAT-002, RN-FAT-003, RN-FAT-004, RN-FAT-005, RN-FAT-006, RN-FAT-010, RN-FAT-011, RN-FAT-012, RN-FAT-014, RN-FAT-015, RN-FAT-016
- Afirmações fora do gabarito: 2, 5, 7, 8, 11, 17, 21, 23, 24, 28, 31
- Arquivos esperados citados: 5 de 5
- Perguntas esperadas:
  - A parcela mínima de R$ 100,00 vale só para pessoa física?: pergunta 1
  - O limite de 3 parcelas vale só para pessoa física?: pergunta 4
- Abrangência: esperada media, registrada media (confere)
- Concluída: sim

### impacto D-FAT-004 [simples] (repetição 1)

- Claude: custo US$ 0.40, 31 turnos, 89 s, resultado `success`
- Tokens: 499759 de entrada (444353 do cache, 55384 gravados no cache), 13793 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 16 afirmações (11 `confirmado`, 4 `inferido`, 1 `a_confirmar`), 6 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 32 de 32
- Regras: 1 de 1 (100%)
  - RN-FAT-016 Arquivo de cobrança sai sem dados do cliente: encontrada (afirmações 1, 4, 6, 16)
- Outras regras do gabarito encontradas: INC-FAT-002, RN-FAT-001, RN-FAT-003, RN-FAT-004, RN-FAT-013, RN-FAT-015
- Afirmações fora do gabarito: 2, 3, 7, 8, 9, 10, 11, 12
- Arquivos esperados citados: 3 de 3
- Perguntas esperadas:
  - Há contrato ou base legal (LGPD) para compartilhar nome, CPF e e-mail com a empresa terceirizada?: pergunta 4
  - Para pessoa jurídica e órgão público, o documento é CNPJ; vai também no arquivo?: pergunta 1
  - Cliente sem e-mail cadastrado ou que não aceita avisos (ACEITA_AVISO) entra no arquivo?: pergunta 2
  - Onde é gerado o arquivo de faturas vencidas que o lote lê, e qual o volume e a janela do lote?: pergunta 3
- Premissa corrigida: sim (afirmação 1)
- Abrangência: esperada grande, registrada grande (confere)
- Dados pessoais e requisitos não funcionais: 4 afirmação(ões) na seção; campos pessoais apontados 2 de 3; aspectos tratados 3 de 3
  - CLIENTE.NOME (nome): afirmação 11
  - CLIENTE.DOCUMENTO (documento de identificação (CPF de pessoa física)): afirmação 10
  - CLIENTE_CONTATO.EMAIL (contato): não apontado
  - auditoria (O lote não registra em log o que envia ao terceiro (só exibe contadores).): afirmação 13
  - desempenho (O lote mensal passa a ler o cliente e o contato de cada fatura vencida.): afirmação 13
  - acesso (Quem recebe e acessa o arquivo com dados pessoais fora do projeto.): pergunta 3
  - Possível falso positivo: CLIENTE_CONTATO.ACEITA_AVISO (afirmações 12)
- Concluída: sim

### impacto D-FAT-004 [workflow] (repetição 1)

- Claude: custo US$ 1.79, 3 turnos, 172 s, resultado `success`
- Tokens: 84182 de entrada (79534 do cache, 4642 gravados no cache), 1863 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 36 afirmações (15 `confirmado`, 15 `inferido`, 6 `a_confirmar`), 12 perguntas, 0 sugestões, 2 termos
- Evidências válidas: 95 de 95
- Regras: 1 de 1 (100%)
  - RN-FAT-016 Arquivo de cobrança sai sem dados do cliente: encontrada (afirmações 2, 4, 7, 8, 9, 14, 16, 21, 23, 25, 30, 35, 36)
- Outras regras do gabarito encontradas: INC-FAT-002, RN-FAT-001, RN-FAT-002, RN-FAT-003, RN-FAT-004, RN-FAT-013, RN-FAT-015
- Afirmações fora do gabarito: 1, 3, 5, 6, 11, 13, 15, 17, 18, 19, 20, 24, 27, 28, 31, 32, 33, 34
- Arquivos esperados citados: 3 de 3
- Perguntas esperadas:
  - Há contrato ou base legal (LGPD) para compartilhar nome, CPF e e-mail com a empresa terceirizada?: pergunta 1
  - Para pessoa jurídica e órgão público, o documento é CNPJ; vai também no arquivo?: pergunta 7
  - Cliente sem e-mail cadastrado ou que não aceita avisos (ACEITA_AVISO) entra no arquivo?: pergunta 1
  - Onde é gerado o arquivo de faturas vencidas que o lote lê, e qual o volume e a janela do lote?: pergunta 6
- Premissa corrigida: sim (afirmação 4)
- Abrangência: esperada grande, registrada grande (confere)
- Dados pessoais e requisitos não funcionais: 7 afirmação(ões) na seção; campos pessoais apontados 3 de 3; aspectos tratados 3 de 3
  - CLIENTE.NOME (nome): afirmação 3
  - CLIENTE.DOCUMENTO (documento de identificação (CPF de pessoa física)): afirmação 5
  - CLIENTE_CONTATO.EMAIL (contato): afirmação 6
  - auditoria (O lote não registra em log o que envia ao terceiro (só exibe contadores).): afirmação 19
  - desempenho (O lote mensal passa a ler o cliente e o contato de cada fatura vencida.): afirmação 19
  - acesso (Quem recebe e acessa o arquivo com dados pessoais fora do projeto.): afirmação 15
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
