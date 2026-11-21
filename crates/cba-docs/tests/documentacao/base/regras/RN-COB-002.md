---
tipo: regra
id: RN-COB-002
titulo: Sem multa até o vencimento
area: cobranca
status: confirmada
evidencias:
- arquivo: db/pkg_cobranca.sql
  linhas: 20-22
  hash: bb
origem: analises/A-0001.md
funcionalidades:
- F-COB-003
---

# RN-COB-002 · Sem multa até o vencimento

- **Identificador:** RN-COB-002
- **Área:** cobranca
- **Status:** confirmada
- **Origem:** [analises/A-0001.md](../analises/A-0001.md)

## Enunciado

Fatura paga até o vencimento não tem multa por atraso.[^1]

## Evidências

[^1]: `db/pkg_cobranca.sql:20-22` · hash `bb`
