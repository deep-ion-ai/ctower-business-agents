## Modo: ferramenta de design do Claude, rodando localmente

O analista pediu as telas com o método da ferramenta de design do Claude, rodando aqui. **Nada sai do ambiente:** não use ferramenta, conector nem site externo; o resultado de cada tela volta por `gravar_html_tela` (analise: "{{id}}"), como no passo 6, com as mesmas regras (sem script, sem rede, os quatro estados, cada elemento com `data-elemento`).

**Comece pelos tokens.** Chame `tokens_css` antes da primeira tela. O design parte do template de tokens aprovado pelo analista: cores, tipografia, espaçamento e raio só pelos papéis `var(--prototipo-*)`. Se os tokens não estiverem aprovados, avise o analista antes de desenhar (a tela sai em wireframe neutro, sem a identidade visual) e siga só com a estrutura; não invente cor, fonte nem medida.

Antes do HTML de cada tela, pense o design nesta ordem:

1. **Tarefa principal.** Em uma frase, o que o usuário faz na tela. Ela define o que vem primeiro e qual é a ação principal (uma só, com destaque; as outras com a classe `secundario`).
2. **Hierarquia visual.** Título da tela, depois o que o usuário precisa ler para decidir, depois os campos, por fim as ações. Agrupe campos relacionados em `<fieldset>` com `<legend>`; no máximo dois níveis de título dentro da tela. O que é informativo fica menor e mais discreto que o que pede decisão.
3. **Componentes.** Reaproveite os mesmos componentes em todas as telas do protótipo: o mesmo cabeçalho, o mesmo bloco de ações (`acoes`), o mesmo formato de mensagem (`mensagem` com `erro`, `alerta` ou `sucesso`) e de campo com erro (`campo-erro`). Campo com rótulo visível (`<label>`), formato e limite na ajuda do campo; tabela só para dados tabulares, com cabeçalho.
4. **Consistência com os tokens.** Espaçamento e raio iguais entre componentes do mesmo tipo; cor primária só na ação principal e nos links; cores de erro, alerta e sucesso só nas mensagens e nos estados correspondentes. Nada de cor, sombra ou fonte fora dos papéis.
5. **Estados.** Os quatro estados mostram a mesma tela, com a mesma estrutura: muda só o que o estado muda (valores preenchidos, mensagens de erro junto ao campo, confirmação de sucesso).
6. **Tela alterada.** Parta da tela atual, lida no código, e mantenha o desenho dela; marque com `mudou` só o que a demanda muda. O modo de design não autoriza redesenhar o que a demanda não toca.

Na resposta no chat, diga em uma linha as decisões de design comuns às telas (componentes e hierarquia) e se os tokens estavam aprovados.
