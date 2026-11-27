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

1. **Consulte a base primeiro.** O contexto no fim deste roteiro traz regras já confirmadas, termos do glossário e análises anteriores relacionados; para outros conceitos da demanda, use `conhecimento`. Regra confirmada não precisa ser redescoberta: confira se a evidência dela ainda vale e use-a. **Documentação publicada:** quando o contexto menciona documentos do repositório de docs (marcados [doc. publicada], com o repositório, o commit e a confiança), consulte-os antes de explorar o código: chame `conhecimento` com `externos: "incluir"`, um conceito por chamada (`juros`, depois `carência`), que devolve o trecho de cada documento e o caminho para ler o completo. Com várias palavras, a busca exige todas, e um documento que cita todas (uma análise) esconde as regras de cada conceito. Só o título, no contexto, não diz o que o documento afirma. Para outros documentos, use `externos: "somente"`. Ela é pista, não prova: use-a para achar o código e confirme no código antes de afirmar. A evidência é sempre o trecho do código, nunca o documento, que é recusado como evidência. Quando a documentação publicada ajudar, diga isso ao analista; se ela divergir do código, vale o código, e a divergência vira pergunta.
2. **Separe fato de suposição.** Antes de explorar, liste para você cada afirmação contida na demanda, explícita ou implícita: o que o solicitante diz que o projeto faz ou não faz hoje, o que ele supõe que basta mudar ("acho que", "salvo engano", "basta", "hoje não tem"), e o que ele pede de novo (valores, limites, públicos). Um pedido como "permitir X" pressupõe que X hoje não é permitido: isso também é uma afirmação a verificar.
3. **Verifique cada afirmação no código, uma por uma.** Nenhuma afirmação do solicitante entra no documento como verdade sem verificação. Para cada uma, procure no código o que a confirma ou a desmente:
   - `referencias` diz onde uma rotina, tabela ou campo é usado (e, com `sentido: "o_que_usa"`, o que ele usa), `estrutura` lista as rotinas de um arquivo com as linhas, `schema` mostra tabelas e colunas e `mapa` dá a visão das pastas. Depois leia o código com as suas ferramentas de leitura e busca, inclusive nas outras camadas (tela, serviço, banco, lotes): uma regra que existe numa camada pode estar repetida, fixa ou diferente em outra.
   - Siga os valores até a origem (constante, parâmetro em tabela, configuração): quando o solicitante diz "basta alterar o parâmetro", procure se alguma parte do projeto usa o valor fixo no lugar do parâmetro.
   - Registre o resultado em `demanda_como_entendida`, uma afirmação por premissa, no formato: "Premissa do solicitante: <o que ele afirmou ou supôs>. Verificação: <confere | não confere | confere em parte>. <o que o código mostra, com valores>." Cite a evidência que confirma ou desmente. Premissa que o código não permite verificar vai com `a_confirmar` (vira pergunta para o solicitante).
   - Feche a seção com uma afirmação "Demanda como entendida: <o que de fato precisa mudar>", que já incorpora as correções (por exemplo: "o parcelamento já existe para pessoa jurídica, em até 6 vezes; a demanda se reduz a limitar a 3").
4. **Descreva como é hoje** (`como_e_hoje`): as regras atuais do assunto, com valores e condições, cada uma com evidência.
5. **Consulte a origem de cada regra que a demanda muda.** Um comportamento que parece defeito pode ter sido pedido de propósito por uma demanda anterior, e mudá-lo revoga essa regra sem o negócio saber. Para cada regra que a demanda muda, chame `historico_do_trecho` com o arquivo e as linhas da evidência (ou com `texto`, o identificador da regra, quando o trecho mudou de lugar). O commit mais antigo é a origem; veja também os que mudaram a regra depois. Quando a mensagem de um deles cita uma demanda (uma chave como DEM-12 ou #12) que pediu esse comportamento:
   - registre em `como_e_hoje` que a regra foi criada (ou mudada) de propósito pela demanda, com a chave, a data e a intenção dela (o título da issue, quando vier, ou a mensagem do commit), passando o hash do commit em `origens` do `afirmar`, ao lado da evidência de código, que continua sendo a evidência da regra;
   - registre com `perguntar`, `tipo: "decisao"`, citando a chave, se a regra da demanda continua valendo, com as opções e o efeito de cada uma (por exemplo: "A regra da DEM-12, que mantém a fatura em aberto quando o pagamento é estornado, continua valendo? A demanda atual pede o contrário.", com as opções "Manter a regra" (efeito: "a demanda atual não é atendida como pedida") e "Revogar a regra" (efeito: "revoga a regra da DEM-12; as faturas estornadas passam a fechar")).
   Commit sem chave é só ajuste: não invente a demanda, e não cite nomes de pessoas. Se a ferramenta avisar que não há histórico (pasta fora do git, clone raso), siga a análise e diga isso ao analista.
6. **Feche o conjunto de quem grava o estado,** quando a demanda trata de um estado errado (um status, uma situação, uma flag com um valor que não deveria ter). A rota que a tela usa é só um dos caminhos: chamadas internas, lotes e a edição de outros registros ligados também gravam o mesmo estado, às vezes por um método com a validação desligada.
   - Parta do campo ou do método de escrita: `referencias` com o campo e `tipos: ["escrita"]` (no banco), com a tabela e `tipos: ["mencao"]` (SQL embutido no backend) ou com o método que grava. Suba com `referencias` e `profundidade: 2` ou `3`, que devolve a árvore de quem chama até os pontos de entrada (telas, rotas, lotes, integrações), inclusive as chamadas internas. Leia cada rotina que grava: o que ela confere antes (a situação atual, valores, o motivo) e se quem chama pode desligar essa conferência (um parâmetro, um modo administrativo).
   - Registre em `como_e_hoje` uma afirmação com a lista: "Caminhos que gravam <estado>: 1. <ponto de entrada> → <rotina>: valida <o quê>; 2. <ponto de entrada> → <rotina>: não valida <o quê>; ...", com a evidência da gravação de cada um, e diga se a lista é completa ou o que ficou aberto (árvore cortada, rotina sem chamador no mapa, chamada dinâmica).
   - Com mais de um caminho, cada um vira item em `riscos` (ou em `funcionalidades`, quando a demanda muda aquele caminho), dizendo o que valida e o que não valida. O caminho sem validação costuma ser a causa que a demanda procura.
   - A suposição do solicitante sobre o caminho ("deve ser a rota antiga") é premissa como as outras: verifique-a no código e não a transforme em escopo sem evidência.
7. **Levante o que muda,** cada item com o arquivo ou a tabela onde a mudança acontece: `funcionalidades` (regras e telas de negócio), `dados` (tabelas, colunas e parâmetros; inclusive dados já gravados no formato atual), `integracoes` (lotes, arquivos e serviços que recebem ou enviam o que muda) e `documentos_e_telas` (relatórios, documentos emitidos e telas).
   - **Escolher a regra é do negócio.** Quando a correção exige escolher uma regra (manter ou revogar a de uma demanda anterior, qual valor vale, para que público), não proponha a regra em `funcionalidades`: registre a decisão com `perguntar`, `tipo: "decisao"`, com 2 a 4 `opcoes`, cada uma com o `efeito` de escolhê-la ("revoga a regra da demanda X", "mantém a exceção para Y"), e a `recomendacao`, se houver. As decisões abrem o documento, na seção "Decisões para o negócio", antes da parte técnica.
   - **Hipótese não vira escopo.** Uma mudança que depende de uma hipótese que você não verificou no código ("pode estar", "se houver", "opcional", "preventiva") não entra em `funcionalidades`. Verifique-a; se o código não basta, registre com `perguntar` a dúvida, dizendo o que a confirmaria (uma consulta, um dado, uma regra fora do código), ou a decisão, com as opções. Se mesmo assim a mudança precisar ficar em `funcionalidades`, ligue a pergunta a ela (`afirmacao`): sem isso, `concluir_analise` aponta a pendência.
8. **Olhe os dados pessoais e os requisitos não funcionais** (`dados_pessoais_e_rnf`). São o que costuma passar despercebido até a homologação. Para cada aspecto, só afirme o que tem evidência:
   - **Dados pessoais e sensíveis (LGPD):** para cada tabela que a demanda toca, chame `schema` com `tabela`: as colunas cujo nome e tipo sugerem dado pessoal vêm marcadas com um indício e a categoria. O indício não é prova: confirme no código o que a coluna guarda e de quem (pessoa física ou jurídica, por exemplo; CNPJ de empresa não é dado pessoal) e procure com `referencias` onde ela aparece (telas, relatórios, arquivos e integrações). Registre um campo por afirmação: "Campo <TABELA.COLUNA>: <categoria> (dado pessoal | dado pessoal sensível). Finalidade aparente: <para que o projeto usa>. Onde aparece: <telas, relatórios, integrações>.", com a evidência da definição da coluna e do uso. Não aponte coluna sem indício nem evidência só porque a tabela é de pessoas, e não liste colunas que a demanda não toca. Texto livre (observações) ou dado sensível (saúde, origem racial, religião, biometria) que a demanda passa a ler ou expor vira pergunta ao solicitante: o código não diz o que foi digitado nem se há base legal.
   - **Auditoria e logs:** a mudança altera o que é registrado (tabelas de log, trilhas, quem fez e quando)? Cite onde o projeto grava o log hoje; se o processo afetado não registra nada, diga isso com a evidência do processo.
   - **Desempenho:** lotes, jobs e consultas pesadas afetados (leitura de tabela inteira, novos joins, arquivos maiores), com a evidência do processo. Volume de registros e janela de processamento quase nunca estão no código: pergunte.
   - **Segurança e acesso:** perfis e permissões envolvidos, com a evidência da checagem (tela, serviço ou banco). Quem recebe o dado fora do projeto (arquivo enviado a terceiro, por exemplo) e com que controle de acesso vira pergunta.
   Aspecto que a demanda não toca não precisa de afirmação. Aspecto sem evidência no código vai como `a_confirmar` ou com `perguntar`, nunca como fato.
9. **Agrupe o que se repete.** Quando a mesma mudança se repete em muitos lugares (a mesma validação em vinte telas, o mesmo campo em quinze relatórios), registre uma afirmação para o grupo, com a contagem e dois ou três exemplos citados como evidência, em vez de uma afirmação por item. Use o total de `referencias` para contar. Diga se a lista é completa ou se há casos que não conseguiu localizar.
10. **Registre os riscos** (`riscos`): dados antigos no formato atual, integrações e lotes que dependem do comportamento atual, relatórios que agrupam pelos dados alterados, regras repetidas em mais de uma camada que podem ficar divergentes, outros públicos afetados pela mesma mudança (um parâmetro único para todos os tipos de cliente, por exemplo). **Mudanças em andamento:** antes de concluir, chame `mudancas_em_andamento` (analise: "{{id}}", com `arquivos`: os arquivos onde a mudança acontece; sem eles, valem os das evidências). Para cada PR ou MR aberto que altera esses arquivos, registre um risco de dependência ou de conflito com o número, o título e o link, e verifique se ele já cobre a demanda (pelo título, pelas demandas citadas e pelos arquivos). Se a ferramenta avisar que não conseguiu consultar (sem repositório cadastrado, sem rede ou sem acesso), siga a análise e diga isso ao analista.
11. **Estime a abrangência** (`abrangencia`) numa afirmação `inferido` que **começa pela estimativa** ("Pequena:", "Média:" ou "Grande:"), seguida da justificativa com a contagem de lugares afetados e citando as evidências principais:
   - pequena: mudança num só lugar ou num parâmetro, sem alterar estrutura de dados e sem efeito em outras camadas;
   - média: mudança em algumas camadas ou módulos, ou numa regra repetida, ainda sem migrar dados;
   - grande: mudança de estrutura de dados com migração do que já está gravado, integrações externas afetadas ou muitos lugares.
   Se reestimar depois, registre outra afirmação: vale a mais recente.
12. **O que o código não responde vira pergunta, não suposição.** Valores em tabela de parâmetros, jobs agendados e sistemas vizinhos vão para `perguntar` (analise: "{{id}}"), escritas para o solicitante. Decisões do negócio (por exemplo, se a mudança vale também para outro público) vão para `perguntar` com `tipo: "decisao"` e as opções. Uma afirmação `a_confirmar` já cria a sua pergunta; não repita.
13. **Registre com `afirmar`** (analise: "{{id}}"), uma regra ou fato por chamada, com a confiança honesta (`confirmado` só com evidência direta; `inferido` para dedução a partir das evidências listadas; `a_confirmar` quando o código não basta) e as evidências: arquivo relativo à raiz do projeto, linhas e o trecho copiado do código, exatamente como está. Evidência recusada volta com o motivo e a correção: corrija e chame de novo. Seções desta análise:
{{secoes}}
14. **Termos de negócio** que você ligou ao código vão para `propor_termo`.
15. **Você não altera o código.** A análise de impacto descreve o que mudaria; se o analista pedir a correção ou a implementação, explique o que mudaria e registre com `registrar_sugestao` (trecho atual, trecho proposto, motivo, impacto e um exemplo com números).
16. **Mantenha a memória da análise** com `memorizar` (analise: "{{id}}"). O documento guarda o resultado; a memória guarda o contexto de trabalho que uma conversa nova precisaria para continuar, se esta acabar: o que já foi investigado (`investigado`) e descartado, com o motivo (`descartado`), as hipóteses em aberto (`hipoteses`), onde está o código relevante, inclusive repositórios fora do projeto (`codigo`), o que o analista orientou (`combinados`), o que ficou pendente (`pendencias`) e onde você parou (`onde_parou`, com `modo: "substituir"`). Atualize nos marcos: depois de uma descoberta relevante, quando o analista der uma orientação e antes de concluir. Seja curto; não repita afirmações, perguntas e termos já registrados.
17. **Conclua.** Confira que as seções obrigatórias ({{secoes_obrigatorias}}) têm afirmação e chame `concluir_analise` (analise: "{{id}}"). Se houver pendências, trate-as e chame de novo.

## Resposta no chat

O chat não é o documento. Depois de `concluir_analise`, dê ao analista só:

- primeiro, as decisões para o negócio, com as opções e o efeito de cada uma;
- a demanda como entendida, destacando a premissa corrigida, se houver;
- as regras criadas de propósito por demandas anteriores que a demanda muda, com a chave e a pergunta se continuam valendo;
- numa demanda sobre um estado errado, os caminhos que gravam o estado, com o que cada um valida e o que não valida;
- a abrangência estimada, em uma frase;
- os principais riscos;
- os dados pessoais e sensíveis afetados e o efeito em auditoria, desempenho e acesso, se houver;
- as perguntas para o solicitante;
- as fontes externas usadas (documentação publicada, com o repositório e o commit), se houver;
- o caminho do documento devolvido por `concluir_analise`, onde está o detalhe com as evidências.

## Contexto da base

{{contexto}}