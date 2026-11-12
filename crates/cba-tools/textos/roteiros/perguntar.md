<!--
Roteiro do fluxo "perguntar ao sistema". O agente o recebe na resposta de
iniciar_analise e de retomar_analise. Este comentário inicial não vai para o
agente.

Para ajustar o roteiro à sua organização, edite este arquivo na base
(_templates/roteiros/perguntar.md). Para voltar ao padrão do app, apague-o: o app
grava de novo a cópia padrão na próxima análise.

Variáveis:
  {{id}}                  identificador da análise (A-0001)
  {{titulo}}              título da análise
  {{fluxo}}               nome do fluxo ("pergunta ao sistema")
  {{documento}}           caminho do documento da análise, relativo à base
  {{entrada}}             a pergunta do analista, como citação Markdown
  {{secoes}}              lista das seções do documento em que o agente registra,
                          com o título, se é obrigatória e o que vai em cada uma
  {{secoes_obrigatorias}} as seções obrigatórias, separadas por vírgula
  {{contexto}}            regras, termos, memórias e análises da base
                          relacionados à pergunta

Sem {{entrada}}, {{secoes}} ou {{contexto}}, o app acrescenta o valor no fim e
avisa. Variável desconhecida fica no texto como está, com aviso.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento na base: `{{documento}}` (montado pelo app a partir dos registros; não o edite).

## Pergunta do analista

{{entrada}}

## Objetivo

Responder à pergunta a partir do código do projeto, em linguagem de negócio, para quem não lê código. O resultado é um documento com a resposta direta, as regras que a sustentam (cada uma com evidência), um exemplo com números e as perguntas que o código não responde.

## Como conduzir

1. **Consulte a base primeiro.** O contexto no fim deste roteiro traz regras já confirmadas, termos do glossário e análises anteriores relacionados; para outros conceitos da pergunta, use `conhecimento`. Regra confirmada não precisa ser redescoberta: confira se a evidência dela ainda vale e use-a. Os termos do glossário dizem onde cada conceito de negócio vive no código.
2. **Entenda o que foi perguntado.** Identifique os conceitos de negócio da pergunta (por exemplo "multa", "atraso", "fatura") e o que o analista quer saber: um valor, uma condição, uma sequência de passos, quem decide. Se a pergunta for ambígua, siga a leitura mais provável e registre a outra como pergunta em aberto.
3. **Localize no código o que decide o comportamento.** Comece pelo mapa do projeto: `referencias` diz onde uma rotina, tabela ou campo é usado (e, com `sentido: "o_que_usa"`, o que ele usa), `estrutura` lista as rotinas de um arquivo com as linhas, `schema` mostra tabelas, colunas e relacionamentos e `mapa` dá a visão das pastas. Depois use as suas ferramentas de leitura e busca nas pastas do projeto: procure os termos de negócio e seus sinônimos em nomes de rotinas, tabelas, colunas, parâmetros e telas. Leia o trecho que calcula ou decide, não só o que o chama. Siga os valores até a origem (constante, parâmetro em tabela, configuração).
4. **Registre cada regra com `afirmar`** (analise: "{{id}}"), uma regra ou fato por chamada, com valores e condições ("Passada a carência de 5 dias, a multa é de 2% sobre o valor original"):
   - confiança honesta: `confirmado` só com evidência direta; `inferido` para dedução a partir das evidências listadas; `a_confirmar` quando o código não basta (vira pergunta);
   - evidências: arquivo relativo à raiz do projeto, linhas e o trecho copiado do código, exatamente como está. Evidência recusada volta com o motivo e a correção: corrija e chame de novo.
5. **Preencha as seções do documento:**
{{secoes}}
6. **Monte o exemplo com números.** Escolha valores plausíveis (uma fatura de R$ 1.000,00 paga com 10 dias de atraso, por exemplo), aplique as regras registradas passo a passo, com os arredondamentos do código, e registre em `exemplo` com confiança `inferido`, citando as evidências das regras usadas. O exemplo é o que o solicitante usa para conferir a resposta.
7. **O que o código não responde vira pergunta, não suposição.** Valores em tabela de parâmetros, jobs agendados, regras em sistemas vizinhos ou decisões do negócio vão para `perguntar` (analise: "{{id}}"), claras para quem não lê código. Uma afirmação `a_confirmar` já cria a sua pergunta; não repita.
8. **Termos de negócio** que você ligou ao código (tabela, rotina ou tela onde o conceito vive) vão para `propor_termo`.
9. **Você não altera o código.** Se o analista pedir correção ou mudança, explique o que mudaria e registre com `registrar_sugestao` (trecho atual, trecho proposto, motivo, impacto e um exemplo com números).
10. **Conclua.** Confira que as seções obrigatórias ({{secoes_obrigatorias}}) têm afirmação e chame `concluir_analise` (analise: "{{id}}"). Se houver pendências, trate-as e chame de novo.

## Resposta no chat

O chat não é o documento. Depois de `concluir_analise`, dê ao analista só:

- a resposta em uma ou duas frases;
- as regras numeradas, curtas;
- o exemplo com números;
- as perguntas em aberto;
- o caminho do documento devolvido por `concluir_analise`, onde está o detalhe com as evidências.

## Contexto da base

{{contexto}}