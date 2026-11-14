<!--
Roteiro do fluxo "analisar reunião". O agente o recebe na resposta de
iniciar_analise e de retomar_analise. Este comentário inicial não vai para o
agente.

Para ajustar o roteiro à sua organização (por exemplo, o jeito da casa de
registrar pendências ou os papéis usuais nas reuniões), edite este arquivo na
base (_templates/roteiros/reuniao.md). Para voltar ao padrão do app, apague-o: o
app grava de novo a cópia padrão na próxima análise.

Variáveis: {{id}}, {{titulo}}, {{fluxo}}, {{documento}}, {{entrada}},
{{secoes}}, {{secoes_obrigatorias}} e {{contexto}}; o que cada uma traz está no
roteiro de perguntar ao sistema (_templates/roteiros/perguntar.md). Aqui,
{{entrada}} é a reunião analisada (identificador, demanda e caminho da ata), e
{{contexto}} traz a demanda, os falantes com o papel sugerido pelo catálogo, as
atas anteriores, as pendências delas e as pessoas do projeto. Sem {{entrada}},
{{secoes}} ou {{contexto}}, o app acrescenta o valor no fim e avisa.

O conteúdo da reunião vai para a ata (registrar_item_ata e
registrar_resumo_ata); o documento da análise ({{documento}}) guarda só a
ligação com o código e as perguntas do agente.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento da análise na base: `{{documento}}` (montado pelo app a partir dos registros; não o edite).

## Reunião

{{entrada}}

## Objetivo

Montar a ata da reunião a partir do transcript, para o analista revisar e aprovar: o objetivo, um resumo, as decisões, as necessidades levantadas, as regras de negócio citadas, as pendências (com responsável e prazo), as dúvidas em aberto e os próximos passos. **Cada item da ata nasce de uma fala**: o que ninguém disse não entra. A ata é um documento interno e não vai ao cliente; o resumo para o cliente sai depois, por um prompt pronto.

## Como conduzir

1. **Comece pelo contexto da demanda** (no fim deste roteiro): as atas anteriores, as pendências delas, as pessoas do projeto e os falantes desta reunião com o papel sugerido pelo catálogo. Use-o para reconhecer o que já foi decidido, o que estava pendente e quem é quem. Não registre de novo o que uma ata anterior já tem, a menos que esta reunião o mude ou o retome: aí registre o que foi dito agora (por exemplo, "a pendência P-2 da reunião anterior foi concluída" ou "a decisão D-1 foi revista"), citando a fala desta reunião.
2. **Leia o transcript inteiro com `ler_transcript`**, página a página, do começo ao fim: chame de novo com o `cursor` igual ao `proximo_cursor` da resposta até não haver próxima página. Não conclua nada antes de ler tudo: uma decisão do começo pode ser desfeita no fim. Depois, para voltar a um trecho ou acompanhar uma pessoa (quem ficou de fazer o quê, o que o cliente pediu), use o filtro por `falante` (parte do nome basta) ou pelo horário (`desde` e `ate`).
3. **Proponha o papel de cada participante com `propor_papel`**, com uma ou duas falas que mostram o papel (quem pede, quem decide, quem explica o sistema) e uma justificativa curta, sem copiar falas. A resposta diz o papel que o catálogo de pessoas sugere: se a sua proposta for diferente, diga na justificativa o que a fala mostra. Quem confirma é o analista, na tela de participantes.
4. **Registre cada item da ata com `registrar_item_ata`**, um item por chamada, nas suas palavras e em linguagem de negócio, sempre com a fala de origem em `citacoes` (o `indice` da fala e um `trecho` curto copiado dela). Cubra todas as seções da ata:
   - `decisao` (Decisões): o que foi decidido, por quem tinha o poder de decidir;
   - `necessidade` (Necessidades e requisitos levantados): o que o negócio precisa que o sistema faça, um requisito por item;
   - `regra` (Regras de negócio citadas): regra dita na reunião, com valores e condições como foram falados;
   - `pendencia` (Pendências): a ação, com `responsavel` e `prazo` como foram ditos ("até sexta"); se a fala não diz o responsável ou o prazo, escreva "a definir" e registre também uma `duvida` sobre isso;
   - `duvida` (Dúvidas em aberto): pergunta que ficou sem resposta na reunião, escrita para quem vai respondê-la;
   - `proximo_passo` (Próximos passos): o que fazer a seguir, apontando em `referencias` os itens de que trata (por exemplo, "Analisar o impacto da necessidade N-3", com `referencias: ["N-3"]`).
5. **Registre o objetivo e o resumo com `registrar_resumo_ata`**, depois de ler o transcript inteiro: o `objetivo` em uma ou duas frases, o `resumo` em poucos parágrafos curtos com o que foi tratado e concluído. Os dois vão nas suas palavras, sem falas copiadas.
6. **Citações curtas e literais; nunca invente.**
   - O trecho é copiado da fala, palavra por palavra, e curto: o bastante para mostrar a origem do item (uma frase ou parte dela), nunca a fala inteira.
   - Cite a fala que de fato sustenta o item. Citação recusada volta com o motivo (fala inexistente, trecho que não aparece na fala, trecho longo): corrija e chame de novo; não troque a citação por uma que não sustenta o item.
   - Não deduza decisões, prazos nem responsáveis que ninguém disse. O que ficou ambíguo vira `duvida`, não suposição. Um item que você percebe que faltou discutir, mas que ninguém falou, não entra na ata: registre-o com `perguntar` (analise: "{{id}}"), para o analista.
7. **Ligação com o código (opcional).** Para uma necessidade ou regra citada, você pode consultar a base (`conhecimento`; o contexto já traz o que a base tem sobre a demanda) e o código (`referencias`, `estrutura`, `schema`, `mapa` e as suas ferramentas de leitura e busca) e anotar, com evidência, se o sistema já faz isso ou se a regra conflita com uma regra já confirmada (como RN-FAT-007). Registre com `afirmar` (analise: "{{id}}") na seção `ligacao_com_o_codigo`, citando o item da ata no texto (por exemplo, "Sobre N-3: o sistema já exige o supervisor na tela de cancelamento") e as evidências: arquivo relativo à raiz do projeto, linhas e o trecho copiado do código, exatamente como está. **Não faça a análise de impacto aqui**: se a necessidade pede uma, registre um `proximo_passo` ("Analisar o impacto da necessidade N-3"), que o analista abre como uma análise nova, ligada à demanda. Seções do documento da análise:
{{secoes}}
8. **Demanda parecida.** Se a reunião trata de um assunto que parece ter demanda própria, diferente da demanda desta reunião (um pedido novo do cliente, por exemplo), use `sugerir_demanda` com o assunto em poucas palavras e conte ao analista o que encontrou no fim, no chat. Quem decide ligar ou criar a demanda é o analista; não registre nada sobre isso na ata.
9. **Privacidade.** O transcript tem dados pessoais e é apagado quando a ata for aprovada. Nada dele vai para fora da ata além dos trechos curtos das citações: não copie falas para o chat, para o resumo, para os textos dos itens nem para as afirmações. Fale das pessoas pelo nome e pelo papel na reunião, sem opiniões sobre elas.
10. **Você não altera o código.** Se o analista pedir uma correção, explique o que mudaria e registre com `registrar_sugestao`.
11. **Conclua.** Confira que a ata tem objetivo, resumo e os itens de cada seção que a reunião tratou (uma seção sem item deve ser porque a reunião não tratou do assunto, não porque faltou ler), e chame `concluir_analise` (analise: "{{id}}"), que confere a ata. Seções obrigatórias do documento da análise: {{secoes_obrigatorias}}. Se houver pendências, trate-as e chame de novo.

## Resposta no chat

O chat não é a ata. Depois de `concluir_analise`, dê ao analista só:

- o objetivo da reunião, em uma frase;
- as decisões, curtas;
- as pendências, com responsável e prazo;
- as dúvidas em aberto;
- os próximos passos e as demandas parecidas que `sugerir_demanda` encontrou, se houver;
- o caminho da ata, onde está o detalhe com as falas citadas.

## Contexto da demanda e da base

{{contexto}}