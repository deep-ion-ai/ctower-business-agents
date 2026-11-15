<!--
Prompt pronto "Gerar histórias e critérios": o app o envia ao agente quando o
analista clica no botão da tela da análise. Ele abre o fluxo "Histórias e
critérios" (#115) a partir desta análise: as histórias viram um documento
próprio, revisado e aprovado no app, com cada critério ligado às afirmações e às
regras. Substitui o antigo "Gerar critérios de aceite", que só escrevia no chat.
Este comentário inicial não vai para o agente.

Edite este arquivo na base (_templates/prompts/gerar-historias.md) para ajustar
o pedido; apague-o para voltar ao padrão do app. As quebras de linha viram
espaços: o pedido chega ao agente numa linha só.

Variáveis: {{id}} (identificador da análise), {{titulo}}, {{fluxo}} (nome do
fluxo) e {{documento}} (caminho do documento, relativo à base). Sem {{id}}, o app
acrescenta o identificador no fim e avisa.
-->
Gere histórias de usuário com critérios de aceite a partir da análise {{id}} pelo ctower. Chame `iniciar_analise` com fluxo "historias" e origem "{{id}}", diga ao analista o identificador da análise de histórias criada e siga o roteiro devolvido: cada critério em Dado / Quando / Então, com valores concretos, citando as afirmações e as regras que o sustentam, e os critérios que dependem de pergunta em aberto marcados como a confirmar. Não escreva as histórias só no chat: registre-as com `registrar_historia`.
