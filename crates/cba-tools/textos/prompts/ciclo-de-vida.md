<!--
Prompt pronto "Montar o ciclo de vida": o app o envia ao agente quando o analista
clica no botão da tela da análise. Este comentário inicial não vai para o agente.

Edite este arquivo na base (_templates/prompts/ciclo-de-vida.md) para ajustar o
pedido; apague-o para voltar ao padrão do app. As quebras de linha viram espaços:
o pedido chega ao agente numa linha só.

Variáveis: {{id}} (identificador da análise), {{titulo}}, {{fluxo}} (nome do
fluxo) e {{documento}} (caminho do documento, relativo à base). Sem {{id}}, o app
acrescenta o identificador no fim e avisa.
-->
Monte pelo ctower o ciclo de vida da entidade principal da análise {{id}} ({{titulo}}). Chame `retomar_analise`, localize o campo de status da entidade com `schema`, `referencias` e `conhecimento`, e registre com `registrar_diagrama` (tipo `estados`) um estado por valor possível do campo (a chave é o valor como o código grava, o rótulo é o nome de negócio, com o inicial e os finais marcados) e uma transição por rotina que grava o campo, com o evento, a condição, os efeitos e a evidência do trecho que grava o valor. Confira no código os achados que a resposta devolver (estado inalcançável, sem saída, transição sem condição) e registre com `perguntar` o que o código não explicar. Não altere o código.
