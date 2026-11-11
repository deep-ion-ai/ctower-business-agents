---
tipo: analise-impacto
id: A-0002
titulo: Parcelamento da fatura para pessoa jurídica
sistema: faturamento-exemplo
demanda:
  origem: texto
  chave: null
status: rascunho
abrangencia: pequena
base:
  branch: develop
  commit: 1a2b3c4d
criado: 2026-11-11
analista: Ana Exemplo
---

# A-0002 · Parcelamento da fatura para pessoa jurídica

Análise de impacto · Sistema `faturamento-exemplo` · Status: rascunho · Criada em 2026-11-11 · Analista: Ana Exemplo · Código: `develop` @ `1a2b3c4d`

## Demanda como veio

Permitir parcelamento da fatura em até 3 vezes para clientes pessoa jurídica. Acho que hoje o sistema não parcela fatura nenhuma.

## Demanda como entendida

A suposição está errada: o parcelamento já existe para pessoa jurídica (e órgão público), em até 6 vezes. A demanda já está atendida; se o objetivo for limitar a 3, mudar MAX_PARCELAS afeta também órgão público, e a tela continua oferecendo até 12 (INC-FAT-001).

## Como é hoje

- **[confirmado]** Parcelamento só para pessoa jurídica e órgão público (RN-FAT-007): Pessoa física não pode parcelar fatura. Pessoa jurídica e órgão público podem.[^1][^2]
- **[confirmado]** Quantidade de parcelas entre 2 e MAX_PARCELAS (RN-FAT-008): O parcelamento aceita de 2 até MAX_PARCELAS parcelas (hoje 6), valor lido da tabela PARAMETRO pelo backend.[^3][^4][^5]

## Funcionalidades

- **[inferido]** Parcelamento de fatura (F-FAT-002): só o limite de parcelas muda, se o objetivo for limitar a 3.[^3][^4][^5]

## Dados

- **[inferido]** Nenhuma tabela muda; o limite vem do parâmetro MAX_PARCELAS.

## Integrações

_Nada registrado._

## Documentos e telas

- **[a confirmar]** Arquivos que mudariam: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java`, `frontend/src/telas/parcelamento-form.ts`, `db/02_parametros.sql`.

## Riscos

- **[confirmado]** Tela oferece até 12 parcelas; backend aceita até 6 (INC-FAT-001): A tela de parcelamento valida e oferece de 2 a 12 parcelas (constante fixa no código), enquanto o backend aceita no máximo MAX_PARCELAS (hoje 6). Pedidos de 7 a 12 parcelas passam pela tela e são recusados pelo backend.[^6][^3]

## Abrangência

Estimativa: **pequena**

Pequena: o parcelamento já existe; a mudança se limita a um parâmetro e à tela.

## Perguntas para o solicitante

1. O limite de 3 parcelas vale também para órgão público?
2. A tela deve deixar de oferecer até 12 parcelas?
   Hoje o backend recusa acima de 6.

## Sugestões de alteração

- [S-0001 · Limitar a tela ao parâmetro MAX_PARCELAS](../sugestoes/S-0001.md) (`frontend/src/telas/parcelamento-form.ts`)

## Evidências

[^1]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:30-32`
[^2]: `frontend/src/telas/parcelamento-form.ts:25-30`
[^3]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:34-35`
[^4]: `backend/src/main/java/exemplo/faturamento/ParcelamentoService.java:14`
[^5]: `db/02_parametros.sql:15-16`
[^6]: `frontend/src/telas/parcelamento-form.ts:19`
