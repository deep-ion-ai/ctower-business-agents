<!-- Gerado pelo ctower-business-agents a partir da base aprovada. Não edite à mão: edite na base, pelo app, e publique de novo. -->
# Faturamento Exemplo · documentação de negócio

O que o sistema faz, em linguagem de negócio: funcionalidades, regras de negócio, termos e análises aprovados pelos analistas, com links para o trecho do código que sustenta cada afirmação.

> **Gerada pelo app.** Para corrigir ou completar, edite na base do ctower-business-agents e publique de novo: o que for mudado direto aqui volta ao que está na base na próxima publicação.

**Código descrito:** `main` @ `c0ffee123456` · **Publicada em:** 2026-11-20

2 funcionalidades · 5 regras de negócio · 2 termos · 2 análises

## Por onde começar

- [Funcionalidades por área](#funcionalidades-por-area): o que o sistema faz, com as regras de cada funcionalidade.
- [Regras sem funcionalidade](#regras-sem-funcionalidade): 2 regras ainda não ligadas a uma funcionalidade do mapa.
- [Glossário](glossario.md): os termos do negócio e onde aparecem.
- [Análises](analises/README.md): as perguntas e análises que deram origem a esta documentação.

<a id="funcionalidades-por-area"></a>
## Funcionalidades por área

### Cancelamento

| Funcionalidade | O que faz | Regras |
| --- | --- | ---: |
| [F-CAN-001 · Cancelar fatura](funcionalidades/cancelamento/f-can-001-cancelar-fatura.md) | Cancela uma fatura emitida \| ainda não paga. | 1 |

### Cobrança

| Funcionalidade | O que faz | Regras |
| --- | --- | ---: |
| [F-COB-001 · Cobrar multa por atraso](funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md) | Calcula e lança a multa da fatura paga depois do vencimento. | 3 |

Mais 1 regra desta área sem funcionalidade: [Regras · Cobrança](regras/cobranca.md).

<a id="regras-sem-funcionalidade"></a>
## Regras sem funcionalidade

- [Cobrança](regras/cobranca.md): 1 regra
- [Faturamento](regras/faturamento.md): 1 regra

O mapa tem mais 1 funcionalidade ainda sem a revisão do analista; ela entra aqui quando for aceita.
