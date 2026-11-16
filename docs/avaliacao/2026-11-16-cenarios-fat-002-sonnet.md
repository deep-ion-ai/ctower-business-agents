# Avaliação dos fluxos contra o gabarito

- Data: 2026-11-16-034612
- Modelo: `sonnet`
- Claude Code: 2.1.325 (Claude Code)
- Commit avaliado: e541283
- Orçamento por execução: US$ 1.50; repetições: 3; itens: CEN-FAT-002
- Edição liberada para o agente (Edit, Write, Bash): não
- Custo total: US$ 0.90
- Falhas graves: nenhuma

## Cobertura por fluxo

| Fluxo | Item | Execuções | Regras (média; mín–máx) | Evidências válidas | `confirmado` sem evidência válida | Perguntas esperadas | Premissa corrigida | Abrangência | Arquivos esperados citados | Arquivos alterados | Concluídas | Custo (US$) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| cenarios | CEN-FAT-002 | 3 | 0% de 1 (0%–0%) | 0 de 0 | 0 | n/a | n/a | n/a | n/a | 0 | 3 de 3 | 0.90 |

## Execuções

### cenarios CEN-FAT-002 (repetição 1)

- Claude: custo US$ 0.27, 27 turnos, 61 s, resultado `success`
- Tokens: 328254 de entrada (292268 do cache, 35966 gravados no cache), 10183 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 0 afirmações (0 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 2 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 0 de 0
- Regras: 0 de 1 (0%)
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: não encontrada
- Afirmações fora do gabarito: nenhuma
- Cenários: 7; tabelas de decisão: 0
- Limites lidos do código: 1 de 1
  - CEN-FAT-002-L1 (RN-FAT-008, limite 2): CT-0001; valores testados: 1 (CT-0001), 2 (CT-0002)
    - Divergentes: CT-0007: 12
- A confirmar: 1 de 1
  - O máximo de parcelas vem do parâmetro MAX_PARCELAS (6 na carga inicial); qual é o valor em produção?: CT-0004
- Concluída: sim

### cenarios CEN-FAT-002 (repetição 2)

- Claude: custo US$ 0.29, 25 turnos, 60 s, resultado `success`
- Tokens: 371465 de entrada (332814 do cache, 38631 gravados no cache), 9734 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 0 afirmações (0 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 1 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 0 de 0
- Regras: 0 de 1 (0%)
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: não encontrada
- Afirmações fora do gabarito: nenhuma
- Cenários: 6; tabelas de decisão: 0
- Limites lidos do código: 1 de 1
  - CEN-FAT-002-L1 (RN-FAT-008, limite 2): CT-0001; valores testados: 1 (CT-0001), 2 (CT-0002)
    - Divergentes: CT-0003: 12; CT-0006: 12
- A confirmar: 1 de 1
  - O máximo de parcelas vem do parâmetro MAX_PARCELAS (6 na carga inicial); qual é o valor em produção?: CT-0004
- Concluída: sim

### cenarios CEN-FAT-002 (repetição 3)

- Claude: custo US$ 0.34, 32 turnos, 77 s, resultado `success`
- Tokens: 445218 de entrada (403699 do cache, 41495 gravados no cache), 12912 de saída
- Falhas graves: nenhuma
- Arquivos do projeto alterados: nenhum
- Análise A-0001 (concluida): 0 afirmações (0 `confirmado`, 0 `inferido`, 0 `a_confirmar`), 2 perguntas, 0 sugestões, 0 termos
- Evidências válidas: 0 de 0
- Regras: 0 de 1 (0%)
  - RN-FAT-008 Quantidade de parcelas entre 2 e MAX_PARCELAS: não encontrada
- Afirmações fora do gabarito: nenhuma
- Cenários: 10; tabelas de decisão: 0
- Limites lidos do código: 1 de 1
  - CEN-FAT-002-L1 (RN-FAT-008, limite 2): CT-0001; valores testados: 1 (CT-0001), 2 (CT-0002)
    - Divergentes: CT-0008: 12; CT-0009: 12
- A confirmar: 1 de 1
  - O máximo de parcelas vem do parâmetro MAX_PARCELAS (6 na carga inicial); qual é o valor em produção?: CT-0006
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
