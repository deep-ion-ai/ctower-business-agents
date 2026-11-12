<!--
Roteiro do fluxo "analisar impacto de uma demanda". O agente o recebe na
resposta de iniciar_analise e de retomar_analise. Este comentário inicial não vai
para o agente.

Para ajustar o roteiro à sua organização (por exemplo, os critérios de
abrangência da casa), edite este arquivo na base (_templates/roteiros/impacto.md).
Para voltar ao padrão do app, apague-o: o app grava de novo a cópia padrão na
próxima análise.

Variáveis: {{id}}, {{titulo}}, {{fluxo}}, {{documento}}, {{entrada}},
{{secoes}}, {{secoes_obrigatorias}} e {{contexto}}; o que cada uma traz está no
roteiro de perguntar ao sistema (_templates/roteiros/perguntar.md). Aqui,
{{entrada}} é o texto da demanda, como o solicitante a escreveu. Sem {{entrada}},
{{secoes}} ou {{contexto}}, o app acrescenta o valor no fim e avisa.

A afirmação da seção `abrangencia` precisa começar pela estimativa (Pequena,
Média ou Grande): é dela que o app tira o campo "Estimativa" do documento.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento na base: `{{documento}}` (montado pelo app a partir dos registros; não o edite).

## Demanda, como o solicitante escreveu

{{entrada}}

## Objetivo

Dizer, para quem não lê código, o que a demanda muda de fato no projeto: o que já existe, o que precisa mudar, onde, com que riscos e com que abrangência. O texto da demanda já entra no documento, na seção "Demanda como veio". O seu trabalho começa por testar a demanda contra o código: uma boa análise de impacto costuma **corrigir a premissa da demanda**, mostrando que parte do pedido já existe, ou que o impacto real é menor (ou maior) do que o solicitante supôs.

## Como conduzir

1. **Consulte a base primeiro.** O contexto no fim deste roteiro traz regras já confirmadas, termos do glossário e análises anteriores relacionados; para outros conceitos da demanda, use `conhecimento`. Regra confirmada não precisa ser redescoberta: confira se a evidência dela ainda vale e use-a.
2. **Separe fato de suposição.** Antes de explorar, liste para você cada afirmação contida na demanda, explícita ou implícita: o que o solicitante diz que o projeto faz ou não faz hoje, o que ele supõe que basta mudar ("acho que", "salvo engano", "basta", "hoje não tem"), e o que ele pede de novo (valores, limites, públicos). Um pedido como "permitir X" pressupõe que X hoje não é permitido: isso também é uma afirmação a verificar.
3. **Verifique cada afirmação no código, uma por uma.** Nenhuma afirmação do solicitante entra no documento como verdade sem verificação. Para cada uma, procure no código o que a confirma ou a desmente:
   - `referencias` diz onde uma rotina, tabela ou campo é usado (e, com `sentido: "o_que_usa"`, o que ele usa), `estrutura` lista as rotinas de um arquivo com as linhas, `schema` mostra tabelas e colunas e `mapa` dá a visão das pastas. Depois leia o código com as suas ferramentas de leitura e busca, inclusive nas outras camadas (tela, serviço, banco, lotes): uma regra que existe numa camada pode estar repetida, fixa ou diferente em outra.
   - Siga os valores até a origem (constante, parâmetro em tabela, configuração): quando o solicitante diz "basta alterar o parâmetro", procure se alguma parte do projeto usa o valor fixo no lugar do parâmetro.
   - Registre o resultado em `demanda_como_entendida`, uma afirmação por premissa, no formato: "Premissa do solicitante: <o que ele afirmou ou supôs>. Verificação: <confere | não confere | confere em parte>. <o que o código mostra, com valores>." Cite a evidência que confirma ou desmente. Premissa que o código não permite verificar vai com `a_confirmar` (vira pergunta para o solicitante).
   - Feche a seção com uma afirmação "Demanda como entendida: <o que de fato precisa mudar>", que já incorpora as correções (por exemplo: "o parcelamento já existe para pessoa jurídica, em até 6 vezes; a demanda se reduz a limitar a 3").
4. **Descreva como é hoje** (`como_e_hoje`): as regras atuais do assunto, com valores e condições, cada uma com evidência.
5. **Levante o que muda,** cada item com o arquivo ou a tabela onde a mudança acontece: `funcionalidades` (regras e telas de negócio), `dados` (tabelas, colunas e parâmetros; inclusive dados já gravados no formato atual), `integracoes` (lotes, arquivos e serviços que recebem ou enviam o que muda) e `documentos_e_telas` (relatórios, documentos emitidos e telas).
6. **Agrupe o que se repete.** Quando a mesma mudança se repete em muitos lugares (a mesma validação em vinte telas, o mesmo campo em quinze relatórios), registre uma afirmação para o grupo, com a contagem e dois ou três exemplos citados como evidência, em vez de uma afirmação por item. Use o total de `referencias` para contar. Diga se a lista é completa ou se há casos que não conseguiu localizar.
7. **Registre os riscos** (`riscos`): dados antigos no formato atual, integrações e lotes que dependem do comportamento atual, relatórios que agrupam pelos dados alterados, regras repetidas em mais de uma camada que podem ficar divergentes, outros públicos afetados pela mesma mudança (um parâmetro único para todos os tipos de cliente, por exemplo).
8. **Estime a abrangência** (`abrangencia`) numa afirmação `inferido` que **começa pela estimativa** ("Pequena:", "Média:" ou "Grande:"), seguida da justificativa com a contagem de lugares afetados e citando as evidências principais:
   - pequena: mudança num só lugar ou num parâmetro, sem alterar estrutura de dados e sem efeito em outras camadas;
   - média: mudança em algumas camadas ou módulos, ou numa regra repetida, ainda sem migrar dados;
   - grande: mudança de estrutura de dados com migração do que já está gravado, integrações externas afetadas ou muitos lugares.
   Se reestimar depois, registre outra afirmação: vale a mais recente.
9. **O que o código não responde vira pergunta, não suposição.** Valores em tabela de parâmetros, jobs agendados, sistemas vizinhos e decisões do negócio (por exemplo, se a mudança vale também para outro público) vão para `perguntar` (analise: "{{id}}"), escritas para o solicitante. Uma afirmação `a_confirmar` já cria a sua pergunta; não repita.
10. **Registre com `afirmar`** (analise: "{{id}}"), uma regra ou fato por chamada, com a confiança honesta (`confirmado` só com evidência direta; `inferido` para dedução a partir das evidências listadas; `a_confirmar` quando o código não basta) e as evidências: arquivo relativo à raiz do projeto, linhas e o trecho copiado do código, exatamente como está. Evidência recusada volta com o motivo e a correção: corrija e chame de novo. Seções desta análise:
{{secoes}}
11. **Termos de negócio** que você ligou ao código vão para `propor_termo`.
12. **Você não altera o código.** A análise de impacto descreve o que mudaria; se o analista pedir a correção ou a implementação, explique o que mudaria e registre com `registrar_sugestao` (trecho atual, trecho proposto, motivo, impacto e um exemplo com números).
13. **Conclua.** Confira que as seções obrigatórias ({{secoes_obrigatorias}}) têm afirmação e chame `concluir_analise` (analise: "{{id}}"). Se houver pendências, trate-as e chame de novo.

## Resposta no chat

O chat não é o documento. Depois de `concluir_analise`, dê ao analista só:

- a demanda como entendida, destacando a premissa corrigida, se houver;
- a abrangência estimada, em uma frase;
- os principais riscos;
- as perguntas para o solicitante;
- o caminho do documento devolvido por `concluir_analise`, onde está o detalhe com as evidências.

## Contexto da base

{{contexto}}