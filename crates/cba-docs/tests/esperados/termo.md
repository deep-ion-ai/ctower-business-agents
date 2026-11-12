---
tipo: termo
termo: Carência
sinonimos:
- prazo de tolerância
- dias de carência
area: cobranca
origem: analises/A-0001.md
---

# Carência

- **Sinônimos:** prazo de tolerância, dias de carência
- **Área:** cobranca
- **Origem:** [analises/A-0001.md](../analises/A-0001.md)

## Definição de negócio

Dias corridos após o vencimento em que o atraso não gera multa nem juros.

## Onde vive no código

- **[confirmado]** Parâmetro `DIAS_CARENCIA` na tabela `PARAMETRO`.[^1]
- **[confirmado]** Pacote de cobrança, nos cálculos de multa e de juros.[^2][^3]

## Evidências

[^1]: `db/02_parametros.sql:12-13`
[^2]: `db/03_pkg_cobranca.sql:66-69`
[^3]: `db/03_pkg_cobranca.sql:90-92`
