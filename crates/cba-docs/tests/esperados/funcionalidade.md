---
tipo: funcionalidade
titulo: Parcelamento de fatura
area: parcelamento
tema: parcelamento-de-fatura
status: em-revisao
origem: analises/A-0003.md
---

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
