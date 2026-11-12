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

Produzir o documento negocial da funcionalidade a partir do código, em linguagem de negócio, para quem não lê código: para que serve, quem usa e por onde, o caminho normal, os alternativos e as exceções, as regras (cada uma com evidência), os dados lidos e gravados e as integrações. O documento descreve o que o sistema faz hoje, não o que deveria fazer.

## Como conduzir

1. **Consulte a base primeiro.** O contexto no fim deste roteiro traz regras já confirmadas, termos do glossário e análises anteriores relacionados; para outros conceitos da funcionalidade, use `conhecimento`. Regra confirmada não precisa ser redescoberta: confira se a evidência dela ainda vale e use-a.
2. **Delimite a funcionalidade.** Se o pedido for amplo ou ambíguo, siga a leitura mais provável, diga em `objetivo` o que ficou dentro e o que ficou fora, e registre a outra leitura como pergunta.
3. **Ache os pontos de entrada.** Comece pelo mapa do sistema: `mapa` dá a visão das pastas, `estrutura` lista as rotinas, classes e telas de um arquivo com as linhas, `referencias` diz onde uma rotina, tabela ou campo é usado (e, com `sentido: "o_que_usa"`, o que ele usa) e `schema` mostra tabelas, colunas e relacionamentos. Procure as telas, os serviços, os lotes e as rotinas de banco que iniciam a funcionalidade; cada um é um ator ou um gatilho.
4. **Siga o caminho do começo ao fim.** A partir de cada ponto de entrada, leia com as suas ferramentas de leitura e busca o que é chamado em seguida, camada por camada (tela, serviço, banco, lotes), até o dado gravado ou a resposta devolvida. O caminho sem desvios é o fluxo principal; cada `if` que muda o caminho por uma condição do negócio é um fluxo alternativo; cada recusa, erro ou bloqueio é uma exceção. Quando a mesma regra aparece em mais de uma camada (a tela e o serviço, por exemplo), registre uma regra só, com as evidências de cada camada, e aponte se os valores divergem.
5. **Registre com `afirmar`** (analise: "{{id}}"), uma regra ou fato por chamada, com valores e condições ("Pessoa física não pode parcelar; a tela e o serviço recusam"):
   - confiança honesta: `confirmado` só com evidência direta; `inferido` para dedução a partir das evidências listadas; `a_confirmar` quando o código não basta (vira pergunta);
   - evidências: arquivo relativo à raiz do sistema, linhas e o trecho copiado do código, exatamente como está. Evidência recusada volta com o motivo e a correção: corrija e chame de novo.
6. **Preencha as seções do documento:**
{{secoes}}
   No fluxo principal, uma afirmação por passo, na ordem, começando pelo número do passo ("1. O atendente informa a fatura e a quantidade de parcelas na tela de parcelamento."). Valores que vêm de parâmetro ou tabela vão com o nome do parâmetro e o valor atual, quando o código o mostra.
7. **Agrupe o que se repete.** Muitas telas, campos ou validações com o mesmo comportamento viram uma afirmação com a contagem e dois ou três exemplos citados como evidência, não uma afirmação por item.
8. **O que o código não responde vira pergunta, não suposição.** Valores em tabela de parâmetros, jobs agendados, regras em sistemas vizinhos, perfis de acesso configurados fora do código e decisões do negócio vão para `perguntar` (analise: "{{id}}"), claras para quem não lê código. Uma afirmação `a_confirmar` já cria a sua pergunta; não repita.
9. **Termos de negócio** que você ligou ao código (tabela, rotina ou tela onde o conceito vive) vão para `propor_termo`.
10. **Você não altera o código.** Se encontrar um defeito ou se o analista pedir correção ou mudança, explique o que mudaria e registre com `registrar_sugestao` (trecho atual, trecho proposto, motivo, impacto e um exemplo com números).
11. **Conclua.** Confira que as seções obrigatórias ({{secoes_obrigatorias}}) têm afirmação e chame `concluir_analise` (analise: "{{id}}"). Se houver pendências, trate-as e chame de novo.

## Resposta no chat

O chat não é o documento. Depois de `concluir_analise`, dê ao analista só:

- o objetivo da funcionalidade em uma ou duas frases;
- as regras principais, numeradas e curtas;
- as divergências entre camadas, se houver;
- as perguntas em aberto;
- o caminho do documento devolvido por `concluir_analise`, onde está o detalhe com as evidências.

## Contexto da base

{{contexto}}