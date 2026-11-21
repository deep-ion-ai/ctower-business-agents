---
tipo: regra
id: RN-COB-003
titulo: Multa arredondada em centavos
area: cobranca
status: confirmada
evidencias:
- arquivo: db/pkg_cobranca.sql
  linhas: 25-25
  hash: ee
  commit: c0ffee1234567890
origem: analises/A-0001.md
---

# RN-COB-003 · Multa arredondada em centavos

- **Identificador:** RN-COB-003
- **Área:** cobranca
- **Status:** confirmada
- **Origem:** [analises/A-0001.md](../analises/A-0001.md)

## Enunciado

O valor da multa é arredondado para duas casas decimais.[^1]

## Evidências

[^1]: `db/pkg_cobranca.sql:25-25` · hash `ee` · commit `c0ffee1234567890`
