<!--
Template do parecer de revisão de um requisito ou história (fluxo "Revisar
requisito"). Este comentário inicial não vai para o documento.

O texto revisado entra em {{requisito}}, com a fonte (a chave da issue no
rastreador) quando veio de lá. O veredito, os achados e a verificação INVEST são
montados pelo app a partir do que o agente registrou: os achados saem agrupados
por severidade, cada um com a característica da ISO/IEC/IEEE 29148 violada, o
trecho, o problema, as regras da base e a reescrita sugerida.

Marcadores de campo: {{id}}, {{titulo}}, {{projeto}}, {{status}}, {{criado}},
{{analista}}, {{abrangencia_estimada}}, {{versao_do_codigo}}.

Marcadores de seção: {{requisito}}, {{veredito}}, {{achados}}, {{invest}},
{{contexto_no_codigo}}, {{perguntas}}, {{sugestoes}}, {{evidencias}}.
-->
# {{id}} · {{titulo}}

Parecer de revisão · Projeto `{{projeto}}` · Status: {{status}} · Criado em {{criado}} · Analista: {{analista}} · Código: {{versao_do_codigo}}

## Requisito revisado

{{requisito}}

## Veredito

{{veredito}}

## Achados

{{achados}}

## Verificação INVEST

{{invest}}

## O que a base e o código dizem

{{contexto_no_codigo}}

## Perguntas para o autor

{{perguntas}}

## Sugestões de alteração

{{sugestoes}}

## Evidências

{{evidencias}}
