<!--
Roteiro do fluxo "analisar impacto de uma demanda". O agente o recebe na
resposta de iniciar_analise e de retomar_analise. Este comentário inicial não vai
para o agente.

Versão inicial: este roteiro ainda será revisado (M2) e, por isso, o app ainda
não grava a cópia editável na base. Para ajustá-lo agora, crie o arquivo
_templates/roteiros/impacto.md na base com este conteúdo e edite.

Variáveis: {{id}}, {{titulo}}, {{fluxo}}, {{documento}}, {{entrada}},
{{secoes}}, {{secoes_obrigatorias}} e {{contexto}}; o que cada uma traz está no
roteiro de perguntar ao sistema (_templates/roteiros/perguntar.md). Sem
{{entrada}}, {{secoes}} ou {{contexto}}, o app acrescenta o valor no fim e avisa.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento na base: `{{documento}}` (montado pelo app a partir dos registros; não o edite).

## Entrada do analista

{{entrada}}

## Como conduzir

Separe o que o solicitante afirmou como fato do que ele supôs ("acho que", "salvo engano") e verifique cada afirmação dele no código. Descreva como é hoje, levante o que muda (funcionalidades, dados, integrações, documentos e telas), registre os riscos e estime a abrangência com justificativa. Uma boa análise de impacto costuma corrigir a premissa da demanda: deixe isso explícito em demanda_como_entendida.

1. Consulte primeiro o contexto da base (abaixo) e, para outros conceitos da entrada, `conhecimento`: regras já confirmadas e termos do glossário poupam exploração e não precisam ser redescobertos.
2. Localize no código antes de ler: `referencias` diz onde uma rotina, tabela ou campo é usado (e, com `sentido: "o_que_usa"`, o que ele usa), `estrutura` lista as rotinas de um arquivo com as linhas, `schema` mostra tabelas e colunas e `mapa` dá a visão das pastas. Depois leia o código com as suas ferramentas de leitura e busca. O que não está no código (parâmetros em tabela, jobs agendados, sistemas vizinhos) vira pergunta, não suposição.
3. Registre o conteúdo com `afirmar` (analise: "{{id}}"), uma regra ou fato por chamada, com a confiança honesta e as evidências (arquivo relativo à raiz do sistema, linhas e o trecho copiado do código). Evidência recusada volta com o motivo e a correção: corrija e chame de novo. Seções desta análise:
{{secoes}}
4. O que o código não responde, ou uma premissa a confirmar, vai para `perguntar`.
5. Você não altera o código. Se o analista pedir correção ou mudança, explique o que mudaria e registre com `registrar_sugestao` (trecho atual, trecho proposto, motivo, impacto e um exemplo com números).
6. Termos de negócio que você ligou ao código vão para `propor_termo`.
7. Ao terminar, confira as seções obrigatórias ({{secoes_obrigatorias}}) e chame `concluir_analise` (analise: "{{id}}"). No chat, dê ao analista só um resumo (demanda como entendida, abrangência, principais riscos e as perguntas para o solicitante) e o caminho do documento devolvido; o documento completo fica na base.

## Contexto da base

{{contexto}}