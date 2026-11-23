<!--
Template do mapa de funcionalidades do projeto (funcionalidades/mapa.md).
Este comentário inicial não vai para o documento.

Marcadores de campo: {{titulo}}, {{projeto}}, {{status}}, {{total}} (quantas
funcionalidades há no mapa, sem as descartadas e as juntadas), {{pendentes}}
(quantas ainda esperam a decisão do analista), {{atualizado}}.

Marcadores de seção: {{indice}}, {{funcionalidades_mapeadas}},
{{nao_mapeados}}, {{evidencias}}.

{{indice}} é a tabela por área: identificador, nome, situação (aceita ou a
revisar; documentada quando tem documento negocial) e pontos de entrada.
{{funcionalidades_mapeadas}} traz cada funcionalidade com a descrição, os atores,
os pontos de entrada (com a evidência como nota de rodapé), os dados lidos e
gravados, as relacionadas e o documento negocial, quando houver.
{{nao_mapeados}} é a tabela dos pontos de entrada que ficaram sem
funcionalidade, com o motivo. O identificador (F-<SIGLA>-NNN) não muda quando o
mapeamento roda de novo.

Uma seção com conteúdo cujo marcador foi removido vai para o fim do documento,
com aviso. Marcador desconhecido também gera aviso e fica no texto como está.
-->
# {{titulo}}

Mapa de funcionalidades · Projeto `{{projeto}}` · Status: {{status}} · {{total}} funcionalidades ({{pendentes}} a revisar) · Atualizado em {{atualizado}}

## Índice

{{indice}}

## Funcionalidades

{{funcionalidades_mapeadas}}

## Pontos de entrada sem funcionalidade

{{nao_mapeados}}

## Evidências

{{evidencias}}
