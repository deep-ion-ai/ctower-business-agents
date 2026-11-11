# Carência

- **Sinônimos:** prazo de tolerância, dias de carência
- **Área:** cobranca
- **Origem:** analises/A-0001.md

## Definição de negócio

Dias corridos após o vencimento em que o atraso não gera multa nem juros.

## Onde vive no código

- **[confirmado]** Parâmetro `DIAS_CARENCIA` na tabela `PARAMETRO`.[^1]
- **[confirmado]** Pacote de cobrança, nos cálculos de multa e de juros.[^2][^3]

## Evidências

[^1]: `db/02_parametros.sql:12-13`
[^2]: `db/03_pkg_cobranca.sql:66-69`
[^3]: `db/03_pkg_cobranca.sql:90-92`

## Apêndice: trechos citados

### [1] `db/02_parametros.sql:12-13`

```sql
12 | INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
13 | VALUES ('DIAS_CARENCIA', 3.0000, 'Dias corridos apos o vencimento sem cobranca de encargos');
```

### [2] `db/03_pkg_cobranca.sql:66-69`

```sql
66 |         -- Dentro da carencia nao ha multa
67 |         IF V_DIAS <= FN_PARAMETRO('DIAS_CARENCIA') THEN
68 |             RETURN 0;
69 |         END IF;
```

### [3] `db/03_pkg_cobranca.sql:90-92`

```sql
90 |         IF V_DIAS <= FN_PARAMETRO('DIAS_CARENCIA') THEN
91 |             RETURN 0;
92 |         END IF;
```
