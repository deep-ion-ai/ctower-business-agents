# R-0001 · Ata da reunião de 2026-11-12

## Identificação

- **Data:** 2026-11-12
- **Duração:** 00:47:10
- **Demanda:** D-0001
- **Projeto:** `faturamento-exemplo`
- **Status:** rascunho

## Participantes e papéis

- Maria Exemplo · PO
- João Fictício · usuário-chave
- Ana Exemplo · papel a definir

## Objetivo

Entender o pedido de parcelamento da fatura para pessoa jurídica.

## Resumo

O cliente quer parcelar faturas de pessoa jurídica em até 12 vezes.

Hoje o sistema limita o parcelamento a 6 parcelas.

## Decisões

- **D-1** O parcelamento vale só para pessoa jurídica. — _Maria Exemplo, 00:12:34_

## Necessidades e requisitos levantados

- **N-1** Parcelar a fatura de pessoa jurídica em até 12 vezes. — _João Fictício, 00:05:02; Maria Exemplo, 00:06:40_[^1]

## Regras de negócio citadas

- **R-1** Cada parcela tem valor mínimo de R$ 50,00. — _João Fictício, 00:21:15_

## Pendências

- **P-1** Enviar a lista de clientes PJ que pediram parcelamento. · responsável: Maria Exemplo · prazo: 2026-11-20 — _Maria Exemplo, 00:40:03_

## Dúvidas em aberto

- **DV-1** O limite de 12 parcelas vale também para órgão público? — _Ana Exemplo, 00:33:48_

## Próximos passos

- **PP-1** Analisar o impacto de aumentar o limite de parcelas. (sobre N-1, D-1) — _sem fala citada_

## Referências às falas

- **D-1** · Maria Exemplo, 00:12:34: “fica só para PJ por enquanto” _(conferida)_
- **N-1** · João Fictício, 00:05:02: “a gente precisa de 12 parcelas” _(conferida)_
- **N-1** · Maria Exemplo, 00:06:40: “12 é o que o comercial prometeu” _(conferida)_
- **R-1** · João Fictício, 00:21:15: “parcela abaixo de 50 reais não pode” _(não confere)_
- **P-1** · Maria Exemplo, 00:40:03: “te mando a lista até sexta” _(conferida)_
- **DV-1** · Ana Exemplo, 00:33:48: “e prefeitura, entra?” _(conferida)_

## Evidências

[^1]: `db/02_parametros.sql:15-16`

## Apêndice: trechos citados

### [1] `db/02_parametros.sql:15-16`

```sql
15 | INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
16 | VALUES ('MAX_PARCELAS', 6.0000, 'Quantidade maxima de parcelas no parcelamento de fatura');
```
