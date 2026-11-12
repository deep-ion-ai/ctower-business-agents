<!--
Prompt pronto "Retomar": o app o envia ao agente para continuar a análise numa
conversa nova (spec, seção 4.7). Este comentário inicial não vai para o agente.

Edite este arquivo na base (_templates/prompts/retomar.md) para ajustar o pedido;
apague-o para voltar ao padrão do app. As quebras de linha viram espaços: o pedido
chega ao agente numa linha só.

Variáveis: {{id}} (identificador da análise), {{titulo}}, {{fluxo}} (nome do
fluxo) e {{documento}} (caminho do documento, relativo à base). Sem {{id}}, o app
acrescenta o identificador no fim e avisa.
-->
Retome a análise {{id}} pelo ctower.
