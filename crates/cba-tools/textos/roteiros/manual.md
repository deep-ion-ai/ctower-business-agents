<!--
Roteiro do fluxo "manual do usuário". O agente o recebe na resposta de
iniciar_analise e de retomar_analise. Este comentário inicial não vai para o
agente.

Para ajustar o roteiro à sua organização (por exemplo, ao padrão de manual da
casa), edite este arquivo na base (_templates/roteiros/manual.md). Para voltar
ao padrão do app, apague-o.

Variáveis: {{id}}, {{titulo}}, {{fluxo}}, {{documento}}, {{entrada}},
{{secoes}}, {{secoes_obrigatorias}} e {{contexto}}; o que cada uma traz está no
roteiro de perguntar ao sistema (_templates/roteiros/perguntar.md). Aqui,
{{entrada}} é a funcionalidade do mapa: descrição, atores, pontos de entrada,
explicação aprovada, regras confirmadas e demandas ligadas.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento na base: `{{documento}}` (montado pelo app a partir dos registros; não o edite).

## Funcionalidade

{{entrada}}

## Objetivo

Escrever o **manual do usuário** da funcionalidade: o texto que o usuário final do sistema lê para fazer a tarefa sozinho. Linguagem de usuário, não de analista nem de programador: o que ele vê na tela, o que clica e digita, o que acontece em seguida e o que fazer quando algo dá errado. Nada de nome de tabela, classe, rotina ou campo técnico no texto; o código fica nas evidências. O manual descreve o que o sistema faz hoje.

## Como conduzir

1. **Parta do que já está aprovado.** Se a entrada cita a explicação aprovada da funcionalidade, chame `retomar_analise` com ela (não com {{id}}) e use as afirmações confirmadas: objetivo, atores, fluxo principal, exceções e regras. As regras confirmadas da base (`RN-…`, também em `conhecimento`) dizem os limites e as recusas que o usuário vai encontrar. Sem explicação aprovada, siga os pontos de entrada no código, como no fluxo de explicar, mas registre só o que o usuário percebe.
2. **Confira no código o que vai para a tela:** os rótulos dos campos e botões, a ordem dos passos, as mensagens de erro e de confirmação, como estão escritas. Use `estrutura`, `referencias` e a leitura dos arquivos das telas citadas nos pontos de entrada.
3. **Registre as seções de texto com `afirmar`** (analise: "{{id}}"), uma ideia por chamada, em linguagem de usuário, citando a evidência do código quando houver (a tela, a validação, a mensagem). Use `confirmado` quando o código mostra; `inferido` quando você deduziu; `a_confirmar` quando só o negócio responde (vira pergunta):
{{secoes}}
4. **Registre o passo a passo com `registrar_passo_manual`** (analise: "{{id}}"), um passo por chamada, na ordem: um título curto no imperativo ("Informe o motivo do cancelamento"), a `instrucao` com o que o usuário faz (o campo, o botão, o valor esperado), a `tela` em que o passo acontece (o nome de negócio da tela, a partir dos pontos de entrada de tipo tela: é o "print sugerido" que o analista vai preencher com a imagem) e o `resultado` que o usuário vê. Para corrigir um passo, chame de novo com o `numero` dele. Não descreva imagens nem invente prints: o analista inclui os prints no app.
5. **Mensagens e erros comuns** saem das validações e das recusas do código: a mensagem como o usuário a vê, quando aparece e o que fazer. **Perguntas frequentes** saem das regras e dos fluxos alternativos ("Posso cancelar uma fatura já paga?"), com a resposta curta.
6. **O que o código não responde vira pergunta,** com `perguntar` (analise: "{{id}}"): perfis de acesso configurados fora do código, prazos definidos pelo negócio, a quem recorrer.
7. **Você não altera o código.** Defeito encontrado vira `registrar_sugestao`.
8. **Mantenha a memória da análise** com `memorizar` (analise: "{{id}}"): o que já foi lido, os passos que faltam e onde você parou (`onde_parou`, com `modo: "substituir"`).
9. **Conclua.** Confira que as seções obrigatórias ({{secoes_obrigatorias}}) têm afirmação e que há ao menos um passo, e chame `concluir_analise` (analise: "{{id}}").

## Resposta no chat

O chat não é o documento. Depois de `concluir_analise`, dê ao analista só:

- o objetivo do manual em uma frase;
- os passos (número e título), com a tela sugerida para o print de cada um;
- as perguntas em aberto;
- o lembrete de que os prints entram no app, em cada passo, e de que passo sem print fica pendente até ele incluir a imagem ou dispensá-la;
- o caminho do documento devolvido por `concluir_analise`.

## Contexto da base

{{contexto}}
