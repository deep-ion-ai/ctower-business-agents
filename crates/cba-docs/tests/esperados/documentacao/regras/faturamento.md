[Início](../README.md) › Regras sem funcionalidade

# Regras de negócio · Faturamento

1 regra confirmada da área Faturamento que ainda não está ligada a uma funcionalidade aceita do mapa.

## Índice

- [RN-FAT-001 · Fatura emitida no dia 5](#rn-fat-001)

## Regras

<!-- cba:regra id=RN-FAT-001 {"caminho":"regras/RN-FAT-001.md","frontmatter":{"area":"faturamento","evidencias":[{"arquivo":"jobs/emitir.sh","commit":"c0ffee1234567890","hash":"gg","linhas":"1-3"}],"id":"RN-FAT-001","status":"confirmada","tipo":"regra","titulo":"Fatura emitida no dia 5"}} -->
<a id="rn-fat-001"></a>
### RN-FAT-001 · Fatura emitida no dia 5

A fatura é emitida todo dia 5; a multa ([RN-COB-001](../funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-001)) conta a partir do vencimento.

* **Evidências no código:** [`emitir.sh:1-3`](<https://github.com/exemplo/faturamento/blob/c0ffee1234567890/jobs/emitir.sh#L1-L3> "jobs/emitir.sh")
<!-- /cba -->
