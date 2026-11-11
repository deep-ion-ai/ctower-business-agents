---
tipo: regra
id: RN-FAT-001
titulo: Multa por atraso vem de parâmetro
area: cobranca
status: confirmada
evidencias:
- arquivo: db/03_pkg_cobranca.sql
  linhas: 71-72
  hash: hash-ficticio-1
  commit: 1a2b3c4d
- arquivo: db/02_parametros.sql
  linhas: 6-7
  hash: hash-ficticio-2
  commit: 1a2b3c4d
origem: analises/A-0001.md
---

# RN-FAT-001 · Multa por atraso vem de parâmetro

- **Identificador:** RN-FAT-001
- **Área:** cobranca
- **Status:** confirmada
- **Origem:** [analises/A-0001.md](../analises/A-0001.md)

## Enunciado

Passada a carência, a fatura em atraso recebe multa única de PERC_MULTA_ATRASO por cento (hoje 2%) sobre o valor original, lida da tabela PARAMETRO e arredondada em 2 casas.[^1][^2]

## Evidências

[^1]: `db/03_pkg_cobranca.sql:71-72` · hash `hash-ficticio-1` · commit `1a2b3c4d`
[^2]: `db/02_parametros.sql:6-7` · hash `hash-ficticio-2` · commit `1a2b3c4d`
