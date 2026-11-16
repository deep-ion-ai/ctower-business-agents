# A-0204 · Histórias do parcelamento para pessoa física

Histórias de usuário · Projeto `faturamento-exemplo` · Status: rascunho · Criadas em 2026-11-13 · Analista: Ana Exemplo · Origem: A-0193 · Código: `main` @ `1a2b3c4d`

## Pedido

Histórias de usuário com critérios de aceite a partir da análise A-0193 (Parcelamento para pessoa física).

## Histórias

### HU-0001 · Parcelar fatura de pessoa física em até 3 vezes

**Como** atendente do faturamento, **quero** parcelar a fatura em aberto de um cliente pessoa física em 2 ou 3 vezes, **para** reduzir a inadimplência das faturas de pessoa física.

**Situação:** aceita

**INVEST:** atende aos seis critérios (I, N, V, E, S, T).

#### Critérios de aceite

**CA-1**

- **Dado** uma pessoa física com uma fatura em aberto de R$ 300,00
- **Quando** ela pede o parcelamento em 3 vezes
- **Então** são geradas 3 parcelas de R$ 100,00, com vencimento a cada 30 dias

_Origem:_

- A-0193 (afirmação 4): Pessoa física não pode parcelar fatura: o backend recusa o pedido.
- RN-FAT-008: Quantidade de parcelas entre 2 e MAX_PARCELAS[^1][^2]

**CA-2** · _a confirmar: depende da pergunta A-0193/2_

- **Dado** uma pessoa física com uma fatura em aberto de R$ 250,00
- **Quando** ela pede 3 parcelas
- **Então** o pedido é recusado: a parcela ficaria abaixo de R$ 100,00

_Origem:_

- RN-FAT-009: Valor mínimo da parcela, exceto órgão público[^3]

### HU-0002 · Mostrar o parcelamento de pessoa física no relatório de recebíveis

**Como** gestor financeiro, **quero** ver as parcelas de pessoa física no relatório de recebíveis, **para** prever o caixa do mês.

**Situação:** sem decisão

**INVEST:** atende a I, N, V, E e T; não atende a:

- **S** (pequena): junta a mudança do relatório com a da tela; não cabe numa entrega

**Quebra proposta:** Separar em duas fatias: o relatório com as parcelas e o filtro por tipo de cliente, que depende da primeira.

#### Critérios de aceite

**CA-1**

- **Dado** uma fatura de pessoa física de R$ 300,00 parcelada em 3 vezes
- **Quando** o relatório de recebíveis de março é emitido
- **Então** aparecem 3 recebíveis de R$ 100,00, um em cada mês

_Origem:_

- A-0193 (afirmação 4): Pessoa física não pode parcelar fatura: o backend recusa o pedido.[^1]

## Rastreabilidade

| História | Critério | Afirmações e regras | Observação |
|---|---|---|---|
| HU-0001 | CA-1 | A-0193/4, RN-FAT-008 |  |
| HU-0001 | CA-2 | RN-FAT-009 | a confirmar (A-0193/2) |
| HU-0002 | CA-1 | A-0193/4 |  |

## Perguntas em aberto

1. O relatório de recebíveis deve separar as parcelas por tipo de cliente?

## Evidências

[^1]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:30-32` · hash `9f2c1e7a`
[^2]: `db/02_parametros.sql:15-16` · hash `9f2c1e7a`
[^3]: `db/02_parametros.sql:18-19` · hash `9f2c1e7a`

## Apêndice: trechos citados

### [1] `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:30-32`

```java
30 |         if (fatura.tipoCliente() == TipoCliente.PF) {
31 |             throw new RegraNegocioException("Parcelamento disponível apenas para pessoa jurídica e órgão público");
32 |         }
```

### [2] `db/02_parametros.sql:15-16`

```sql
15 | INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
16 | VALUES ('MAX_PARCELAS', 6.0000, 'Quantidade maxima de parcelas no parcelamento de fatura');
```

### [3] `db/02_parametros.sql:18-19`

```sql
18 | INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
19 | VALUES ('VALOR_MIN_PARCELA', 50.0000, 'Valor minimo de cada parcela, em reais');
```
