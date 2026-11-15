<!--
Template das histórias de usuário com critérios de aceite (fluxo "Histórias e
critérios"). Este comentário inicial não vai para o documento.

Marcadores de campo: {{id}}, {{titulo}}, {{projeto}}, {{status}}, {{criado}},
{{analista}}, {{versao_do_codigo}}, {{origem}}.

Marcadores de seção: {{pedido}}, {{historias}}, {{rastreabilidade}},
{{perguntas}}, {{evidencias}}.

Cada história sai com o identificador (HU-0001), a narrativa "Como / quero /
para", a decisão do analista, a verificação INVEST e os critérios de aceite
(CA-1, CA-2...) em Dado / Quando / Então, com as afirmações e as regras que
sustentam cada critério. Critério que depende de pergunta em aberto sai como "a
confirmar". {{rastreabilidade}} é a tabela história → critério → afirmações e
regras; as evidências de código das afirmações e das regras citadas são as notas
de rodapé.

Uma seção com conteúdo cujo marcador foi removido vai para o fim do documento,
com aviso. Marcador desconhecido também gera aviso e fica no texto como está.
-->
# {{id}} · {{titulo}}

Histórias de usuário · Projeto `{{projeto}}` · Status: {{status}} · Criadas em {{criado}} · Analista: {{analista}} · Origem: {{origem}} · Código: {{versao_do_codigo}}

## Pedido

{{pedido}}

## Histórias

{{historias}}

## Rastreabilidade

{{rastreabilidade}}

## Perguntas em aberto

{{perguntas}}

## Evidências

{{evidencias}}
