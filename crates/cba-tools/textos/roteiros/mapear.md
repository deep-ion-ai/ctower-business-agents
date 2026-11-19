# Roteiro do mapeamento de funcionalidades

O mapa de funcionalidades é um cadastro do projeto, não uma análise: não há análise A-NNNN para citar, e as ferramentas abaixo agem direto no mapa. Ele é montado pelo app em `{{documento}}` a partir dos seus registros; não edite esse arquivo.

## Recorte desta sessão

Mapear {{escopo}}.

{{orientacao}}

## Objetivo

Dar ao analista o inventário das funcionalidades do sistema **em linguagem de negócio**: quais existem, em que área ficam e por onde começam. Cada funcionalidade é um objetivo de quem usa ("Parcelar fatura", e não `PKG_FAT.PRC_PARCELA`), com os pontos de entrada que a iniciam, cada um com evidência no código. Você não documenta cada funcionalidade em detalhe: isso é o fluxo "Explicar funcionalidade", que o analista abre depois a partir do mapa.

## Como conduzir

1. **Veja os pontos de entrada.** Chame `pontos_de_entrada`{{argumentos}}: o app já achou no mapa estrutural as telas, os endpoints, os lotes, as rotinas de banco públicas, os serviços e as integrações. É a lista que o app vai conferir no fim. Use `mapa`, `estrutura`, `referencias` e `schema` para entender o que cada um faz e quem chama quem; leia o código só onde o nome não basta.
2. **Agrupe em funcionalidades pelo objetivo do usuário,** não pelo ponto de entrada: a tela de cancelamento e a procedure que ela chama são a mesma funcionalidade ("Cancelar fatura"). Uma funcionalidade tem um ou mais pontos de entrada; um ponto fica numa funcionalidade só (as outras que o usam são `relacionadas`).
3. **Agrupe as funcionalidades em áreas.** Use as áreas do projeto (estão no contexto, quando o `projeto.md` as declara); sem elas, proponha áreas de negócio (Cobrança, Parcelamento, Cadastro) e diga no chat que o analista pode mudá-las na revisão.
4. **Continue o mapa que já existe.** O contexto lista as funcionalidades já mapeadas. Para atualizar uma, registre com `id` (o F-<SIGLA>-NNN dela), mesmo que o nome mude: a nova versão substitui a anterior, com todos os pontos de entrada, e volta para a revisão do analista. Uma funcionalidade nova não leva `id`: o app reserva o próximo. Não reaproveite o identificador de uma funcionalidade para outra.
5. **Registre com `registrar_funcionalidade`,** uma por chamada: `nome` de negócio, `area`, `descricao` em uma ou duas frases (o que ela faz para quem usa), `atores` (quem usa: o atendente, o cliente, o lote noturno), `pontos_de_entrada` (cada um com o `ponto`, pela chave que `pontos_de_entrada` mostra, e a `evidencia` do trecho que o define: a declaração da tela, da rotina, do endpoint), os principais `dados_lidos` e `dados_gravados` (tabelas, arquivos) e as `relacionadas`. Sem ponto de entrada com evidência válida, a funcionalidade é recusada: corrija a evidência e chame de novo.
6. **O que o código não responde, pergunte ao analista no chat:** menu montado por tabela, perfil configurado fora do código, job agendado fora do repositório, endpoint que o front chama e o backend não declara. Um ponto de entrada que você conhece por fora da lista do mapa entra com a evidência que o sustenta.
7. **Trate todos os pontos do recorte antes de concluir.** Chame `pontos_de_entrada` com `so_sem_tratamento: true`. Para cada um: atribua a uma funcionalidade (`atribuir`), crie uma funcionalidade nova ou justifique por que fica sem funcionalidade (`justificar` e o motivo: `codigo_morto`, `utilitario` ou `outro`). Utilitário é a rotina que só serve às outras (ler parâmetro, gravar log); diga a quais.
   - **Trate em lote, não um por chamada.** `tratar_ponto_de_entrada` recebe `pontos`, uma lista de itens (até 500), cada um com `ponto`, `tratamento` e os campos dele (`funcionalidade` no `atribuir`; `motivo` e `justificativa` no `justificar`). Para justificar de uma vez os pontos sem tratamento de um arquivo ou de uma pasta com o mesmo motivo (os utilitários, por exemplo), use `justificar_padrao` com `arquivo` ou `prefixo` da chave. Tudo vai numa gravação só; os itens válidos são aplicados mesmo que outros sejam recusados, e a resposta diz, item por item, o que foi aplicado e por que o resto foi recusado: corrija e mande de novo só os recusados.
   - **Cite o ponto pela chave** que `pontos_de_entrada` mostra. O nome do símbolo também vale e, num endpoint, a rota com ou sem o método (`GET /api/faturas/{id}`), mesmo com outro nome de parâmetro (`{idFatura}`, `:id`). Se a citação casar com mais de um ponto, a recusa lista os candidatos; se não casar com nenhum, sugere chaves parecidas.
8. **Em projetos grandes, vá por partes:** uma área ou pasta de cada vez (`pasta` ou `area` em `pontos_de_entrada` e `concluir_mapeamento`). O mapa guarda tudo o que já foi registrado, então uma sessão nova continua de onde a anterior parou.
9. **Você não altera o código** nem escreve o documento do mapa: ele é montado pelo app a partir dos registros.
10. **Conclua** com `concluir_mapeamento`{{argumentos}}. O app recusa enquanto houver ponto de entrada do recorte sem tratamento e lista quais: trate-os e chame de novo.

## Resposta no chat

Depois de `concluir_mapeamento`, dê ao analista só:

- as áreas, com o número de funcionalidades de cada uma;
- as funcionalidades novas e as atualizadas (F-<SIGLA>-NNN e nome);
- os pontos de entrada que ficaram sem funcionalidade, com o motivo;
- o que você perguntou e ficou sem resposta.

A revisão é feita pelo analista na tela "Mapa de funcionalidades" do app: ele aceita, edita, junta, divide ou descarta cada funcionalidade.

## Contexto

{{contexto}}
