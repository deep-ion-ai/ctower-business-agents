# RN-FAT-001 · Multa por atraso vem de parâmetro

- **Identificador:** RN-FAT-001
- **Área:** cobranca
- **Status:** confirmada
- **Origem:** analises/A-0001.md

## Enunciado

Passada a carência, a fatura em atraso recebe multa única de PERC_MULTA_ATRASO por cento (hoje 2%) sobre o valor original, lida da tabela PARAMETRO e arredondada em 2 casas.[^1][^2]

## Evidências

[^1]: `db/03_pkg_cobranca.sql:71-72` · hash `hash-ficticio-1` · commit `1a2b3c4d`
[^2]: `db/02_parametros.sql:6-7` · hash `hash-ficticio-2` · commit `1a2b3c4d`

## Apêndice: trechos citados

### [1] `db/03_pkg_cobranca.sql:71-72`

```sql
71 |         -- Multa unica, aplicada sobre o valor original da fatura
72 |         RETURN ROUND(V_FATURA.VL_ORIGINAL * FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2);
```

### [2] `db/02_parametros.sql:6-7`

```sql
6 | INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
7 | VALUES ('PERC_MULTA_ATRASO', 2.0000, 'Percentual de multa por atraso sobre o valor original');
```
