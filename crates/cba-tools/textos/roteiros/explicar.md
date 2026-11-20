<!--
Roteiro do fluxo "explicar funcionalidade". O agente o recebe na resposta de
iniciar_analise e de retomar_analise. Este comentário inicial não vai para o
agente.

Para ajustar o roteiro à sua organização (por exemplo, ao padrão de documento
negocial da casa), edite este arquivo na base (_templates/roteiros/explicar.md).
Para voltar ao padrão do app, apague-o: o app grava de novo a cópia padrão na
próxima análise.

Variáveis: {{id}}, {{titulo}}, {{fluxo}}, {{documento}}, {{entrada}},
{{secoes}}, {{secoes_obrigatorias}} e {{contexto}}; o que cada uma traz está no
roteiro de perguntar ao sistema (_templates/roteiros/perguntar.md). Aqui,
{{entrada}} é a funcionalidade que o analista quer documentada. Sem {{entrada}},
{{secoes}} ou {{contexto}}, o app acrescenta o valor no fim e avisa.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento na base: `{{documento}}` (montado pelo app a partir dos registros; não o edite).

## Funcionalidade pedida pelo analista

{{entrada}}

## Objetivo

Produzir o documento negocial da funcionalidade a partir do código, em linguagem de negócio, para quem não lê código: para que serve, quem usa e por onde, o caminho normal, os alternativos e as exceções, as regras (cada uma com evidência), os dados lidos e gravados e as integrações, com o fluxograma do processo em raias por ator e o modelo de dados conceitual. O documento descreve o que o projeto faz hoje, não o que deveria fazer.

## Como conduzir

1. **Consulte a base primeiro.** O contexto no fim deste roteiro traz regras já confirmadas, termos do glossário e análises anteriores relacionados; para outros conceitos da funcionalidade, use `conhecimento`. Regra confirmada não precisa ser redescoberta: confira se a evidência dela ainda vale e use-a. **Documentação publicada:** quando o contexto menciona documentos do repositório de docs (marcados [doc. publicada], com o repositório, o commit e a confiança), consulte-os antes de explorar o código: chame `conhecimento` com os conceitos e `externos: "incluir"`, que devolve o trecho de cada documento e o caminho para ler o completo. Só o título, no contexto, não diz o que o documento afirma. Para outros documentos, use `externos: "somente"`. Ela é pista, não prova: use-a para achar o código e confirme no código antes de afirmar. A evidência é sempre o trecho do código, nunca o documento, que é recusado como evidência. Quando a documentação publicada ajudar, diga isso ao analista; se ela divergir do código, vale o código, e a divergência vira pergunta.
2. **Delimite a funcionalidade.** Se o pedido for amplo ou ambíguo, siga a leitura mais provável, diga em `objetivo` o que ficou dentro e o que ficou fora, e registre a outra leitura como pergunta.
3. **Ache os pontos de entrada.** Comece pelo mapa do projeto: `mapa` dá a visão das pastas, `estrutura` lista as rotinas, classes e telas de um arquivo com as linhas, `referencias` diz onde uma rotina, tabela ou campo é usado (e, com `sentido: "o_que_usa"`, o que ele usa) e `schema` mostra tabelas, colunas e relacionamentos. Procure as telas, os serviços, os lotes e as rotinas de banco que iniciam a funcionalidade; cada um é um ator ou um gatilho.
4. **Siga o caminho do começo ao fim.** A partir de cada ponto de entrada, leia com as suas ferramentas de leitura e busca o que é chamado em seguida, camada por camada (tela, serviço, banco, lotes), até o dado gravado ou a resposta devolvida. O caminho sem desvios é o fluxo principal; cada `if` que muda o caminho por uma condição do negócio é um fluxo alternativo; cada recusa, erro ou bloqueio é uma exceção. Quando a mesma regra aparece em mais de uma camada (a tela e o serviço, por exemplo), registre uma regra só, com as evidências de cada camada, e aponte se os valores divergem.
5. **Registre com `afirmar`** (analise: "{{id}}"), uma regra ou fato por chamada, com valores e condições ("Pessoa física não pode parcelar; a tela e o serviço recusam"):
   - confiança honesta: `confirmado` só com evidência direta; `inferido` para dedução a partir das evidências listadas; `a_confirmar` quando o código não basta (vira pergunta);
   - evidências: arquivo relativo à raiz do projeto, linhas e o trecho copiado do código, exatamente como está. Evidência recusada volta com o motivo e a correção: corrija e chame de novo.
6. **Preencha as seções do documento:**
{{secoes}}
   No fluxo principal, uma afirmação por passo, na ordem, começando pelo número do passo ("1. O atendente informa a fatura e a quantidade de parcelas na tela de parcelamento."). Valores que vêm de parâmetro ou tabela vão com o nome do parâmetro e o valor atual, quando o código o mostra.
7. **Ciclo de vida.** Se o pedido é o ciclo de vida de uma entidade ("em que situações um contrato pode ser cancelado?"), ou se a funcionalidade muda um campo de status, monte o diagrama de estados com `registrar_diagrama` (analise: "{{id}}", tipo `estados`, seção `fluxo_principal`):
   - localize o campo de status com `schema` (coluna, `CHECK`, valor padrão), `referencias` (quem lê e quem grava o campo) e `conhecimento` (termo do glossário da entidade);
   - os estados são os valores possíveis do campo: a `chave` é o valor como o código grava (`ABERTA`), o `rotulo` é o nome de negócio (do glossário, se houver) e a descrição diz o que o estado significa; marque o inicial (`inicio`, em geral o valor padrão) e os finais (`fim`), cada um com a evidência de onde o valor aparece;
   - cada rotina que grava o campo é uma transição: `de` e `para`, o evento ou a rotina que dispara (`rotulo`), a `condicao` que o código confere antes de gravar e os `efeitos` (o que mais grava ou dispara), com a evidência do trecho que grava o valor;
   - a resposta devolve os achados que o app calculou (estado inalcançável, sem saída e transição sem condição): confira cada um no código antes de aceitá-lo; o que o código não explica vira pergunta.
8. **Desenhe o processo e os dados com `registrar_diagrama`** (analise: "{{id}}"). Não escreva Mermaid: registre os elementos com evidência e o app gera as figuras.
   - **Fluxo do processo** (tipo `fluxo`, secao `fluxo_processo`): um passo `inicio`, os passos do fluxo principal na ordem, um nó `decisao` para cada condição que leva a um fluxo alternativo ou a uma exceção (os caminhos saem com a condição no rótulo, como "sim" e "não") e um passo `fim` para cada desfecho (concluído, recusado). Cada passo tem a `raia` do ator que o executa, com o mesmo nome em todos os passos dele ("Atendente", "Tela de cancelamento", "Banco de dados", "Lote mensal"). Use rótulos curtos, de negócio, e cite a evidência do trecho de cada passo e de cada caminho.
   - **Modelo de dados conceitual** (tipo `dados`, secao `modelo_dados`): uma entidade por tabela de negócio que a funcionalidade lê ou grava, com o nome do glossário como rótulo (consulte `conhecimento`; se a tabela não tem termo, proponha um com `propor_termo`), a `tabela` de origem e só os `atributos` de negócio (sem chaves artificiais e colunas de controle). Cada relação tem a `cardinalidade` e a `origem`: `chave_estrangeira` quando os scripts declaram a constraint (veja `schema`; a evidência é a constraint) ou `join` quando a ligação só aparece num join ou numa gravação do código, com `confianca` `inferido` e a evidência do trecho. Relação inferida que o negócio precisa confirmar também vira pergunta.
9. **Agrupe o que se repete.** Muitas telas, campos ou validações com o mesmo comportamento viram uma afirmação com a contagem e dois ou três exemplos citados como evidência, não uma afirmação por item.
10. **O que o código não responde vira pergunta, não suposição.** Valores em tabela de parâmetros, jobs agendados, regras em sistemas vizinhos, perfis de acesso configurados fora do código e decisões do negócio vão para `perguntar` (analise: "{{id}}"), claras para quem não lê código. Uma afirmação `a_confirmar` já cria a sua pergunta; não repita.
11. **Termos de negócio** que você ligou ao código (tabela, rotina ou tela onde o conceito vive) vão para `propor_termo`.
12. **Você não altera o código.** Se encontrar um defeito ou se o analista pedir correção ou mudança, explique o que mudaria e registre com `registrar_sugestao` (trecho atual, trecho proposto, motivo, impacto e um exemplo com números).
13. **Mantenha a memória da análise** com `memorizar` (analise: "{{id}}"). O documento guarda o resultado; a memória guarda o contexto de trabalho que uma conversa nova precisaria para continuar, se esta acabar: o que já foi investigado (`investigado`) e descartado, com o motivo (`descartado`), as hipóteses em aberto (`hipoteses`), onde está o código relevante, inclusive repositórios fora do projeto (`codigo`), o que o analista orientou (`combinados`), o que ficou pendente (`pendencias`) e onde você parou (`onde_parou`, com `modo: "substituir"`). Atualize nos marcos: depois de uma descoberta relevante, quando o analista der uma orientação e antes de concluir. Seja curto; não repita afirmações, perguntas e termos já registrados.
14. **Conclua.** Confira que as seções obrigatórias ({{secoes_obrigatorias}}) têm afirmação e chame `concluir_analise` (analise: "{{id}}"). Se houver pendências, trate-as e chame de novo.

## Resposta no chat

O chat não é o documento. Depois de `concluir_analise`, dê ao analista só:

- o objetivo da funcionalidade em uma ou duas frases;
- as regras principais, numeradas e curtas;
- as divergências entre camadas, se houver;
- as perguntas em aberto;
- as fontes externas usadas (documentação publicada, com o repositório e o commit), se houver;
- o caminho do documento devolvido por `concluir_analise`, onde está o detalhe com as evidências.

## Contexto da base

{{contexto}}