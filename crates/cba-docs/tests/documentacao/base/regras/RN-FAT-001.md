---
tipo: regra
id: RN-FAT-001
titulo: Fatura emitida no dia 5
area: faturamento
status: confirmada
evidencias:
- arquivo: jobs/emitir.sh
  linhas: 1-3
  hash: gg
  commit: c0ffee1234567890
---

# RN-FAT-001 · Fatura emitida no dia 5

- **Identificador:** RN-FAT-001
- **Área:** faturamento
- **Status:** confirmada
- **Origem:** —

## Enunciado

A fatura é emitida todo dia 5; a multa (RN-COB-001) conta a partir do vencimento.[^1]

## Evidências

[^1]: `jobs/emitir.sh:1-3` · hash `gg` · commit `c0ffee1234567890`
