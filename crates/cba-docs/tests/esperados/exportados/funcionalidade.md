# Parcelamento de fatura

Documento negocial da funcionalidade · Status: em revisão

## Objetivo

Dividir uma fatura em aberto em parcelas mensais.

## Atores e telas

- **[confirmado]** Atendente, pela tela de parcelamento.[^1]

## Fluxo principal

_Nada registrado._

## Fluxos alternativos

_Nada registrado._

## Exceções

- **[confirmado]** Tela oferece até 12 parcelas; backend aceita até 6 (INC-FAT-001): A tela de parcelamento valida e oferece de 2 a 12 parcelas (constante fixa no código), enquanto o backend aceita no máximo MAX_PARCELAS (hoje 6). Pedidos de 7 a 12 parcelas passam pela tela e são recusados pelo backend.[^1][^2]
- **[confirmado]** Tela calcula a parcela mínima sobre o valor original (INC-FAT-003): A tela confere o valor mínimo da parcela dividindo só o valor da fatura, enquanto o backend divide o valor original mais os encargos. Uma fatura em atraso pode ser recusada pela tela e aceita pelo backend.[^3][^4]

## Regras de negócio

- **[confirmado]** Parcelamento só para pessoa jurídica e órgão público (RN-FAT-007): Pessoa física não pode parcelar fatura. Pessoa jurídica e órgão público podem.[^5][^6]
- **[confirmado]** Quantidade de parcelas entre 2 e MAX_PARCELAS (RN-FAT-008): O parcelamento aceita de 2 até MAX_PARCELAS parcelas (hoje 6), valor lido da tabela PARAMETRO pelo backend.[^2][^7][^8]
- **[confirmado]** Valor mínimo da parcela, exceto órgão público (RN-FAT-009): Cada parcela deve valer ao menos VALOR_MIN_PARCELA (hoje R$ 50,00). Órgão público não tem valor mínimo de parcela.[^9][^10][^11]
- **[confirmado]** Só fatura aberta pode ser parcelada (RN-FAT-010): O parcelamento só é aceito para fatura com status ABERTA.[^12]
- **[confirmado]** Composição e vencimentos das parcelas (RN-FAT-011): O valor parcelado é o valor original mais os encargos (já com teto) na data do pedido. As parcelas são iguais, arredondadas para baixo em centavos; a diferença vai para a primeira parcela. Os vencimentos são a cada 30 dias a partir da data do pedido.[^13][^14][^15]

## Dados lidos e gravados

_Nada registrado._

## Integrações

_Nada registrado._

## Perguntas em aberto

1. Qual dos limites de parcelas (tela ou backend) é o correto?

## Sugestões de alteração

_Nenhuma sugestão registrada._

## Evidências

[^1]: `frontend/src/telas/parcelamento-form.ts:19`
[^2]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:34-35`
[^3]: `frontend/src/telas/parcelamento-form.ts:44`
[^4]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:42-45`
[^5]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:30-32`
[^6]: `frontend/src/telas/parcelamento-form.ts:25-30`
[^7]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:14`
[^8]: `db/02_parametros.sql:15-16`
[^9]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:47-53`
[^10]: `db/02_parametros.sql:18-19`
[^11]: `frontend/src/telas/parcelamento-form.ts:45`
[^12]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:26-28`
[^13]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:40-42`
[^14]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:55-61`
[^15]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:15`

## Apêndice: trechos citados

### [1] `frontend/src/telas/parcelamento-form.ts:19`

```typescript
19 | const MAX_PARCELAS = 12;
```

### [2] `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:34-35`

```java
34 |         int maxParcelas = parametros.inteiro(ParametroRepository.MAX_PARCELAS);
35 |         if (quantidade < MIN_PARCELAS || quantidade > maxParcelas) {
```

### [3] `frontend/src/telas/parcelamento-form.ts:44`

```typescript
44 |   const valorParcela = pedido.valorFatura / pedido.quantidadeParcelas;
```

### [4] `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:42-45`

```java
42 |         BigDecimal totalParcelado = fatura.valorOriginal().add(encargos.totalComTeto());
43 |
44 |         BigDecimal valorParcela = totalParcelado
45 |                 .divide(BigDecimal.valueOf(quantidade), 2, RoundingMode.DOWN);
```

### [5] `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:30-32`

```java
30 |         if (fatura.tipoCliente() == TipoCliente.PF) {
31 |             throw new RegraNegocioException("Parcelamento disponível apenas para pessoa jurídica e órgão público");
32 |         }
```

### [6] `frontend/src/telas/parcelamento-form.ts:25-30`

```typescript
25 |   if (pedido.tipoCliente === 'PF') {
26 |     erros.push({
27 |       campo: 'tipoCliente',
28 |       mensagem: 'Parcelamento disponível apenas para empresas e órgãos públicos.',
29 |     });
30 |     return erros;
```

### [7] `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:14`

```java
14 |     private static final int MIN_PARCELAS = 2;
```

### [8] `db/02_parametros.sql:15-16`

```sql
15 | INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
16 | VALUES ('MAX_PARCELAS', 6.0000, 'Quantidade maxima de parcelas no parcelamento de fatura');
```

### [9] `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:47-53`

```java
47 |         // Órgão público não tem valor mínimo de parcela
48 |         if (fatura.tipoCliente() != TipoCliente.PUB) {
49 |             BigDecimal minimo = parametros.valor(ParametroRepository.VALOR_MIN_PARCELA);
50 |             if (valorParcela.compareTo(minimo) < 0) {
51 |                 throw new RegraNegocioException("Valor da parcela abaixo do mínimo de " + minimo);
52 |             }
53 |         }
```

### [10] `db/02_parametros.sql:18-19`

```sql
18 | INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
19 | VALUES ('VALOR_MIN_PARCELA', 50.0000, 'Valor minimo de cada parcela, em reais');
```

### [11] `frontend/src/telas/parcelamento-form.ts:45`

```typescript
45 |   if (pedido.tipoCliente !== 'PUB' && valorParcela < VALOR_MINIMO_PARCELA) {
```

### [12] `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:26-28`

```java
26 |         if (!fatura.isAberta()) {
27 |             throw new RegraNegocioException("Somente fatura ABERTA pode ser parcelada");
28 |         }
```

### [13] `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:40-42`

```java
40 |         // Valor parcelado = valor original + encargos na data do pedido
41 |         EncargosService.Encargos encargos = encargosService.calcular(fatura, dataPedido);
42 |         BigDecimal totalParcelado = fatura.valorOriginal().add(encargos.totalComTeto());
```

### [14] `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:55-61`

```java
55 |         // A diferença de centavos do arredondamento vai para a primeira parcela
56 |         BigDecimal residuo = totalParcelado.subtract(valorParcela.multiply(BigDecimal.valueOf(quantidade)));
57 |
58 |         List<Parcela> parcelas = new ArrayList<>();
59 |         for (int i = 1; i <= quantidade; i++) {
60 |             BigDecimal valor = (i == 1) ? valorParcela.add(residuo) : valorParcela;
61 |             LocalDate vencimento = dataPedido.plusDays((long) DIAS_ENTRE_PARCELAS * i);
```

### [15] `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:15`

```java
15 |     private static final int DIAS_ENTRE_PARCELAS = 30;
```
