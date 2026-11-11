<!--
Prompt pronto "Conferir evidências": o app o envia ao agente quando o analista
clica no botão da tela da análise. Este comentário inicial não vai para o agente.

Edite este arquivo na base (_templates/prompts/conferir-evidencias.md) para
ajustar o pedido; apague-o para voltar ao padrão do app. As quebras de linha viram
espaços: o pedido chega ao agente numa linha só.

Variáveis: {{id}} (identificador da análise), {{titulo}}, {{fluxo}} (nome do
fluxo) e {{documento}} (caminho do documento, relativo à base). Sem {{id}}, o app
acrescenta o identificador no fim e avisa.
-->
Confira as evidências da análise {{id}} pelo ctower. Chame `retomar_analise` e, para cada afirmação, releia no código atual os trechos citados: o trecho sustenta o que foi afirmado, com os mesmos valores e condições? Me diga, por número de afirmação, o que confere, o que mudou e o que não se sustenta. Registre com `afirmar` a versão corrigida do que não se sustentar e com `perguntar` o que ficar em dúvida. Não altere o código.
