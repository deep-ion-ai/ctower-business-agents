---
tipo: pergunta
id: A-0001
titulo: Cálculo da multa por atraso
projeto: faturamento-exemplo
demanda:
  origem: texto
  chave: null
status: rascunho
abrangencia: null
base:
  branch: develop
  commit: 1a2b3c4d
criado: 2026-11-11
analista: Ana Exemplo
---

# A-0001 · Cálculo da multa por atraso

Pergunta ao sistema · Projeto `faturamento-exemplo` · Status: rascunho · Criada em 2026-11-11 · Analista: Ana Exemplo · Código: `develop` @ `1a2b3c4d`

## Pergunta

Como é calculada a multa por atraso de pagamento?

## Resposta

Multa única de 2% (parâmetro PERC_MULTA_ATRASO) sobre o valor original, só depois de 3 dias de carência. Cliente com isenção cadastrada não paga; órgão público não paga multa, só juros. Multa e juros somados respeitam o teto por tipo de cliente (PF 10%, PJ 20%, PUB 5%). Exemplo: fatura de R$ 1.000,00 de pessoa jurídica paga com 10 dias de atraso tem multa de R$ 20,00 e juros de R$ 3,30 (0,033% x 10 dias), total de R$ 23,30, abaixo do teto de R$ 200,00. Ponto em aberto: o lote de cobrança calcula a multa com valor fixo.

## Regras encontradas

- **[confirmado]** Multa por atraso vem de parâmetro (RN-FAT-001): Passada a carência, a fatura em atraso recebe multa única de PERC_MULTA_ATRASO por cento (hoje 2%) sobre o valor original, lida da tabela PARAMETRO e arredondada em 2 casas.[^1][^2]
- **[confirmado]** Juros de mora desde o primeiro dia de atraso (RN-FAT-002): Passada a carência, os juros de mora são simples, de PERC_JUROS_DIA por cento ao dia (hoje 0,033%) sobre o valor original, e contam desde o primeiro dia após o vencimento, não só a partir do fim da carência.[^3][^4][^5]
- **[confirmado]** Carência de 3 dias sem encargos (RN-FAT-003): Atraso de até DIAS_CARENCIA dias corridos (hoje 3) não gera multa nem juros. A partir do quarto dia, multa e juros são cobrados.[^6][^7][^8]
- **[confirmado]** Cliente com isenção cadastrada não paga encargos (RN-FAT-004): Cliente com CLIENTE.ISENTO_ENCARGOS = 'S' não paga multa nem juros, seja qual for o tipo de cliente.[^9][^10][^11][^12]
- **[confirmado]** Órgão público não paga multa (RN-FAT-005): Cliente do tipo órgão público (PUB) não paga multa por atraso; paga apenas os juros de mora. A exceção é aplicada no backend.[^13]
- **[confirmado]** Teto de encargos por tipo de cliente (RN-FAT-006): A soma de multa e juros é limitada a um percentual do valor original: 10% para pessoa física, 20% para pessoa jurídica e 5% para órgão público.[^14][^15][^16]
- **[inferido]** Lote COBOL usa valores fixos e não aplica exceções do backend (INC-FAT-002): O lote de cobrança tem multa (2%), juros (0,033% ao dia) e carência (3 dias) fixos no programa, sem ler a tabela PARAMETRO, e não aplica a isenção de multa do órgão público nem o teto por tipo de cliente. Hoje os percentuais coincidem com os parâmetros, mas divergem assim que um parâmetro mudar; e o valor cobrado no lote pode ser maior que o calculado pelo backend para PUB e para atrasos longos.[^17][^18][^19]

## Exemplo

Fatura de R$ 1.000,00 de pessoa jurídica, paga com 10 dias de atraso:

| Item | Valor |
|---|---|
| Multa (2%) | R$ 20,00 |
| Juros (0,033% x 10 dias) | R$ 3,30 |
| Total de encargos | R$ 23,30 |

## Perguntas em aberto

1. O lote de cobrança deveria ler os parâmetros em vez de usar valores fixos?

## Sugestões de alteração

_Nenhuma sugestão registrada._

## Evidências

[^1]: `db/03_pkg_cobranca.sql:71-72`
[^2]: `db/02_parametros.sql:6-7`
[^3]: `db/03_pkg_cobranca.sql:94-96`
[^4]: `db/03_pkg_cobranca.sql:47`
[^5]: `db/02_parametros.sql:9-10`
[^6]: `db/03_pkg_cobranca.sql:66-69`
[^7]: `db/03_pkg_cobranca.sql:90-92`
[^8]: `db/02_parametros.sql:12-13`
[^9]: `db/03_pkg_cobranca.sql:59-62`
[^10]: `db/03_pkg_cobranca.sql:84-86`
[^11]: `db/01_tabelas.sql:13-14`
[^12]: `cobol/CALCOBR.cbl:69`
[^13]: `backend/src/main/java/exemplo/faturamento/EncargosService.java:31-34`
[^14]: `backend/src/main/java/exemplo/faturamento/TetoEncargosRegra.java:17-22`
[^15]: `backend/src/main/java/exemplo/faturamento/TetoEncargosRegra.java:28-32`
[^16]: `backend/src/main/java/exemplo/faturamento/EncargosService.java:36-37`
[^17]: `cobol/CALCOBR.cbl:38-40`
[^18]: `cobol/CALCOBR.cbl:69-77`
[^19]: `backend/src/main/java/exemplo/faturamento/EncargosService.java:31-37`
