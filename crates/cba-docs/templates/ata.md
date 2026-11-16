<!--
Template da ata de reunião. Este comentário inicial não vai para o documento.

Marcadores de campo: {{id}}, {{projeto}}, {{demanda}}, {{data}}, {{duracao}},
{{status}}.

Marcadores de seção: {{identificacao}}, {{participantes}}, {{objetivo}},
{{resumo}}, {{decisoes}}, {{necessidades}}, {{regras}}, {{pendencias}},
{{duvidas}}, {{proximos_passos}}, {{referencias_falas}}, {{evidencias}}.

Cada item sai com o identificador (D-1, N-3, P-2...) e as falas de origem
(falante e horário); o trecho de cada fala e a conferência ficam em
{{referencias_falas}}. A ata é de uso interno: não vai ao cliente.

Uma seção com conteúdo cujo marcador foi removido vai para o fim do documento,
com aviso. Marcador desconhecido também gera aviso e fica no texto como está.
-->
# {{id}} · Ata da reunião de {{data}}

## Identificação

- **Data:** {{data}}
- **Duração:** {{duracao}}
- **Demanda:** {{demanda}}
- **Projeto:** `{{projeto}}`
- **Status:** {{status}}

{{identificacao}}

## Participantes e papéis

{{participantes}}

## Objetivo

{{objetivo}}

## Resumo

{{resumo}}

## Decisões

{{decisoes}}

## Necessidades e requisitos levantados

{{necessidades}}

## Regras de negócio citadas

{{regras}}

## Pendências

{{pendencias}}

## Dúvidas em aberto

{{duvidas}}

## Próximos passos

{{proximos_passos}}

## Referências às falas

{{referencias_falas}}

## Evidências

{{evidencias}}
