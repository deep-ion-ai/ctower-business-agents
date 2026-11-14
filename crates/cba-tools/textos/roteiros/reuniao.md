<!--
Roteiro PROVISÓRIO do fluxo "análise de reunião". O texto revisado vem na #140;
até lá, este roteiro não é copiado para a base. O agente o recebe na resposta de
iniciar_analise e de retomar_analise. Este comentário inicial não vai para o
agente.

Variáveis:
  {{id}}                  identificador da análise (A-0001)
  {{titulo}}              título da análise
  {{fluxo}}               nome do fluxo ("análise de reunião")
  {{documento}}           caminho do documento da análise, relativo à base
  {{entrada}}             a reunião analisada (identificador e ata), como citação
  {{secoes}}              seções do documento da análise que recebem afirmações
  {{secoes_obrigatorias}} as seções obrigatórias, separadas por vírgula
  {{contexto}}            a demanda, as atas anteriores, as pendências, as pessoas
                          do projeto e o que a base tem sobre o assunto

Sem {{entrada}}, {{secoes}} ou {{contexto}}, o app acrescenta o valor no fim e
avisa. Variável desconhecida fica no texto como está, com aviso.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento da análise na base: `{{documento}}` (montado pelo app; não o edite).

## Reunião

{{entrada}}

## Como conduzir

1. **Leia o transcript com `ler_transcript`**, página a página (use o `cursor` da resposta). Filtre por falante ou por horário quando precisar voltar a um trecho.
2. **Proponha o papel de cada participante com `propor_papel`**, citando as falas que mostram o papel. A resposta traz o papel que o catálogo de pessoas sugere; quem confirma é o analista.
3. **Registre cada item da ata com `registrar_item_ata`**: decisão, necessidade, regra citada, pendência (ação, responsável e prazo), dúvida ou próximo passo, sempre com a fala de origem (o índice da fala e um trecho curto copiado dela). Citação recusada volta com o motivo: corrija e chame de novo.
4. **Ligação com o código (opcional):** para necessidades e regras citadas, consulte a base (`conhecimento`) e o código, e cite a evidência no item (`evidencias`). A análise de impacto completa vira um próximo passo.
5. **Seções do documento da análise:**
{{secoes}}
6. **Conclua** com `concluir_analise` (analise: "{{id}}"), que confere se a ata tem itens. Seções obrigatórias do documento da análise: {{secoes_obrigatorias}}.

## Contexto

{{contexto}}
