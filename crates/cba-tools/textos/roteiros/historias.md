<!--
Roteiro do fluxo "histórias e critérios de aceite". O agente o recebe na
resposta de iniciar_analise e de retomar_analise. Este comentário inicial não vai
para o agente.

Roteiro provisório: entra na cópia editável da base (_templates/roteiros/) depois
da avaliação do fluxo contra o gabarito (docs/avaliacao.md).

Variáveis: {{id}}, {{titulo}}, {{fluxo}}, {{documento}}, {{entrada}},
{{secoes}}, {{secoes_obrigatorias}} e {{contexto}}; o que cada uma traz está no
roteiro de perguntar ao sistema (_templates/roteiros/perguntar.md). Aqui,
{{entrada}} é o pedido do analista: a análise de origem e o que ele quer nas
histórias.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento na base: `{{documento}}` (montado pelo app a partir dos registros; não o edite).

## Pedido do analista

{{entrada}}

## Objetivo

Transformar o que foi descoberto no código em histórias de usuário que o analista entrega ao time: cada história com critérios de aceite verificáveis, com valores concretos, e **cada critério ligado à afirmação ou à regra que o sustenta**. Você não descobre regra nova aqui: usa o que a análise de origem e a base já registraram, e o que não está registrado vira pergunta.

## Como conduzir

1. **Leia a análise de origem.** O contexto no fim deste roteiro diz qual é. Chame `retomar_analise` com ela (não com {{id}}) e anote: as afirmações (número, texto e se o analista confirmou), as regras citadas, as perguntas em aberto e a abrangência. Prefira afirmações confirmadas pelo analista; afirmação marcada como incorreta não sustenta critério. Sem análise de origem (pedido a partir de uma demanda), use `conhecimento` para achar as regras (`RN-…`) e as análises relacionadas, e cite-as como `A-NNNN/n`.
2. **Separe as histórias por valor para quem usa,** não por camada técnica: "Como <papel>, quero <objetivo>, para <benefício>". O papel é de negócio (o analista financeiro do cliente, o atendente), o objetivo é o que a pessoa faz e o benefício é para o negócio. Uma história por fatia de valor que pode ser entregue sozinha.
3. **Escreva os critérios de aceite em Dado / Quando / Então,** um comportamento por critério, sempre com valores concretos (um exemplo com números: valores, datas, quantidades, tipos de cliente). Inclua os casos de borda que o código trata (limites, recusas, arredondamentos) e as recusas com a mensagem, quando o código a mostra.
4. **Cada critério cita a sua origem,** em `afirmacoes` (o número da afirmação na análise de origem, como `4`, ou `A-NNNN/n` para outra análise) e em `regras` (`RN-<SIGLA>-NNN`). Critério sem origem é recusado: se nada registrado sustenta o comportamento, ele não é critério, é pergunta.
5. **O que depende de pergunta em aberto fica a confirmar:** cite a pergunta em `pergunta` (o número na análise de origem, ou `{{id}}/n` para uma pergunta que você registrar nesta análise com `perguntar`). Não invente o comportamento que o código não mostra.
6. **Verifique o INVEST de cada história** (independente, negociável, valiosa, estimável, pequena, testável) e informe em `invest` só o que ela **não** atende, com o motivo. Uma história sem critério que já possa ser testado é marcada como não testável pelo app.
7. **Quebre o que é grande.** Quando a abrangência da origem é média ou grande, ou a história junta mudanças que não cabem numa entrega (tela e relatório, regra nova e migração de dados), proponha a quebra em fatias verticais em `quebra`, dizendo por quê, e pergunte ao analista no chat. Se ele aceitar (ou pedir a quebra no app), registre cada fatia com `fatia_de` apontando a história original, que fica marcada como quebrada.
8. **Registre com `registrar_historia`** (analise: "{{id}}"), uma história por chamada, com os critérios em `criterios`; para acrescentar um critério depois, use `registrar_criterio` com a história (HU-NNNN). O app dá o identificador estável (HU-NNNN) de cada história; cite-o no chat. Referência recusada volta com o motivo: corrija e chame de novo. Seções desta análise que recebem afirmações:
{{secoes}}
9. **Você não altera o código** nem registra regras novas: as regras nascem na aprovação das análises do código.
10. **Conclua.** Confira que toda história tem ao menos um critério (seções obrigatórias de afirmação: {{secoes_obrigatorias}}) e chame `concluir_analise` (analise: "{{id}}"). Se houver pendências, trate-as e chame de novo.

## Resposta no chat

O chat não é o documento. Depois de `concluir_analise`, dê ao analista só:

- a lista das histórias (HU-NNNN e título), com quantos critérios cada uma tem;
- os critérios a confirmar e as perguntas de que dependem;
- os alertas INVEST e as quebras propostas;
- o caminho do documento devolvido por `concluir_analise`, onde estão os critérios e as ligações com as afirmações e as regras.

## Contexto da base

{{contexto}}
