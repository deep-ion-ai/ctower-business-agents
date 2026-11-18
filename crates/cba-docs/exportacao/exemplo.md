---
tipo: analise-impacto
id: A-0000
titulo: Documento de exemplo
projeto: projeto-exemplo
status: rascunho
---

# A-0000 · Documento de exemplo

Análise de impacto · Projeto `projeto-exemplo` · Status: rascunho · Criada em 2026-01-01 · Analista: Ana Exemplo

## Demanda como entendida

Este é um documento de exemplo para a prévia da exportação em HTML. Ele mostra **negrito**, _itálico_, `código em linha` e um [link externo](https://exemplo.com).

## Como é hoje

- **[confirmado]** O limite de parcelas é lido da tabela de parâmetros.[^1]
- **[inferido]** A tela oferece mais parcelas do que o serviço aceita.[^2]
- **[a confirmar]** O limite vale também para órgão público.[^1]

> Citação: o cliente pediu parcelamento em até 12 vezes.

| Campo | Hoje | Proposto |
|:--|--:|--:|
| Parcelas | 6 | 12 |
| Valor mínimo | 100,00 | 50,00 |

## Perguntas para o solicitante

1. O limite de 12 parcelas vale para todos os clientes?
2. Há valor mínimo por parcela?

## Evidências

[^1]: `db/parametros.sql:3-4` · hash `exemplo`
[^2]: `telas/parcelamento.ts:10`
