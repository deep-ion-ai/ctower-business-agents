# Guia: publicar a documentação aprovada

Este guia é para o analista. Ele mostra como guardar a documentação aprovada num repositório git,
revisada por PR/MR, e como ela volta para as análises seguintes. O comportamento completo está em
[Repositórios do projeto](repositorios.md#documentação-publicada-273).

**Exportar ou publicar?** "Exportar" grava um arquivo avulso (Markdown, Word, HTML ou CSV) para
mandar a alguém. "Publicar" versiona a base aprovada do projeto inteiro num repositório, com
histórico e revisão, e outros analistas do projeto passam a consultá-la nas análises deles.

## 1. Escolher o destino

No projeto, aba "Código e demandas", cartão **Onde a documentação é publicada**:

- **Pasta num repositório de código:** escolha um dos repositórios de código já cadastrados e a
  subpasta (por exemplo, `docs/negocio`). Bom quando a equipe quer a documentação junto do código.
- **Repositório próprio:** informe a forja, o caminho (`dono/repo`), a URL de clone, a branch base e
  de onde vem o token, como num repositório de código, e uma subpasta, se quiser. Use **Clonar na
  pasta dos clones** se ainda não houver clone. Bom quando a documentação tem dono ou ciclo próprio.

Salve o destino. A saúde do projeto mostra se o clone e o token estão prontos. O token precisa de
**permissão de escrita** no repositório e de abrir PR/MR; para só ler o código, continua bastando um
token de leitura.

A pasta da documentação não conta como código: o agente não a cita como evidência e ela não dispara
o aviso de alteração.

## 2. Publicar

1. Aprove as análises que quer publicar. Só entra o que foi aprovado: análises, documentos de
   funcionalidade e mapa aprovados, regras confirmadas, termos do glossário e sugestões aceitas.
   Demandas, atas, pessoas e memórias nunca são publicadas.
2. Na tela de uma análise, clique em **Publicar**, ao lado do "Exportar". A publicação leva a base
   aprovada do projeto inteiro, não só aquela análise.
3. Confira a prévia: o que é novo, o que muda e o que sai do repositório.
4. Confira que não há segredos (senhas, tokens, chaves, endereços internos) nem dados pessoais nos
   documentos e marque a confirmação.
5. Clique em **Publicar**. O app grava os documentos na branch `docs/cba-<projeto>` e abre o PR/MR;
   o resultado traz o link. Nada é gravado direto na branch base, e o seu clone fica como estava.

Publicar de novo com o PR/MR ainda aberto acrescenta um commit no mesmo PR/MR. Se nada mudou, o app
diz que não há o que publicar.

## 3. Revisar o PR/MR

O PR/MR é revisado e mesclado na forja (GitHub ou GitLab), como qualquer mudança no repositório:

- os arquivos são os `.md` da base, com o frontmatter (tipo, identificador, status, evidências com o
  commit), no mesmo layout de pastas;
- o `.cba-publicacao.json` lista o que o app publicou; o app só remove do repositório o que está
  nele, e nunca apaga nem sobrescreve um arquivo escrito à mão (a publicação para com um conflito);
- comentários e commits de revisores na branch são preservados na próxima publicação.

Mudanças de conteúdo devem ser feitas no app (revisão e aprovação da análise) e publicadas de novo;
um ajuste feito direto no repositório é sobrescrito na próxima publicação daquele documento.

## 4. Sincronizar

Depois do merge, a documentação publicada chega à base de cada analista do projeto pela
sincronização:

- automaticamente, em segundo plano, ao iniciar uma análise (no máximo a cada 10 minutos);
- ou pelo botão **Sincronizar agora**, no cartão do destino.

O cartão mostra o commit, quando sincronizou, quantos documentos vieram e o que foi pulado ou
mascarado (segredos, CPF, e-mail). Na tela da base, os documentos aparecem com o rótulo da origem e
o filtro "Externos", só para leitura. A sua própria base não recebe de volta o que ela mesma
publicou: os documentos já estão nela.

Se a sincronização falhar (rede, token, clone) ou a cópia tiver mais de 24 horas, o agente recebe
um aviso de que a documentação publicada pode estar desatualizada, e a análise continua.

## 5. Como a documentação volta para as análises

- Ao iniciar uma análise, o agente recebe até 3 títulos da documentação publicada relacionados ao
  assunto, como lembrete.
- Quando ajuda, ele consulta a documentação publicada pela ferramenta `conhecimento` e recebe os
  documentos rotulados `[doc. publicada]`, com o repositório, o commit e a confiança. A base do
  analista vem antes, em relevância parecida.
- A documentação é **pista, não prova**: o agente confirma no código antes de afirmar, e a evidência
  é sempre o código. Divergência entre o documento e o código vira pergunta.
- No chat, o agente diz quais fontes externas usou (repositório e commit).

## Problemas comuns

| Mensagem | O que fazer |
|---|---|
| "não tem pasta local: clone o repositório antes de publicar" | Clone o repositório do destino no cartão ou informe a pasta do clone. |
| "sem permissão de escrita" | Use um token com escrita no repositório e permissão de abrir PR/MR. |
| "já existem no repositório, sem terem sido publicados pelo app" | Mova os arquivos escritos à mão ou escolha outra subpasta. |
| "a pasta ... já recebe a publicação do projeto ..." | Cada projeto precisa da própria pasta: escolha outra subpasta. |
| "o clone em ... tem alterações não comitadas" | Comite, descarte ou mova as alterações nos arquivos publicados. |
| Aviso de documentação desatualizada na análise | Clique em "Sincronizar agora" e veja o erro no cartão. |
