[Início](../../README.md) › Cobrança

<a id="f-cob-001"></a>
# F-COB-001 · Cobrar multa por atraso

Calcula e lança a multa da fatura paga depois do vencimento.

- **Área:** Cobrança
- **Atores:** Operador de cobrança, Rotina noturna
- **Dados gravados:** FATURA.VL\_MULTA
- **Relacionadas:** [F-CAN-001 · Cancelar fatura](../cancelamento/f-can-001-cancelar-fatura.md) · F-COB-002 · Parcelar fatura (ainda em revisão no mapa)

## Regras de negócio

<!-- cba:regra id=RN-COB-001 {"caminho":"regras/RN-COB-001.md","frontmatter":{"area":"cobranca","evidencias":[{"arquivo":"db/pkg_cobranca.sql","commit":"c0ffee1234567890","hash":"aa","linhas":"12-14"}],"funcionalidades":["F-COB-001","F-CAN-001"],"id":"RN-COB-001","origem":"analises/A-0001.md","status":"confirmada","tipo":"regra","titulo":"Multa de 2% sobre a fatura paga depois do vencimento"}} -->
<a id="rn-cob-001"></a>
### RN-COB-001 · Multa de 2% sobre a fatura paga depois do vencimento

A fatura paga depois do vencimento recebe multa de 2% sobre o valor. Veja a [RN-COB-002](#rn-cob-002) e a [análise](../../analises/A-0001.md).

* **Evidências no código:** [`pkg_cobranca.sql:12-14`](<https://github.com/exemplo/faturamento/blob/c0ffee1234567890/db/pkg_cobranca.sql#L12-L14> "db/pkg_cobranca.sql")
* **Também em:** [F-CAN-001 · Cancelar fatura](../cancelamento/f-can-001-cancelar-fatura.md)
* **Origem:** [A-0001 · Como a multa por atraso é calculada?](../../analises/A-0001.md)
<!-- /cba -->

<!-- cba:regra id=RN-COB-002 {"caminho":"regras/RN-COB-002.md","frontmatter":{"area":"cobranca","evidencias":[{"arquivo":"db/pkg_cobranca.sql","hash":"bb","linhas":"20-22"}],"funcionalidades":["F-COB-003"],"id":"RN-COB-002","origem":"analises/A-0001.md","status":"confirmada","tipo":"regra","titulo":"Sem multa até o vencimento"}} -->
<a id="rn-cob-002"></a>
### RN-COB-002 · Sem multa até o vencimento

Fatura paga até o vencimento não tem multa por atraso.

* **Evidências no código:** [`pkg_cobranca.sql:20-22`](<https://github.com/exemplo/faturamento/blob/c0ffee1234567890/db/pkg_cobranca.sql#L20-L22> "db/pkg_cobranca.sql")
* **Origem:** [A-0001 · Como a multa por atraso é calculada?](../../analises/A-0001.md)
<!-- /cba -->

<!-- cba:regra id=RN-COB-003 {"caminho":"regras/RN-COB-003.md","frontmatter":{"area":"cobranca","evidencias":[{"arquivo":"db/pkg_cobranca.sql","commit":"c0ffee1234567890","hash":"ee","linhas":"25-25"}],"id":"RN-COB-003","origem":"analises/A-0001.md","status":"confirmada","tipo":"regra","titulo":"Multa arredondada em centavos"}} -->
<a id="rn-cob-003"></a>
### RN-COB-003 · Multa arredondada em centavos

O valor da multa é arredondado para duas casas decimais.

* **Evidências no código:** [`pkg_cobranca.sql:25`](<https://github.com/exemplo/faturamento/blob/c0ffee1234567890/db/pkg_cobranca.sql#L25> "db/pkg_cobranca.sql")
* **Origem:** [A-0001 · Como a multa por atraso é calculada?](../../analises/A-0001.md)
<!-- /cba -->

## Termos usados

- [Fatura](../../glossario.md#fatura): Documento de cobrança emitido ao cliente todo mês.
- [Multa](../../glossario.md#multa): Valor cobrado a mais quando a fatura é paga depois do vencimento.

## Análises

- [A-0001 · Como a multa por atraso é calculada?](../../analises/A-0001.md) (Pergunta ao sistema, 2026-11-10)

## Pontos de entrada no código

- Rotina de banco `PKG_COBRANCA.PRC_MULTA`: [`pkg_cobranca.sql:10-30`](<https://github.com/exemplo/faturamento/blob/c0ffee1234567890/db/pkg_cobranca.sql#L10-L30> "db/pkg_cobranca.sql")
