<!--
Template do manual do usuário de uma funcionalidade (fluxo "Manual do
usuário"). Este comentário inicial não vai para o documento.

Marcadores de campo: {{titulo}}, {{status}}, {{id}}, {{projeto}}, {{criado}},
{{analista}}, {{versao_do_codigo}}, {{area}}, {{tema}}, {{origem}}. Os campos
que o documento não tem (por exemplo, {{area}} no manual ainda em análise, em
analises/) saem como "—".

Marcadores de seção: {{objetivo}}, {{quem_usa}}, {{pre_requisitos}},
{{passo_a_passo}}, {{resultados_esperados}}, {{mensagens_e_erros}},
{{perguntas_frequentes}}, {{perguntas}}, {{evidencias}}.

{{passo_a_passo}} traz os passos numerados, cada um com a instrução, o print de
tela (imagem com legenda e texto alternativo, guardada na pasta manual/ da
funcionalidade) e o resultado esperado. Passo sem print mostra o marcador
"Print sugerido: tela X" até o analista incluir a imagem ou dispensá-la.

Uma seção com conteúdo cujo marcador foi removido vai para o fim do documento,
com aviso. Marcador desconhecido também gera aviso e fica no texto como está.
-->
# {{titulo}}

Manual do usuário · Status: {{status}}

## Objetivo

{{objetivo}}

## Quem usa

{{quem_usa}}

## Antes de começar

{{pre_requisitos}}

## Passo a passo

{{passo_a_passo}}

## Resultados esperados

{{resultados_esperados}}

## Mensagens e erros comuns

{{mensagens_e_erros}}

## Perguntas frequentes

{{perguntas_frequentes}}

## Perguntas em aberto

{{perguntas}}

## Evidências

{{evidencias}}
