<!--
Template dos cenários de teste derivados das regras (fluxo "Cenários de
teste"). Este comentário inicial não vai para o documento.

Marcadores de campo: {{id}}, {{titulo}}, {{projeto}}, {{status}}, {{criado}},
{{analista}}, {{versao_do_codigo}}, {{origem}}.

Marcadores de seção: {{pedido}}, {{tabelas_decisao}}, {{valores_limite}},
{{cenarios}}, {{rastreabilidade}}, {{perguntas}}, {{evidencias}}.

{{tabelas_decisao}} traz cada tabela de decisão (TD-1, TD-2...): as condições
com a evidência de cada uma, as ações e as colunas (R1, R2...) com S, N ou —
(indiferente), as combinações impossíveis com a justificativa e o cenário que
exercita cada coluna. {{valores_limite}} é a tabela dos limites lidos do código,
com o limite em destaque, o valor testado e a evidência. {{cenarios}} traz cada
cenário (CT-0001, CT-0002...) com a técnica, Dado / Quando / Então, as regras e
afirmações de origem e a história ligada; cenário que depende de configuração
fora do código sai "a confirmar", com a pergunta. {{rastreabilidade}} é a
tabela cenário → regra → evidência → história. As evidências de código são as
notas de rodapé.

A exportação em CSV (uma linha por cenário, para importar em ferramentas de
teste) é montada a partir do registro, e não deste template.

Uma seção com conteúdo cujo marcador foi removido vai para o fim do documento,
com aviso. Marcador desconhecido também gera aviso e fica no texto como está.
-->
# {{id}} · {{titulo}}

Cenários de teste · Projeto `{{projeto}}` · Status: {{status}} · Criados em {{criado}} · Analista: {{analista}} · Origem: {{origem}} · Código: {{versao_do_codigo}}

## Pedido

{{pedido}}

## Tabelas de decisão

{{tabelas_decisao}}

## Valores-limite e partições

{{valores_limite}}

## Cenários de teste

{{cenarios}}

## Rastreabilidade

{{rastreabilidade}}

## Perguntas em aberto

{{perguntas}}

## Evidências

{{evidencias}}
