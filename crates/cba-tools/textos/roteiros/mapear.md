<!--
Roteiro do fluxo "mapear funcionalidades". O agente o recebe na resposta de
iniciar_analise e de retomar_analise. Este comentário inicial não vai para o
agente.

Para ajustar o roteiro à sua organização (por exemplo, as áreas da casa ou o
nível de detalhe das descrições), edite este arquivo na base
(_templates/roteiros/mapear.md). Para voltar ao padrão do app, apague-o: o app
grava de novo a cópia padrão na próxima análise.

Variáveis: {{id}}, {{titulo}}, {{fluxo}}, {{documento}}, {{entrada}},
{{secoes}}, {{secoes_obrigatorias}} e {{contexto}}; o que cada uma traz está no
roteiro de perguntar ao sistema (_templates/roteiros/perguntar.md). Aqui,
{{entrada}} é o escopo do mapeamento (o projeto inteiro, uma pasta ou uma área)
e a orientação do analista, e {{contexto}} traz o catálogo atual.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento na base: `{{documento}}` (montado pelo app a partir dos registros; não o edite).

## Pedido do analista

{{entrada}}

## Objetivo

Dar ao analista o inventário das funcionalidades do sistema **em linguagem de negócio**: quais existem, em que área ficam e por onde começam. Cada funcionalidade é um objetivo de quem usa ("Parcelar fatura", e não `PKG_FAT.PRC_PARCELA`), com os pontos de entrada que a iniciam, cada um com evidência no código. Você não documenta cada funcionalidade em detalhe: isso é o fluxo "Explicar funcionalidade", que o analista abre depois a partir do catálogo.

## Como conduzir

1. **Veja os pontos de entrada.** Chame `pontos_de_entrada` (analise: "{{id}}"): o app já achou no mapa estrutural as telas, os endpoints, os lotes, as rotinas de banco públicas, os serviços e as integrações do escopo. É a lista que o app vai conferir no fim. Use `mapa`, `estrutura`, `referencias` e `schema` para entender o que cada um faz e quem chama quem; leia o código só onde o nome não basta.
2. **Agrupe em funcionalidades pelo objetivo do usuário,** não pelo ponto de entrada: a tela de cancelamento e a procedure que ela chama são a mesma funcionalidade ("Cancelar fatura"). Uma funcionalidade tem um ou mais pontos de entrada; um ponto fica numa funcionalidade só (as outras que o usam são `relacionadas`).
3. **Agrupe as funcionalidades em áreas.** Use as áreas do projeto (estão no contexto, quando o `projeto.md` as declara); sem elas, proponha áreas de negócio (Cobrança, Parcelamento, Cadastro) e diga no chat que o analista pode aceitá-las ou mudá-las na revisão.
4. **Mantenha os identificadores do catálogo.** O contexto lista as funcionalidades que já estão no catálogo. Para uma que continua existindo, registre com `id` (o F-<SIGLA>-NNN dela), mesmo que o nome mude. Uma funcionalidade nova não leva `id`: o app reserva o próximo. Não reaproveite o identificador de uma funcionalidade para outra.
5. **Registre com `registrar_funcionalidade`** (analise: "{{id}}"), uma por chamada: `nome` de negócio, `area`, `descricao` em uma ou duas frases (o que ela faz para quem usa), `atores` (quem usa: o atendente, o cliente, o lote noturno), `pontos_de_entrada` (cada um com o `ponto`, pela chave que `pontos_de_entrada` mostra, e a `evidencia` do trecho que o define: a declaração da tela, da rotina, do endpoint), os principais `dados_lidos` e `dados_gravados` (tabelas, arquivos) e as `relacionadas`. Sem ponto de entrada com evidência válida, a funcionalidade é recusada: corrija a evidência e chame de novo.
6. **O que o código não responde vira pergunta,** com `perguntar`: menu montado por tabela, perfil configurado fora do código, job agendado fora do repositório, endpoint que o front chama e o backend não declara. Um ponto de entrada que você conhece por fora da lista do mapa entra com a evidência que o sustenta.
7. **Trate todos os pontos do escopo antes de concluir.** Chame `pontos_de_entrada` com `so_sem_tratamento: true`. Para cada um: atribua a uma funcionalidade (`tratar_ponto_de_entrada` com `atribuir`), crie uma funcionalidade nova ou justifique por que fica sem funcionalidade (`tratar_ponto_de_entrada` com `justificar` e o motivo: `codigo_morto`, `utilitario` ou `outro`). Utilitário é a rotina que só serve às outras (ler parâmetro, gravar log); diga a quais.
8. **Em projetos grandes, vá por partes.** Se o escopo for grande, mapeie uma área ou pasta de cada vez e registre na memória o que já foi coberto.
9. **Você não altera o código** nem escreve o catálogo: ele é montado pelo app a partir dos registros e atualizado quando o analista aprova a análise. Seções desta análise que recebem afirmações:
{{secoes}}
10. **Mantenha a memória da análise** com `memorizar` (analise: "{{id}}"): as áreas combinadas com o analista (`combinados`), as pastas e áreas já cobertas e o que falta (`pendencias`), os agrupamentos que você considerou e descartou, com o motivo (`descartado`), e onde parou (`onde_parou`, com `modo: "substituir"`). Atualize depois de cada área e antes de concluir. Não repita as funcionalidades já registradas: cite-as pelo identificador.
11. **Conclua** com `concluir_analise` (analise: "{{id}}"). O app recusa enquanto houver ponto de entrada sem tratamento e lista quais: trate-os e chame de novo. Seções obrigatórias de afirmação: {{secoes_obrigatorias}}.

## Resposta no chat

O chat não é o documento. Depois de `concluir_analise`, dê ao analista só:

- as áreas, com o número de funcionalidades de cada uma;
- as funcionalidades novas e as que já estavam no catálogo (F-<SIGLA>-NNN e nome);
- os pontos de entrada que ficaram sem funcionalidade, com o motivo;
- as perguntas em aberto;
- o caminho do documento devolvido por `concluir_analise`. Na revisão, o analista aceita, edita, junta, divide ou descarta cada funcionalidade; ao aprovar, as aceitas entram no catálogo do projeto.

## Contexto da base

{{contexto}}
