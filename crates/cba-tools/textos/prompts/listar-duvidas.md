<!--
Prompt pronto "Listar dúvidas para enviar", da análise de reunião: o app o envia
ao agente quando o analista clica no botão da tela da análise. Este comentário
inicial não vai para o agente.

Edite este arquivo na base (_templates/prompts/listar-duvidas.md) para ajustar o
pedido (por exemplo, o formato da lista que a sua organização envia ao cliente);
apague-o para voltar ao padrão do app. As quebras de linha viram espaços: o
pedido chega ao agente numa linha só.

Variáveis: {{id}} (identificador da análise), {{titulo}}, {{fluxo}} (nome do
fluxo) e {{documento}} (caminho do documento, relativo à base). Sem {{id}}, o app
acrescenta o identificador no fim e avisa.
-->
Liste as dúvidas em aberto da reunião da análise {{id}}, prontas para eu enviar
ao cliente. Chame `retomar_analise` e reúna as dúvidas em aberto da ata, as
pendências ainda sem responsável ou sem prazo e as perguntas em aberto da análise
que só o cliente pode responder. Escreva no chat uma lista numerada, uma pergunta
por item, clara para quem não esteve na reunião e não lê código, com o contexto
mínimo para responder (o assunto e, se houver, as opções levantadas na reunião);
agrupe por assunto se forem muitas. A lista vai ao cliente: omita as citações e
os trechos de falas, os identificadores (dos itens da ata, das reuniões, das
demandas e das análises), as evidências de código e os nomes de arquivos. Não
registre nada na análise nem na ata.
