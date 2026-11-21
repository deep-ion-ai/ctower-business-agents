---
tipo: regra
id: RN-COB-004
titulo: Parcela mínima de R$ 50
area: cobranca
status: confirmada
evidencias:
- arquivo: src/parcelar.ts
  linhas: 3-4
  hash: ff
  commit: c0ffee1234567890
funcionalidades:
- F-COB-002
---

# RN-COB-004 · Parcela mínima de R$ 50

- **Identificador:** RN-COB-004
- **Área:** cobranca
- **Status:** confirmada
- **Origem:** —

## Enunciado

Nenhuma parcela pode ficar abaixo de R$ 50,00.[^1]

## Evidências

[^1]: `src/parcelar.ts:3-4` · hash `ff` · commit `c0ffee1234567890`
