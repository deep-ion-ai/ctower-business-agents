<!--
Roteiro do fluxo "cenários de teste". O agente o recebe na resposta de
iniciar_analise e de retomar_analise. Este comentário inicial não vai para o
agente.

Roteiro provisório: entra na cópia editável da base (_templates/roteiros/) depois
da avaliação do fluxo contra o gabarito (docs/avaliacao.md).

Variáveis: {{id}}, {{titulo}}, {{fluxo}}, {{documento}}, {{entrada}},
{{secoes}}, {{secoes_obrigatorias}} e {{contexto}}; o que cada uma traz está no
roteiro de perguntar ao sistema (_templates/roteiros/perguntar.md). Aqui,
{{entrada}} é o pedido do analista: a análise ou as histórias de origem, ou as
regras a cobrir.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento na base: `{{documento}}` (montado pelo app a partir dos registros; não o edite).

## Pedido do analista

{{entrada}}

## Objetivo

Derivar das regras de negócio os cenários de teste que o analista e o QA vão executar, **com os valores que o código aplica de fato**: os limites lidos do código, as combinações de condições e as recusas que o sistema trata. Cada cenário fica ligado à regra (ou afirmação) de que veio e à evidência do limite. Você não descobre regra nova aqui: o que o código e a base não mostram vira pergunta.

## Como conduzir

1. **Junte as regras a cobrir.** O contexto no fim deste roteiro diz se há análise de origem. Se houver, chame `retomar_analise` com ela (não com {{id}}): numa análise do código, anote as afirmações (prefira as confirmadas; a incorreta não sustenta cenário) e as regras citadas; numa análise de histórias, anote as histórias (HU-NNNN) e as regras e afirmações dos critérios, para ligar cada cenário à história que ele verifica. Sem origem, use `conhecimento` (tipo regra) para achar as regras pedidas.
2. **Leia o código de cada regra.** Abra o trecho citado nas evidências e ache onde a regra é decidida: as comparações (`<`, `<=`, `>`), as constantes, os parâmetros lidos de tabela, as mensagens e os códigos de erro. É dali que saem os valores dos cenários.
3. **Regra com duas ou mais condições combinadas vira tabela de decisão** (`registrar_tabela_decisao`): as condições (verdadeiras ou falsas, cada uma com a evidência do trecho que a testa), as ações (com a mensagem ou o código de erro) e as colunas, uma por combinação, com S, N ou - (indiferente). Cubra todas as combinações; a que não pode acontecer vira coluna `impossivel` com a justificativa. O app recusa a tabela incompleta ou com duas colunas para a mesma combinação. Depois, registre um cenário por coluna possível (técnica `tabela_de_decisao`, `tabela`: "TD-n/Rn").
4. **Regra numérica ou de data vira partição de equivalência e valores-limite.** Separe as classes (válida e inválidas) e teste um representante de cada (`particao_de_equivalencia`); teste o limite e o valor logo antes e logo depois (`valor_limite`): numa regra "recusa se `dias > 30`", os dias 30 e 31; numa "recusa se `tamanho < 10`", os tamanhos 9 e 10. Veja se a comparação é estrita ou não: é ela que diz de que lado o limite cai. Inclua o zero, o vazio e o máximo do campo quando o código os trata.
5. **Informe o limite com a evidência** em `limite`: a grandeza, o valor como está no código e o trecho que o define. O valor precisa aparecer no trecho citado; se não aparece, o app recusa. Quando o código lê o limite de tabela de parâmetros ou de configuração (o valor não está no código, só a chave), informe `configuracao` com o parâmetro e cite a `pergunta` (registre-a com `perguntar` nesta análise, como `{{id}}/n`, se ainda não existir): o cenário fica "a confirmar". O valor de uma carga inicial de parâmetros não garante o valor em produção.
6. **Inclua os cenários negativos e de exceção que o código trata** (`negativo`, `excecao`): entrada recusada, estado que impede a operação, a mensagem ou o código de erro exato no Então.
7. **Cada cenário cita a origem** em `regras` (`RN-<SIGLA>-NNN`) ou `afirmacoes` (o número na análise de origem ou `A-NNNN/n`) e, se verifica uma história, `historia` (HU-NNNN). Cenário sem origem é recusado.
8. **Escreva os passos com valores concretos:** Dado (a pré-condição, com os dados), Quando (a ação, com o valor testado) e Então (o resultado esperado, com a mensagem). Informe o valor testado em `valor`. O app dá o identificador estável (CT-NNNN) de cada cenário; cite-o no chat. Seções desta análise que recebem afirmações:
{{secoes}}
9. **Você não altera o código** nem registra regras novas.
10. **Conclua.** Confira que toda coluna possível das tabelas tem cenário (seções obrigatórias de afirmação: {{secoes_obrigatorias}}) e chame `concluir_analise` (analise: "{{id}}"). Se houver pendências, trate-as e chame de novo.

## Resposta no chat

O chat não é o documento. Depois de `concluir_analise`, dê ao analista só:

- as tabelas de decisão (TD-n) e quantos cenários por técnica;
- os limites lidos do código, com o valor;
- os cenários a confirmar e as perguntas de que dependem;
- o caminho do documento devolvido por `concluir_analise` (que também se exporta em CSV, uma linha por cenário).

## Contexto da base

{{contexto}}
