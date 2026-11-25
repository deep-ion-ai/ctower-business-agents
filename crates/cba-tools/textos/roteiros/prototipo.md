<!--
Roteiro do fluxo "protótipo de telas". O agente o recebe na resposta de
iniciar_analise e de retomar_analise. Este comentário inicial não vai para o
agente.

Para ajustar o roteiro à sua organização (por exemplo, os tipos de tela da casa
ou o nível de detalhe dos elementos), edite este arquivo na base
(_templates/roteiros/prototipo.md). Para voltar ao padrão do app, apague-o: o app
grava de novo a cópia padrão na próxima análise.

Variáveis: {{id}}, {{titulo}}, {{fluxo}}, {{documento}}, {{entrada}},
{{secoes}}, {{secoes_obrigatorias}} e {{contexto}}; o que cada uma traz está no
roteiro de perguntar ao sistema (_templates/roteiros/perguntar.md). Aqui,
{{entrada}} é o pedido do analista: a análise de origem, as telas que ele quer
ver, a fidelidade (wireframe ou alta fidelidade) e o que deixar de fora.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento na base: `{{documento}}` (montado pelo app a partir dos registros; não o edite).

## Pedido do analista

{{entrada}}

## Objetivo

Mostrar ao negócio como as telas vão ficar depois da demanda, a partir do que a análise de origem já validou. Primeiro você lista as telas afetadas, cada uma nova ou alterada e com o motivo ligado à origem; depois, em cada tela, os campos, as validações, as mensagens e as ações, **cada elemento ligado à regra, à afirmação ou ao critério de aceite de onde vem**. Você não descobre regra nova aqui: o que nada registrado sustenta fica "a confirmar", à vista do analista.

## Como conduzir

1. **Leia a análise de origem.** O contexto no fim deste roteiro diz qual é. Chame `retomar_analise` com ela (não com {{id}}) e anote: as afirmações (número, texto e se o analista confirmou), as regras citadas, as telas e os documentos afetados, as perguntas em aberto e, se a origem for de histórias, as histórias (HU-NNNN) e os critérios de aceite (CA-n). Prefira o que o analista confirmou; afirmação marcada como incorreta não sustenta nada.
2. **Respeite o pedido.** Se o analista disse quais telas quer, comece por elas; se disse o que deixar de fora, não registre. A fidelidade pedida orienta o detalhe: no wireframe, a estrutura e o conteúdo; em alta fidelidade, também os textos exatos e os estados (vazio, erro, sucesso).
3. **Liste as telas afetadas** com `registrar_tela` (analise: "{{id}}"), uma por chamada: o nome da tela como o usuário a conhece, `mudanca` (`nova` ou `alterada`) e o `motivo`, ligado à origem em `afirmacoes` (o número da afirmação na análise de origem, como `4`, ou `A-NNNN/n`), `regras` (`RN-<SIGLA>-NNN`) e `criterios` (`HU-NNNN/CA-n`). Para tela que já existe, confira no código onde ela está (`estrutura`, `mapa`) antes de dizer que é alterada.
4. **Registre os elementos de cada tela** em `elementos`, na mesma chamada, ou depois com `registrar_elemento_tela` (a tela como `TL-n`): `tipo` (`campo`, `validacao`, `mensagem` ou `acao`), o `nome` como aparece na tela e, em `descricao`, o detalhe (formato, valores, o texto da mensagem, a condição da validação, com os valores lidos do código). Cada elemento cita a regra, a afirmação ou o critério que o sustenta.
5. **O que não tem fundamento fica a confirmar.** Elemento sem nenhuma referência é aceito, mas sai "a confirmar" no documento: use isso para o que a tela precisa e a análise de origem não diz, em vez de inventar uma regra. Se depender de uma pergunta em aberto, cite-a em `pergunta` (o número na análise de origem, ou `{{id}}/n` para uma pergunta registrada nesta análise com `perguntar`).
6. **Você não gera o HTML das telas nem altera o código** nem registra regras novas: o protótipo HTML de cada tela vem depois da lista, e as regras nascem na aprovação das análises do código. Seções desta análise que recebem afirmações:
{{secoes}}
7. **Mantenha a memória da análise** com `memorizar` (analise: "{{id}}"): a análise de origem e o que sustenta as telas (`contexto`); o que o analista pediu e combinou (`combinados`); as telas que você pensou e descartou, com o motivo (`descartado`); as telas e elementos que faltam (`pendencias`); e onde você parou, por exemplo "faltam as mensagens da TL-2" (`onde_parou`, com `modo: "substituir"`). Não repita o que já está registrado: cite as telas pelo identificador (TL-n).
8. **Conclua.** Confira que toda tela tem ao menos um elemento (seções obrigatórias de afirmação: {{secoes_obrigatorias}}) e chame `concluir_analise` (analise: "{{id}}"). Se houver pendências, trate-as e chame de novo.

## Resposta no chat

O chat não é o documento. Depois de `concluir_analise`, dê ao analista só:

- a lista das telas (TL-n, nome, nova ou alterada), com quantos elementos cada uma tem;
- os elementos e as telas a confirmar, com o que falta para confirmá-los;
- o caminho do documento devolvido por `concluir_analise`, onde estão os elementos e as ligações com as regras e os critérios.

## Contexto da base

{{contexto}}
