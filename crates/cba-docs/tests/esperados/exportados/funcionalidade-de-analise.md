# Cálculo de encargos por atraso

Documento negocial da funcionalidade · Status: rascunho

## Objetivo

Calcular multa e juros de uma fatura vencida numa data de referência.

## Atores e telas

_Nada registrado._

## Fluxo principal

_Nada registrado._

## Fluxos alternativos

_Nada registrado._

## Exceções

_Nada registrado._

## Regras de negócio

- **[confirmado]** Multa por atraso vem de parâmetro (RN-FAT-001): Passada a carência, a fatura em atraso recebe multa única de PERC_MULTA_ATRASO por cento (hoje 2%) sobre o valor original, lida da tabela PARAMETRO e arredondada em 2 casas.[^1][^2]
- **[confirmado]** Juros de mora desde o primeiro dia de atraso (RN-FAT-002): Passada a carência, os juros de mora são simples, de PERC_JUROS_DIA por cento ao dia (hoje 0,033%) sobre o valor original, e contam desde o primeiro dia após o vencimento, não só a partir do fim da carência.[^3][^4][^5]
- **[confirmado]** Carência de 3 dias sem encargos (RN-FAT-003): Atraso de até DIAS_CARENCIA dias corridos (hoje 3) não gera multa nem juros. A partir do quarto dia, multa e juros são cobrados.[^6][^7][^8]
- **[confirmado]** Cliente com isenção cadastrada não paga encargos (RN-FAT-004): Cliente com CLIENTE.ISENTO_ENCARGOS = 'S' não paga multa nem juros, seja qual for o tipo de cliente.[^9][^10][^11][^12]
- **[confirmado]** Órgão público não paga multa (RN-FAT-005): Cliente do tipo órgão público (PUB) não paga multa por atraso; paga apenas os juros de mora. A exceção é aplicada no backend.[^13]
- **[confirmado]** Teto de encargos por tipo de cliente (RN-FAT-006): A soma de multa e juros é limitada a um percentual do valor original: 10% para pessoa física, 20% para pessoa jurídica e 5% para órgão público.[^14][^15][^16]

## Dados lidos e gravados

_Nada registrado._

## Integrações

_Nada registrado._

## Perguntas em aberto

_Nenhuma pergunta em aberto._

## Sugestões de alteração

_Nenhuma sugestão registrada._

## Evidências

[^1]: `db/03_pkg_cobranca.sql:71-72`
[^2]: `db/02_parametros.sql:6-7`
[^3]: `db/03_pkg_cobranca.sql:94-96`
[^4]: `db/03_pkg_cobranca.sql:47`
[^5]: `db/02_parametros.sql:9-10`
[^6]: `db/03_pkg_cobranca.sql:66-69`
[^7]: `db/03_pkg_cobranca.sql:90-92`
[^8]: `db/02_parametros.sql:12-13`
[^9]: `db/03_pkg_cobranca.sql:59-62`
[^10]: `db/03_pkg_cobranca.sql:84-86`
[^11]: `db/01_tabelas.sql:13-14`
[^12]: `cobol/CALCOBR.cbl:69`
[^13]: `backend/src/main/java/exemplo/faturamento/EncargosService.java:31-34`
[^14]: `backend/src/main/java/exemplo/faturamento/TetoEncargosRegra.java:17-22`
[^15]: `backend/src/main/java/exemplo/faturamento/TetoEncargosRegra.java:28-32`
[^16]: `backend/src/main/java/exemplo/faturamento/EncargosService.java:36-37`

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

### [3] `db/03_pkg_cobranca.sql:94-96`

```sql
94 |         -- Passada a carencia, os juros contam desde o primeiro dia de atraso
95 |         -- (juros simples sobre o valor original)
96 |         RETURN ROUND(V_FATURA.VL_ORIGINAL * FN_PARAMETRO('PERC_JUROS_DIA') / 100 * V_DIAS, 2);
```

### [4] `db/03_pkg_cobranca.sql:47`

```sql
47 |         RETURN TRUNC(P_DT_REFERENCIA) - TRUNC(P_DT_VENCIMENTO);
```

### [5] `db/02_parametros.sql:9-10`

```sql
 9 | INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
10 | VALUES ('PERC_JUROS_DIA', 0.0330, 'Percentual de juros de mora por dia de atraso');
```

### [6] `db/03_pkg_cobranca.sql:66-69`

```sql
66 |         -- Dentro da carencia nao ha multa
67 |         IF V_DIAS <= FN_PARAMETRO('DIAS_CARENCIA') THEN
68 |             RETURN 0;
69 |         END IF;
```

### [7] `db/03_pkg_cobranca.sql:90-92`

```sql
90 |         IF V_DIAS <= FN_PARAMETRO('DIAS_CARENCIA') THEN
91 |             RETURN 0;
92 |         END IF;
```

### [8] `db/02_parametros.sql:12-13`

```sql
12 | INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
13 | VALUES ('DIAS_CARENCIA', 3.0000, 'Dias corridos apos o vencimento sem cobranca de encargos');
```

### [9] `db/03_pkg_cobranca.sql:59-62`

```sql
59 |         -- Cliente com isencao cadastrada nao paga multa nem juros
60 |         IF V_CLIENTE.ISENTO_ENCARGOS = 'S' THEN
61 |             RETURN 0;
62 |         END IF;
```

### [10] `db/03_pkg_cobranca.sql:84-86`

```sql
84 |         IF V_CLIENTE.ISENTO_ENCARGOS = 'S' THEN
85 |             RETURN 0;
86 |         END IF;
```

### [11] `db/01_tabelas.sql:13-14`

```sql
13 |     -- S = cliente isento de encargos por atraso (cadastro manual)
14 |     ISENTO_ENCARGOS   CHAR(1)        DEFAULT 'N' NOT NULL,
```

### [12] `cobol/CALCOBR.cbl:69`

```cobol
69 |            IF FAT-ISENTO NOT = 'S'
```

### [13] `backend/src/main/java/exemplo/faturamento/EncargosService.java:31-34`

```java
31 |         // Órgão público não paga multa por atraso, apenas juros
32 |         if (fatura.tipoCliente() == TipoCliente.PUB) {
33 |             multa = BigDecimal.ZERO;
34 |         }
```

### [14] `backend/src/main/java/exemplo/faturamento/TetoEncargosRegra.java:17-22`

```java
17 |             case PF:
18 |                 return new BigDecimal("10.00");
19 |             case PJ:
20 |                 return new BigDecimal("20.00");
21 |             case PUB:
22 |                 return new BigDecimal("5.00");
```

### [15] `backend/src/main/java/exemplo/faturamento/TetoEncargosRegra.java:28-32`

```java
28 |     public static BigDecimal limitar(BigDecimal encargos, BigDecimal valorOriginal, TipoCliente tipo) {
29 |         BigDecimal teto = valorOriginal
30 |                 .multiply(percentualTeto(tipo))
31 |                 .divide(new BigDecimal("100"), 2, RoundingMode.HALF_UP);
32 |         return encargos.min(teto);
```

### [16] `backend/src/main/java/exemplo/faturamento/EncargosService.java:36-37`

```java
36 |         BigDecimal total = TetoEncargosRegra.limitar(
37 |                 multa.add(juros), fatura.valorOriginal(), fatura.tipoCliente());
```
