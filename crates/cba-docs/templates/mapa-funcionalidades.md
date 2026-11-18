<!--
Template do mapa de funcionalidades (fluxo "Mapear funcionalidades"): o que uma
rodada do mapeamento achou no código. Este comentário inicial não vai para o
documento.

Marcadores de campo: {{id}}, {{titulo}}, {{projeto}}, {{status}}, {{criado}},
{{analista}}, {{versao_do_codigo}}.

Marcadores de seção: {{pedido}}, {{funcionalidades_mapeadas}}, {{nao_mapeados}},
{{perguntas}}, {{evidencias}}.

{{funcionalidades_mapeadas}} traz as funcionalidades por área, cada uma com o
identificador estável (F-<SIGLA>-NNN), a descrição, os atores, os pontos de
entrada (com a evidência como nota de rodapé), os dados lidos e gravados, as
funcionalidades relacionadas e a decisão do analista. {{nao_mapeados}} lista os
pontos de entrada do código que ficaram sem funcionalidade, com a justificativa
(código morto, utilitário ou outro motivo).

Ao aprovar a análise, as funcionalidades aceitas entram no catálogo do projeto
(funcionalidades/catalogo.md).

Uma seção com conteúdo cujo marcador foi removido vai para o fim do documento,
com aviso. Marcador desconhecido também gera aviso e fica no texto como está.
-->
# {{id}} · {{titulo}}

Mapa de funcionalidades · Projeto `{{projeto}}` · Status: {{status}} · Criado em {{criado}} · Analista: {{analista}} · Código: {{versao_do_codigo}}

## Escopo

{{pedido}}

## Funcionalidades

{{funcionalidades_mapeadas}}

## Pontos de entrada sem funcionalidade

{{nao_mapeados}}

## Perguntas em aberto

{{perguntas}}

## Evidências

{{evidencias}}
