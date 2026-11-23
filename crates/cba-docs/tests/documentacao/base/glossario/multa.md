---
tipo: termo
termo: Multa
sinonimos:
- Multa por atraso
- Penalidade
area: cobranca
origem: analises/A-0001.md
funcionalidades:
- F-COB-001
---

# Multa

- **Sinônimos:** Multa por atraso, Penalidade
- **Área:** cobranca
- **Origem:** [analises/A-0001.md](../analises/A-0001.md)

## Definição de negócio

Valor cobrado a mais quando a fatura é paga depois do vencimento. Calculada pela RN-COB-001.[^1]

## Onde vive no código

- coluna FATURA.VL_MULTA

## Evidências

[^1]: `db/pkg_cobranca.sql:12-14` · hash `aa` · commit `c0ffee1234567890`
