<!--
Prompt pronto "Resumir para o cliente", da análise de reunião: o app o envia ao
agente quando o analista clica no botão da tela da análise. Este comentário
inicial não vai para o agente.

A ata é interna e não vai ao cliente: o resumo para o cliente sai deste pedido,
sem as referências internas da ata (citações das falas, identificadores dos
itens, das reuniões e das demandas, evidências de código e papéis internos).

Edite este arquivo na base (_templates/prompts/resumir-para-o-cliente.md) para
ajustar o pedido (por exemplo, o tom ou a saudação da sua organização); apague-o
para voltar ao padrão do app. As quebras de linha viram espaços: o pedido chega
ao agente numa linha só.

Variáveis: {{id}} (identificador da análise), {{titulo}}, {{fluxo}} (nome do
fluxo) e {{documento}} (caminho do documento, relativo à base). Sem {{id}}, o app
acrescenta o identificador no fim e avisa.
-->
Escreva um resumo da reunião da análise {{id}} para enviar ao cliente. Chame
`retomar_analise` e use só o que está registrado na ata: o objetivo, o que foi
tratado, as decisões, os próximos passos combinados, as pendências que cabem ao
cliente (com o prazo) e as dúvidas que ele precisa responder. Escreva no chat um
texto curto, em linguagem de negócio, pronto para eu enviar.

A ata é interna e não vai ao cliente; por isso o resumo não leva nenhuma
referência interna: omita as citações e os trechos de falas, os identificadores
(dos itens da ata, como D-1, N-3 ou PP-2, das reuniões, das demandas, das
análises e das regras), as evidências de código, os nomes de arquivos, rotinas e
tabelas, e os papéis internos dos participantes. Antes de me entregar, releia o
texto e tire o que restar dessas referências. Não registre nada na análise nem
na ata.
