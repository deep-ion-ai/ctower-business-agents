[Início](README.md) › Glossário

# Glossário

2 termos do negócio, em ordem alfabética, com as regras e funcionalidades que os usam.

[F](#letra-f) · [M](#letra-m)

<a id="letra-f"></a>
## F

<!-- cba:termo id=fatura {"caminho":"glossario/fatura.md","frontmatter":{"area":"faturamento","sinonimos":[],"termo":"Fatura","tipo":"termo"}} -->
<a id="fatura"></a>
### Fatura

*Área:* Faturamento

Documento de cobrança emitido ao cliente todo mês.

**Onde vive no código**

- tabela FATURA

* **Usado em:** [F-CAN-001 · Cancelar fatura](funcionalidades/cancelamento/f-can-001-cancelar-fatura.md) · [F-COB-001 · Cobrar multa por atraso](funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md) · [RN-COB-001](funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-001) · [RN-COB-002](funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-002) · [RN-FAT-001](regras/faturamento.md#rn-fat-001)
<!-- /cba -->

<a id="letra-m"></a>
## M

<!-- cba:termo id=multa {"caminho":"glossario/multa.md","frontmatter":{"area":"cobranca","funcionalidades":["F-COB-001"],"origem":"analises/A-0001.md","sinonimos":["Multa por atraso","Penalidade"],"termo":"Multa","tipo":"termo"}} -->
<a id="multa"></a>
### Multa

*Também chamado de:* Multa por atraso, Penalidade · *Área:* Cobrança

Valor cobrado a mais quando a fatura é paga depois do vencimento. Calculada pela [RN-COB-001](funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-001).

**Onde vive no código**

- coluna FATURA.VL_MULTA

* **Evidências no código:** [`pkg_cobranca.sql:12-14`](<https://github.com/exemplo/faturamento/blob/c0ffee1234567890/db/pkg_cobranca.sql#L12-L14> "db/pkg_cobranca.sql")
* **Usado em:** [F-COB-001 · Cobrar multa por atraso](funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md) · [RN-COB-001](funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-001) · [RN-COB-002](funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-002) · [RN-COB-003](funcionalidades/cobranca/f-cob-001-cobrar-multa-por-atraso.md#rn-cob-003) · [RN-FAT-001](regras/faturamento.md#rn-fat-001)
* **Origem:** [A-0001 · Como a multa por atraso é calculada?](analises/A-0001.md)
<!-- /cba -->
