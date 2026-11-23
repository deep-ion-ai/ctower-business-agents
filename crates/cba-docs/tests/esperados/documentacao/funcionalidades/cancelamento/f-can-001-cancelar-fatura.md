[Início](../../README.md) › Cancelamento

<a id="f-can-001"></a>
# F-CAN-001 · Cancelar fatura

Cancela uma fatura emitida | ainda não paga.

- **Área:** Cancelamento
- **Atores:** Atendente

## Como funciona

<!-- cba:funcionalidade id=cancelamento/cancelar-fatura/negocial {"caminho":"funcionalidades/cancelamento/cancelar-fatura/negocial.md","frontmatter":{"area":"cancelamento","funcionalidade":"F-CAN-001","origem":"analises/A-0002.md","status":"aprovada","tema":"cancelar-fatura","tipo":"funcionalidade","titulo":"Cancelar fatura"}} -->
<a id="cancelamento-cancelar-fatura-negocial"></a>

### Objetivo

- **[confirmado]** O atendente cancela a fatura que ainda não foi paga; a multa da [RN-COB-001](../cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-001) deixa de valer.[^cancelamento-cancelar-fatura-negocial-1]

### Exceções

- **[confirmado]** Fatura paga não pode ser cancelada.[^cancelamento-cancelar-fatura-negocial-2]

[^cancelamento-cancelar-fatura-negocial-1]: [`src/cancelar.ts:5-9`](<https://github.com/exemplo/faturamento/blob/c0ffee1234567890/src/cancelar.ts#L5-L9>)
[^cancelamento-cancelar-fatura-negocial-2]: [`src/cancelar.ts:12`](<https://github.com/exemplo/faturamento/blob/c0ffee1234567890/src/cancelar.ts#L12>)

*Explicação aprovada na análise [A-0002](../../analises/A-0002.md).*
<!-- /cba -->

## Regras de negócio

Regras descritas em outra funcionalidade:

- [RN-COB-001 · Multa de 2% sobre a fatura paga depois do vencimento](../cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-001)

## Termos usados

- [Fatura](../../glossario.md#fatura): Documento de cobrança emitido ao cliente todo mês.
- [Multa](../../glossario.md#multa): Valor cobrado a mais quando a fatura é paga depois do vencimento.

## Análises

- [A-0002 · Explicar o cancelamento de fatura](../../analises/A-0002.md) (Explicação de funcionalidade, 2026-11-12)
