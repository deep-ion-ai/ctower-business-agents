# S-0001 · Ler a multa do parâmetro no lote de cobrança

Sugestão de alteração · Sistema `faturamento-exemplo` · Status: proposta · Análise: A-0004 · Arquivo: `cobol/CALCOBR.cbl` · Criada em 2026-11-11

## Contexto

A demanda reduz a multa de 2% para 1% alterando o parâmetro PERC_MULTA_ATRASO.

## Trecho atual

```cobol
       01  WS-PERC-MULTA           PIC 9V9999     VALUE 2.0000.
```

[^1]

## Trecho proposto

```cobol
       01  WS-PERC-MULTA           PIC 9V9999.
      * Lido de PARAMETRO no início do lote.
```

## Motivo

O lote usa 2% fixo e continuaria cobrando a multa antiga.

## Impacto

- **[inferido]** O lote passa a seguir o parâmetro, como o banco e o backend.[^2]

## Exemplo

Fatura de R$ 1.000,00 com 10 dias de atraso: hoje o lote cobra R$ 20,00 de multa; com o parâmetro em 1%, cobraria R$ 10,00.

## Evidências

[^1]: `cobol/CALCOBR.cbl:38-40`
[^2]: `db/03_pkg_cobranca.sql:71-72`

## Apêndice: trechos citados

### [1] `cobol/CALCOBR.cbl:38-40`

```cobol
38 |        77  WS-PERC-MULTA           PIC 9V9999     VALUE 2.0000.
39 |        77  WS-PERC-JUROS-DIA       PIC 9V9999     VALUE 0.0330.
40 |        77  WS-DIAS-CARENCIA        PIC 9(03)      VALUE 3.
```

### [2] `db/03_pkg_cobranca.sql:71-72`

```sql
71 |         -- Multa unica, aplicada sobre o valor original da fatura
72 |         RETURN ROUND(V_FATURA.VL_ORIGINAL * FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2);
```
