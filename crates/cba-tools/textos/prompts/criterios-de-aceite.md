<!--
Prompt pronto "Gerar critérios de aceite": o app o envia ao agente quando o
analista clica no botão da tela da análise. Este comentário inicial não vai para o
agente.

Edite este arquivo na base (_templates/prompts/criterios-de-aceite.md) para
ajustar o pedido (por exemplo, para o formato de critério que a sua organização
usa); apague-o para voltar ao padrão do app. As quebras de linha viram espaços: o
pedido chega ao agente numa linha só.

Variáveis: {{id}} (identificador da análise), {{titulo}}, {{fluxo}} (nome do
fluxo) e {{documento}} (caminho do documento, relativo à base). Sem {{id}}, o app
acrescenta o identificador no fim e avisa.
-->
Gere critérios de aceite a partir da análise {{id}}. Chame `retomar_analise` e escreva no chat um critério por regra ou mudança registrada, no formato Dado / Quando / Então, com valores concretos (um exemplo com números em cada um) e o número da afirmação de origem. Inclua os casos de borda que o código trata e marque os critérios que dependem de perguntas ainda em aberto. Use só o que está registrado na análise e não registre nada novo.
