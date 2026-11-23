---
tipo: regra
id: RN-COB-001
titulo: Multa de 2% sobre a fatura paga depois do vencimento
area: cobranca
status: confirmada
evidencias:
- arquivo: db/pkg_cobranca.sql
  linhas: 12-14
  hash: aa
  commit: c0ffee1234567890
origem: analises/A-0001.md
funcionalidades:
- F-COB-001
- F-CAN-001
---

# RN-COB-001 · Multa de 2% sobre a fatura paga depois do vencimento

- **Identificador:** RN-COB-001
- **Área:** cobranca
- **Status:** confirmada
- **Origem:** [analises/A-0001.md](../analises/A-0001.md)

## Enunciado

A fatura paga depois do vencimento recebe multa de 2% sobre o valor. Veja a RN-COB-002 e a [análise](../analises/A-0001.md).[^1]

## Evidências

[^1]: `db/pkg_cobranca.sql:12-14` · hash `aa` · commit `c0ffee1234567890`
