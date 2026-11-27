<!--
Prompt pronto "Resumir para o solicitante": o app o envia ao agente quando o
analista clica no botão da tela da análise. Este comentário inicial não vai para o
agente.

Edite este arquivo na base (_templates/prompts/resumir.md) para ajustar o pedido;
apague-o para voltar ao padrão do app. As quebras de linha viram espaços: o pedido
chega ao agente numa linha só.

Variáveis: {{id}} (identificador da análise), {{titulo}}, {{fluxo}} (nome do
fluxo) e {{documento}} (caminho do documento, relativo à base). Sem {{id}}, o app
acrescenta o identificador no fim e avisa.
-->
Resuma a análise {{id}} ({{titulo}}) para o solicitante, que não lê código. Chame `retomar_analise` e escreva no chat um texto curto, pronto para eu enviar. Comece pelas decisões que cabem a ele (as perguntas do tipo decisão), cada uma com as opções, o efeito de cada opção e a recomendação, se houver. Depois: a resposta ou conclusão principal, as regras numeradas em linguagem de negócio, um exemplo com números e as perguntas que ele precisa responder. O que for dedução e não fato (afirmação inferida) diga como dedução. Sem nomes de arquivos, rotinas ou tabelas: o detalhe técnico, se precisar entrar, vai num bloco final "Notas para a equipe técnica", que eu posso tirar antes de enviar. Não registre nada novo na análise.
