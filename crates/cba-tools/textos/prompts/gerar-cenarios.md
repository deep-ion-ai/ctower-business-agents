<!--
Prompt pronto "Gerar cenários de teste": o app o envia ao agente quando o
analista clica no botão da tela da análise. Ele abre o fluxo "Cenários de teste"
(#117) a partir desta análise (do código ou de histórias): os cenários viram um
documento próprio, revisado e aprovado no app, com cada cenário ligado à regra e
à evidência do limite, e exportável em CSV. Este comentário inicial não vai para
o agente.

Edite este arquivo na base (_templates/prompts/gerar-cenarios.md) para ajustar
o pedido; apague-o para voltar ao padrão do app. As quebras de linha viram
espaços: o pedido chega ao agente numa linha só.

Variáveis: {{id}} (identificador da análise), {{titulo}}, {{fluxo}} (nome do
fluxo) e {{documento}} (caminho do documento, relativo à base). Sem {{id}}, o app
acrescenta o identificador no fim e avisa.
-->
Gere cenários de teste a partir da análise {{id}} pelo ctower. Chame `iniciar_analise` com fluxo "cenarios" e origem "{{id}}", diga ao analista o identificador da análise de cenários criada e siga o roteiro devolvido: tabela de decisão para regra com várias condições, partição de equivalência e valores-limite com os valores lidos do código e a evidência do limite, cenários negativos e de exceção, e o que depende de parâmetro fora do código marcado como a confirmar. Não escreva os cenários só no chat: registre-os com `registrar_tabela_decisao` e `registrar_cenario`.
