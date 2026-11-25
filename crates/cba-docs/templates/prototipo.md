<!--
Template do protótipo de telas (fluxo "Protótipo de telas"). Este comentário
inicial não vai para o documento.

Marcadores de campo: {{id}}, {{titulo}}, {{projeto}}, {{status}}, {{criado}},
{{analista}}, {{versao_do_codigo}}, {{origem}}.

Marcadores de seção: {{pedido}}, {{telas}}, {{rastreabilidade}},
{{perguntas}}, {{evidencias}}.

Cada tela sai com o identificador (TL-1), se é nova ou alterada, o motivo ligado
à análise de origem e os elementos (E-1, E-2...): campos, validações, mensagens e
ações, cada um com as afirmações, as regras e os critérios de aceite de que vem.
Tela ou elemento sem fundamento sai como "a confirmar". Com o protótipo HTML
gravado, a tela traz o link para ele (`A-NNNN/prototipo/TL-n.html`, com os
estados vazio, preenchido, erro de validação e sucesso). {{rastreabilidade}} é a
tabela tela → elemento → afirmações, regras e critérios; as evidências de código
das afirmações e das regras citadas são as notas de rodapé.

Uma seção com conteúdo cujo marcador foi removido vai para o fim do documento,
com aviso. Marcador desconhecido também gera aviso e fica no texto como está.
-->
# {{id}} · {{titulo}}

Protótipo de telas · Projeto `{{projeto}}` · Status: {{status}} · Criado em {{criado}} · Analista: {{analista}} · Origem: {{origem}} · Código: {{versao_do_codigo}}

## Pedido

{{pedido}}

## Telas afetadas

{{telas}}

## Rastreabilidade

{{rastreabilidade}}

## Perguntas em aberto

{{perguntas}}

## Evidências

{{evidencias}}
