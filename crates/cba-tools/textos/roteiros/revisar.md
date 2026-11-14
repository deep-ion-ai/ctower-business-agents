<!--
Roteiro do fluxo "revisar requisito". O agente o recebe na resposta de
iniciar_analise e de retomar_analise. Este comentário inicial não vai para o
agente.

Para ajustar o roteiro à sua organização (por exemplo, os termos vagos que a
casa também evita ou o formato de história adotado), edite este arquivo na base
(_templates/roteiros/revisar.md). Para voltar ao padrão do app, apague-o.

Variáveis: {{id}}, {{titulo}}, {{fluxo}}, {{documento}}, {{entrada}},
{{secoes}}, {{secoes_obrigatorias}} e {{contexto}}; o que cada uma traz está no
roteiro de perguntar ao sistema (_templates/roteiros/perguntar.md). Aqui,
{{entrada}} é o requisito ou a história a revisar, como o autor escreveu (colado
pelo analista ou lido do rastreador). Sem {{entrada}}, {{secoes}} ou
{{contexto}}, o app acrescenta o valor no fim e avisa.

O conteúdo da revisão vai para o parecer (registrar_achado, registrar_invest e
registrar_veredito); as perguntas para o autor, para perguntar.
-->
# Roteiro da análise {{id}}: {{titulo}}

Fluxo: {{fluxo}}. Documento na base: `{{documento}}` (o parecer de revisão, montado pelo app a partir dos registros; não o edite).

## Texto a revisar, como o autor escreveu

{{entrada}}

## Objetivo

Dizer ao analista se o requisito ou a história pode ir para o desenvolvimento, e o que corrigir antes. O diferencial desta revisão é **conferir o texto contra a base e o código**: um texto bem escrito que contradiz o que o sistema faz é o achado mais importante. Você não reescreve o texto inteiro: aponta cada problema com o trecho, a característica violada, a severidade e uma reescrita sugerida.

## Como conduzir

1. **Entenda o texto.** Identifique se é uma história de usuário ("Como <perfil>, quero <ação>, para <valor>", com critérios de aceite) ou um requisito, os perfis citados, as regras e valores afirmados e as necessidades pedidas.
2. **Consulte a base primeiro.** O contexto no fim deste roteiro traz regras confirmadas (`RN-…`), termos do glossário e análises anteriores relacionados ao texto; para outros conceitos, use `conhecimento`. Regra confirmada não precisa ser redescoberta: confira se a evidência dela ainda vale e use-a.
3. **Confira cada afirmação do texto no código.** Para cada regra, valor, perfil ou condição que o texto afirma, procure no código o que o sistema faz: `referencias`, `estrutura`, `schema`, `mapa` e as suas ferramentas de leitura e busca, inclusive nas outras camadas (tela, serviço, banco, lotes).
4. **Avalie o texto pelas características da ISO/IEC/IEEE 29148** e registre cada problema com `registrar_achado` (analise: "{{id}}"), um achado por problema:
   - `correcao`: o texto **contradiz uma regra confirmada da base ou o que o código faz** (um valor, um perfil, uma condição diferente). Diga no `problema` o que a regra ou o código fazem, com valores, cite em `regras` a regra da base contrariada (RN-…) e, em `evidencias`, o trecho do código: arquivo relativo à raiz do projeto, linhas e o trecho copiado, exatamente como está. Sem evidência, o achado de correção é recusado. Na dúvida se a diferença é erro do texto ou mudança pedida de propósito, registre o achado e pergunte ao autor com `perguntar`.
   - `ambiguidade`: termos vagos. Chame `verificar_termos_vagos` (analise: "{{id}}"): ele devolve os termos da lista de referência que aparecem no texto ("rápido", "adequado", "etc.", "normalmente", "e/ou" e outros). Registre um achado para cada um (pode juntar no mesmo achado termos da mesma frase), com a reescrita verificável. Aponte também outros termos vagos que a lista não tem.
   - `verificabilidade`: critério de aceite que não se pode testar com um resultado objetivo ("deve ser rápido", "o sistema deve funcionar bem").
   - `completude`: casos de exceção e perfis que o código trata e o texto não diz (outro tipo de cliente, um bloqueio, um limite, uma aprovação). Compare com as regras e exceções do código e cite a evidência; o `trecho` pode ficar vazio, porque o achado aponta o que falta.
   - `consistencia`: termo que diverge do glossário do projeto (um sinônimo que o glossário não usa, um nome do sistema em vez do nome do negócio). Na reescrita, use o termo do glossário.
   - `singularidade`: o texto junta mais de uma necessidade; sugira a divisão.
   Severidade: `alta` quando o texto, como está, leva a construir a coisa errada (contradição, regra errada, critério que contradiz outro); `media` quando gera dúvida ou retrabalho provável; `baixa` para melhoria de redação. O `trecho` é copiado do texto revisado, como está; trecho que não aparece no texto é recusado.
5. **Se for uma história, faça a verificação INVEST** com `registrar_invest` (analise: "{{id}}"): os seis critérios (independente, negociável, valiosa, estimável, pequena e testável), cada um com o resultado e uma justificativa curta. Requisito que não é história não tem INVEST.
6. **O que o texto não responde vira pergunta para o autor**, com `perguntar` (analise: "{{id}}"), escrita para quem escreveu o texto (por exemplo, "A história muda a regra atual de cancelamento, que não aceita fatura com pagamento parcial, ou o texto está errado?").
7. **Fatos do sistema que sustentam os achados** (opcional): registre com `afirmar` (analise: "{{id}}"), na seção abaixo, uma regra ou fato por afirmação, com evidência. Seções do documento que recebem afirmações:
{{secoes}}
8. **Registre o veredito** com `registrar_veredito` (analise: "{{id}}"), depois dos achados: `pronta`, `pronta_com_ressalvas` ou `precisa_de_retrabalho`, com uma justificativa curta. Achado de severidade alta pede retrabalho; os demais, ao menos ressalvas.
9. **Você não altera o código nem o texto do autor.** Se o analista pedir uma correção no código, explique o que mudaria e registre com `registrar_sugestao`.
10. **Conclua.** Chame `concluir_analise` (analise: "{{id}}"), que confere o veredito e avisa dos termos vagos sem achado. Seções obrigatórias: {{secoes_obrigatorias}}. Se houver pendências, trate-as e chame de novo.

## Resposta no chat

O chat não é o parecer. Depois de `concluir_analise`, dê ao analista só:

- o veredito, em uma frase;
- as contradições com a base ou o código, com a regra;
- os principais achados, agrupados;
- as perguntas para o autor;
- o caminho do parecer, onde está o detalhe com as evidências.

## Contexto da base

{{contexto}}
