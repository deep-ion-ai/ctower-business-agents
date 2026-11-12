<!--
Prompt pronto "Aprofundar": o app o envia ao agente quando o analista clica no
botão da tela da análise. Este comentário inicial não vai para o agente.

Edite este arquivo na base (_templates/prompts/aprofundar.md) para ajustar o
pedido; apague-o para voltar ao padrão do app. As quebras de linha viram espaços:
o pedido chega ao agente numa linha só.

Variáveis: {{id}} (identificador da análise), {{titulo}}, {{fluxo}} (nome do
fluxo) e {{documento}} (caminho do documento, relativo à base). Sem {{id}}, o app
acrescenta o identificador no fim e avisa.
-->
Aprofunde a análise {{id}} pelo ctower. Chame `retomar_analise` e trate primeiro as afirmações que marquei para aprofundar, depois as inferidas e as a confirmar: procure no código a evidência que falta, registre o que encontrar com `afirmar` e o que o código não responder com `perguntar`. Não repita o que já está registrado. Ao terminar, chame `concluir_analise`.
