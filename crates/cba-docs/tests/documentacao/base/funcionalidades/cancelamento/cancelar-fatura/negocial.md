---
tipo: funcionalidade
titulo: Cancelar fatura
area: cancelamento
tema: cancelar-fatura
status: aprovada
origem: analises/A-0002.md
funcionalidade: F-CAN-001
---

# Cancelar fatura

Documento negocial da funcionalidade · Status: aprovada

## Objetivo

- **[confirmado]** O atendente cancela a fatura que ainda não foi paga; a multa da RN-COB-001 deixa de valer.[^1]

## Exceções

- **[confirmado]** Fatura paga não pode ser cancelada.[^2]

## Evidências

[^1]: `src/cancelar.ts:5-9` · hash `cc` · commit `c0ffee1234567890`
[^2]: `src/cancelar.ts:12` · hash `dd` · commit `c0ffee1234567890`
