---
tipo: sugestao
id: S-0001
titulo: Ler a multa do parâmetro no lote de cobrança
sistema: faturamento-exemplo
status: proposta
analise: A-0004
arquivo: cobol/CALCOBR.cbl
criado: 2026-11-11
---

# S-0001 · Ler a multa do parâmetro no lote de cobrança

Sugestão de alteração · Sistema `faturamento-exemplo` · Status: proposta · Análise: [A-0004](../analises/A-0004.md) · Arquivo: `cobol/CALCOBR.cbl` · Criada em 2026-11-11

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
