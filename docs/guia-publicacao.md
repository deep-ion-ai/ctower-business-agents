# Guia: publicar a documentação aprovada

Este guia é para o analista. Ele mostra como publicar a documentação aprovada num repositório git,
como páginas fáceis de ler e de navegar na forja (GitHub ou GitLab), revisadas por PR/MR, e como ela
volta para as análises seguintes. O comportamento completo está em
[Repositórios do projeto](repositorios.md#documentação-publicada-273).

**Exportar ou publicar?** "Exportar" grava um arquivo avulso (Markdown, Word, HTML ou CSV) para
mandar a alguém. "Publicar" leva a base aprovada do projeto inteiro para um repositório, como
um pequeno site em Markdown (um README de entrada, uma página por funcionalidade, o glossário e as
análises), com histórico e revisão, e outros analistas do projeto passam a consultá-la nas análises
deles.

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
   Demandas, atas, pessoas e memórias nunca são publicadas. Na janela de aprovação, confira as
   **Funcionalidades do mapa** da análise: as regras e os termos herdam essa escolha, e é ela que
   decide em que página da documentação cada regra aparece. Documentos aprovados antes do vínculo
   podem ser ligados pelo botão **Religar documentos**, na tela do mapa.
2. Na tela de uma análise, clique em **Publicar**, ao lado do "Exportar". A publicação leva a base
   aprovada do projeto inteiro, não só aquela análise.
3. Confira a prévia: por documento ("2 regras novas, 1 alterada; 1 termo novo") e as páginas
   afetadas. Se o PR/MR aberto já tem esta versão, a prévia diz que não há o que enviar.
   Sem nenhum documento aprovado, ela avisa que ainda não há aprovados no projeto.
4. Confira que não há segredos (senhas, tokens, chaves, endereços internos) nem dados pessoais nos
   documentos e marque a confirmação. Por padrão, o CPF, o CNPJ, o e-mail e o telefone de "Demanda como
   veio" e "Demanda como entendida" já saem mascarados (`[CPF mascarado]`, `[telefone mascarado]`),
   como na exportação; a base local guarda o texto original. A preferência fica no projeto, em
   Código e demandas, "Dados pessoais da demanda". Nomes de pessoas não são mascarados: confira-os.
5. Clique em **Publicar**. O app grava as páginas na branch `docs/cba-<projeto>` e abre o PR/MR;
   o resultado traz o link. Nada é gravado direto na branch base, e o seu clone fica como estava.

Publicar de novo com o PR/MR ainda aberto acrescenta um commit no mesmo PR/MR. Se nada mudou, o app
diz que não há o que publicar.

## 3. Navegar no repositório

O que vai para a pasta do destino são páginas para ler na própria forja, geradas pelo app a partir
da base aprovada. Comece pelo `README.md` e siga os links:

```text
README.md                                  entrada: o que é, o código descrito, as contagens e o
                                           índice por área → funcionalidade
funcionalidades/<área>/<id>-<nome>.md       uma página por funcionalidade aceita no mapa: o que faz,
                                           atores, "Como funciona", regras, termos, análises e os
                                           pontos de entrada no código
regras/<área>.md                           as regras que ainda não têm funcionalidade aceita
glossario.md                               os termos em ordem alfabética, com "Usado em"
analises/README.md e analises/<id>.md      as análises que deram origem à documentação
```

Um caminho típico de leitura:

1. **README → funcionalidade:** na tabela da área, clique na funcionalidade (por exemplo,
   "F-COB-001 · Cobrar multa por atraso").
2. **Funcionalidade → regra:** na seção "Regras de negócio", cada regra tem o próprio título
   (`RN-COB-001 · Multa de 2% …`) e uma âncora: o link `…/f-cob-001-cobrar-multa.md#rn-cob-001`
   leva direto a ela. Uma regra citada em outro texto vira link para essa âncora.
3. **Regra → código:** a linha "Evidências no código" traz o arquivo e as linhas. O link é um
   **permalink**: aponta para o commit analisado (`/blob/<commit>/…#L12-L14` no GitHub,
   `/-/blob/<commit>/…#L12-14` no GitLab), então mostra o mesmo trecho mesmo depois que o código
   mudar.
4. **Termos e análises:** "Termos usados" leva ao glossário, e "Origem" leva à análise que
   descobriu a regra.

Uma funcionalidade ainda em revisão no mapa não tem página: as regras dela ficam em
`regras/<área>.md`, com a menção "ainda em revisão no mapa". Quando ela for aceita (e a
documentação for publicada de novo), as regras passam para a página dela.

Cada documento da base (uma regra, um termo, uma análise) fica, dentro da página, entre dois
comentários HTML, invisíveis na forja (`<!-- cba:regra id=RN-COB-001 {…} -->` … `<!-- /cba -->`).
É por eles que o app sabe o que mudou e traz cada documento de volta na sincronização. Não os apague
nem os edite.

## 4. Revisar o PR/MR

O PR/MR é revisado e mesclado na forja, como qualquer mudança no repositório:

- o corpo do PR/MR resume o que muda por documento ("1 regra nova, 1 alterada"), lista os
  documentos novos, alterados e removidos e traz o link do `README.md` na branch: abra-o para
  conferir as páginas como vão ficar;
- na aba de arquivos, veja só as páginas afetadas; o texto entre os marcadores é o que o leitor vai
  ver;
- se o repositório tinha uma publicação antiga (um arquivo `.md` por documento, como na base), a
  primeira publicação nova troca esses arquivos pelas páginas, e o PR/MR avisa a migração; arquivos
  escritos à mão continuam;
- o `.cba-publicacao.json` lista as páginas que o app publicou; o app só remove do repositório o que
  está nele, e nunca apaga nem sobrescreve um arquivo escrito à mão (a publicação para com um
  conflito);
- comentários e commits de revisores na branch são preservados na próxima publicação.

Mudanças de conteúdo devem ser feitas no app (revisão e aprovação da análise) e publicadas de novo:
o README avisa que a documentação é gerada pelo app, e um ajuste feito direto no repositório é
sobrescrito na próxima publicação daquela página.

## 5. Sincronizar

Depois do merge, a documentação publicada chega à base de cada analista do projeto pela
sincronização:

- automaticamente, em segundo plano, ao iniciar uma análise (no máximo a cada 10 minutos);
- ou pelo botão **Sincronizar agora**, no cartão do destino.

A sincronização recorta as páginas pelos marcadores: cada regra, termo ou análise volta como um
documento próprio, com o caminho da página e a âncora de onde veio
(`funcionalidades/cobranca/f-cob-001-cobrar-multa.md#rn-cob-001`). O cartão mostra o commit,
quando sincronizou, quantos documentos vieram e o que foi pulado ou mascarado (segredos, CPF,
CNPJ, e-mail, telefone). Na tela da base, os documentos aparecem com o rótulo da origem e o filtro "Externos", só
para leitura. A sua própria base não recebe de volta o que ela mesma publicou: os documentos já
estão nela.

Se a sincronização falhar (rede, token, clone) ou a cópia tiver mais de 24 horas, o agente recebe
um aviso de que a documentação publicada pode estar desatualizada, e a análise continua.

## 6. Como a documentação volta para as análises

- Ao iniciar uma análise, o agente recebe até 3 títulos da documentação publicada relacionados ao
  assunto, como lembrete.
- Quando ajuda, ele consulta a documentação publicada pela ferramenta `conhecimento` e recebe os
  documentos rotulados `[doc. publicada]`, cada regra ou termo separado, com o repositório, o
  commit, a confiança, o trecho da página (`página#âncora`) e o link para ele na forja. A base do
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
| "N páginas importadas inteiras (marcadores ilegíveis)" no cartão do destino | Um marcador da página foi apagado ou editado à mão. Publique de novo pelo app para refazer a página. |
| Regra em `regras/<área>.md` e não na página da funcionalidade | Aceite a funcionalidade no mapa ou ligue a análise a ela na aprovação (ou em "Religar documentos") e publique de novo. |
