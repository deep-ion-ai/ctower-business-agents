---
tipo: caso-de-uso
titulo: Cancelamento de fatura
area: cancelamento
tema: cancelamento-de-fatura
status: rascunho
origem: null
---

# Caso de uso: Cancelamento de fatura

## Identificação

- **Área:** cancelamento
- **Tema:** cancelamento-de-fatura
- **Status:** rascunho
- **Origem:** não informada

## Objetivo

Cancelar uma fatura emitida, registrando motivo e responsável.

## Gatilho

O atendente pede o cancelamento de uma fatura emitida.

## Pré-condições

- **[confirmado]** Cancelamento só de fatura aberta e sem pagamento (RN-FAT-012): Só fatura com status ABERTA e sem nenhum valor pago pode ser cancelada. Fatura com pagamento parcial não pode ser cancelada.[^1][^2]

## Fluxo principal

1. O atendente informa a fatura e o motivo.
2. O projeto confere as regras.
3. A fatura é cancelada com o responsável registrado.

## Fluxos alternativos

- **[confirmado]** Cancelamento após 30 dias da emissão exige supervisor (RN-FAT-014): Fatura emitida há mais de 30 dias só pode ser cancelada por usuário com perfil SUPERVISOR. A regra existe apenas na tela de retaguarda; o banco não a verifica.[^3][^4]

## Exceções

- **[confirmado]** Motivo do cancelamento com ao menos 10 caracteres (RN-FAT-013): O cancelamento exige motivo com ao menos 10 caracteres (desprezando espaços nas pontas). O motivo, a data e o usuário ficam gravados na fatura e no log.[^5][^6][^7]

## Regras de negócio

- **[confirmado]** Cancelamento só de fatura aberta e sem pagamento (RN-FAT-012): Só fatura com status ABERTA e sem nenhum valor pago pode ser cancelada. Fatura com pagamento parcial não pode ser cancelada.[^1][^2]
- **[confirmado]** Motivo do cancelamento com ao menos 10 caracteres (RN-FAT-013): O cancelamento exige motivo com ao menos 10 caracteres (desprezando espaços nas pontas). O motivo, a data e o usuário ficam gravados na fatura e no log.[^5][^6][^7]
- **[confirmado]** Cancelamento após 30 dias da emissão exige supervisor (RN-FAT-014): Fatura emitida há mais de 30 dias só pode ser cancelada por usuário com perfil SUPERVISOR. A regra existe apenas na tela de retaguarda; o banco não a verifica.[^3][^4]

## Evidências

[^1]: `db/03_pkg_cobranca.sql:111-113`
[^2]: `db/03_pkg_cobranca.sql:115-117`
[^3]: `delphi/UFrmCancelamento.pas:69-75`
[^4]: `delphi/UFrmCancelamento.pas:41-42`
[^5]: `db/03_pkg_cobranca.sql:119-121`
[^6]: `db/03_pkg_cobranca.sql:123-131`
[^7]: `delphi/UFrmCancelamento.pas:63-67`
