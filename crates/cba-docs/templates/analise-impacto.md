<!--
Template da análise de impacto. Este comentário inicial não vai para o documento.

Marcadores de campo: {{id}}, {{titulo}}, {{projeto}}, {{status}}, {{criado}},
{{analista}}, {{abrangencia_estimada}}, {{versao_do_codigo}}.

Marcadores de seção: {{demanda_como_veio}}, {{demanda_como_entendida}},
{{decisoes}}, {{como_e_hoje}}, {{funcionalidades}}, {{dados}}, {{integracoes}},
{{documentos_e_telas}}, {{dados_pessoais_e_rnf}}, {{riscos}}, {{abrangencia}},
{{verificacoes_em_dados}}, {{perguntas}}, {{sugestoes}}, {{evidencias}}.

Uma seção com conteúdo cujo marcador foi removido vai para o fim do documento,
com aviso. A exceção é {{decisoes}} (as decisões para o negócio, com as opções):
sem o marcador, ela entra logo depois de {{demanda_como_entendida}}, porque quem
decide precisa achá-la sem ler a parte técnica. Marcador desconhecido também
gera aviso e fica no texto como está.
-->
# {{id}} · {{titulo}}

Análise de impacto · Projeto `{{projeto}}` · Status: {{status}} · Criada em {{criado}} · Analista: {{analista}} · Código: {{versao_do_codigo}}

## Demanda como veio

{{demanda_como_veio}}

## Demanda como entendida

{{demanda_como_entendida}}

## Decisões para o negócio

{{decisoes}}

## Como é hoje

{{como_e_hoje}}

## Funcionalidades

{{funcionalidades}}

## Dados

{{dados}}

## Integrações

{{integracoes}}

## Documentos e telas

{{documentos_e_telas}}

## Dados pessoais e requisitos não funcionais

{{dados_pessoais_e_rnf}}

## Riscos

{{riscos}}

## Abrangência

Estimativa: **{{abrangencia_estimada}}**

{{abrangencia}}

## Verificações em dados

{{verificacoes_em_dados}}

## Perguntas para o solicitante

{{perguntas}}

## Sugestões de alteração

{{sugestoes}}

## Evidências

{{evidencias}}
