# Repositórios do projeto (código, issues e documentação)

Cada projeto pode ter, opcionalmente, **onde fica o código** (um ou vários repositórios, no GitHub
e no GitLab), **onde ficam as issues** (GitHub Issues, GitLab Issues ou Jira) e **onde a
documentação aprovada é publicada** (uma pasta num repositório de código ou um repositório próprio),
em qualquer combinação. Sem eles o app funciona como antes, só com as pastas locais.

O passo a passo do analista para publicar e reaproveitar a documentação está no
[guia da publicação](guia-publicacao.md); esta página descreve o comportamento.

## Repositórios de código

Cada repositório de código (`repositorios.codigo`, #178) guarda a forja (`github` ou `gitlab`), o
host, o caminho (`dono/repo`, `grupo/sub/app`), a URL de clone, a pasta local (opcional), a branch e
de onde vem o token: `global` (do host), `proprio` (só deste repositório) ou `comando` (o comando que
imprime o token, em `comando_credencial`). Dois repositórios não podem ter o mesmo host e caminho.

O cadastro antigo, com uma forja só (`repositorios.forge`), é lido como uma lista de um repositório,
sem pasta: o GitHub com token `global` (o `gh auth`) e o GitLab com token `comando`. Na próxima
gravação, só a forma nova é escrita.

Uma pasta do projeto é de código-fonte (fica fora do aviso de alteração, #519) quando algum remoto
do repositório git dela aponta para **qualquer** repositório cadastrado. Os remotos são lidos do
`.git/config`, sem executar o git.

A prévia do cadastro mostra o remoto de cada pasta e a forja, o host e o caminho deduzidos dele. Ao
escolher uma pasta, `detectar_repositorios_app` lista os repositórios git nela: ela mesma, se é um,
ou as subpastas imediatas que são.

O modelo vem do Control Tower (`deep-ion-ai/ctower-agents`): os tipos, os nomes de campo e as regras
do Jira são os mesmos. O código está em `crates/cba-repos` (sem Tauri), copiado de `ct-core`,
`ct-proc` e `ct-ledger`. Diferenças daqui: o GitLab também é rastreador, e o GitHub guarda o
`dono/repositório`, porque o projeto não é o repositório.

## Credenciais

O cadastro nunca guarda o token. Há três origens (#179):

- **Token global do host** (`token: global`): um PAT no chaveiro do sistema (Keychain no macOS,
  Gerenciador de Credenciais no Windows, keyutils no Linux), na conta `host:<host>` do serviço
  `ai.deep-ion.ctower-business-agents`. Vale para todos os repositórios daquele host; trocá-lo vale
  para todos de uma vez.
- **Token só deste repositório** (`token: proprio`): conta `repo:<host>/<caminho>`. Tem precedência
  sobre o global.
- **Comando de credencial** (`token: comando`; no Jira e no GitLab de issues, quando preenchido): o comando (por exemplo
  `pass show jira | head -1`) roda com `/bin/sh -c`, sem entrada, sem terminal e com prazo de 10 s;
  vale a primeira linha da saída.

Os mesmos tokens servem às issues: o GitHub recebe o token global do `github.com` em `GH_TOKEN` (sem
ele, vale o `gh auth login`); o GitLab de issues sem comando de credencial usa o token global do
host; o Jira sem comando de credencial usa o token do host do Jira (conta `host:<host do Jira>`).
No cadastro, "Testar conexão" do Jira roda os passos da prontidão com o token digitado, sem gravá-lo.
O token chega ao `git`, ao `jira`, ao `glab` e ao `gh` só no ambiente do processo filho. Nada
vai a disco, a log ou à configuração do Claude.

### Seção Acessos (#193)

Em Configurações › Acessos ficam os tokens globais de cada host do GitHub e do GitLab (quem os usa,
o último teste e a validade, Testar, Trocar, Remover com confirmação que lista os afetados e
"+ Adicionar host") e os repositórios com token próprio, com atalho para o projeto. No lugar do
token do host dá para usar um **comando que imprime o token**: ele fica no chaveiro, na conta
`comando:host:<host>`, e vale para tudo que usaria o token global (clone, issues, prontidão) quando
não há token guardado. Guardar um token apaga o comando do host, e vice-versa.

"Testar" o token do host usa a API da forja pelas CLIs, com o token só no ambiente:
`gh api -i user` (a validade vem do cabeçalho `github-authentication-token-expiration`) e
`glab api user` mais `glab api personal_access_tokens/self` (`expires_at`). Sem a CLI, o teste é o
`git ls-remote` num repositório do host. O resultado (sucesso, quando, mensagem sem o token, dono e
validade) e os hosts acrescentados à mão ficam nas preferências do app; o token, nunca. Trocar ou
remover o token apaga o último teste daquele token.

## Clone e atualização

O app clona os repositórios (HTTPS ou SSH) numa pasta base configurável (preferências do app; padrão
`Documentos/ctower-business-repos`), uma subpasta por repositório, com progresso e erro claro por
repositório. "Testar acesso" roda `git ls-remote --heads` com o token digitado (ou o guardado) e
confirma a permissão de leitura antes de clonar.

O token não vai para a URL, para os argumentos nem para o `.git/config`: o `git` recebe com `-c` um
credential helper que vale só para aquela execução e lê o token de uma variável de ambiente
(`CBA_GIT_TOKEN`). URL de clone com senha ou token embutido é recusada. Nenhuma execução pede nada
no terminal (`GIT_TERMINAL_PROMPT=0`, SSH em `BatchMode`); no SSH valem as chaves do analista.

Depois do clone, o app só mexe no clone do destino da documentação: atualiza para sincronizar e
escreve numa branch própria para publicar ([Documentação publicada](#documentação-publicada-273)).
Atualizar o código (pull) fica com o analista. A atualização (`cba_repos::escrita::atualizar_clone`)
busca o remoto e avança a branch base local só por fast-forward: com a base no checkout, atualiza a
pasta e recusa alterações não comitadas em arquivos rastreados; base local e remota divergentes são
conflito, resolvido pelo analista.

## Regras que não mudam depois do cadastro

Cada repositório de código já cadastrado (forja, host e caminho) não pode ser removido nem trocado;
outros podem ser acrescentados. No de issues, o tipo e, no GitLab, o host e o caminho. O que ainda
não foi cadastrado pode ser acrescentado depois. O destino da documentação pode ser trocado ou
removido a qualquer momento.

## Prontidão

Em Configurações, cada projeto mostra os passos de cada integração e a orientação de quem falhou.
Repositório de código: `git` instalado, token disponível e acesso de leitura (`git ls-remote`).
Jira: `jira` instalado, configuração presente, `server` igual à URL, comando de credencial, `jira me`
e `jira serverinfo`. GitLab de issues: `glab version`, token e `glab api version`. GitHub de issues:
`gh` e token (chaveiro ou `gh auth token`). Destino da documentação, por último: no repositório
próprio, os passos de um repositório de código; na subpasta, que o repositório de código continua
cadastrado e que a subpasta, se já existe, é uma pasta. O resultado aparece na saúde do projeto. A
prontidão não bloqueia o app.

## Uso nas análises

- **Análise a partir de uma issue:** o campo “Issue do projeto” (`PROJ-123`, `45`) busca título e
  descrição como entrada. O link vai em `demanda.origem` e a chave em `demanda.chave`.
- **Status das demandas (#292):** a demanda com `chave` do rastreador do projeto (e `origem` dele,
  quando houver) tem o estado conferido em lote — `gh api graphql`, `glab api
  projects/:id/issues?iids[]=…` e `jira issue list --jql "key in (…)"` — ao ser importada, ao abrir
  a lista de demandas, o painel da demanda e o da funcionalidade (com cache de 5 minutos) e pelo
  botão "Atualizar status". O estado vai para o bloco `rastreador` do `demanda.md` (`estado`:
  `aberta`, `em-andamento` ou `encerrada`; `conferido`; `encerrado`, a data de fechamento), sem
  mudar o `status` local, e passa a valer para a demanda estar aberta ou encerrada. No Jira, o estado
  sai da categoria do status (`new`, `indeterminate`, `done`), não do nome. O arquivo só é regravado
  quando o estado muda ou o dia vira. Sem rede ou sem token, a tela mostra o último estado conhecido
  com um aviso. A demanda sem `chave` mantém o status local. Nada é escrito no rastreador.
- **Issues do projeto (#322):** o comando `issues_do_projeto_app` lista as issues do rastreador
  (GitHub e Jira; os últimos 6 meses, como em "Vincular demandas") a partir de um cache em
  `<dados>/issues/<projeto>.json`, com o instante da última leitura. Com cache, abrir a lista não
  chama `gh` nem `jira`: a leitura com mais de 5 minutos é refeita em segundo plano, e o evento
  `issues-do-projeto-atualizadas` avisa a tela quando termina; depois dela, as demandas ligadas às
  issues são conferidas como acima, com o mesmo cache. Sem cache, a primeira leitura é feita na
  hora; `forcar` ("Atualizar") também. Busca por número (`41`, `#41`, `PROJ-41`) ou título, sem
  diferença de acentos; filtros por estado no rastreador (abertas, o padrão; encerradas; todas),
  por rótulo e pela situação na base (`sem-analise`, `em-andamento`, `em-revisao`, `aprovada`, pela
  análise mais recente da demanda ou da chave da issue). Cada issue traz a demanda que a
  representa e as funcionalidades ligadas a ela. Sem rede, a lista é a última leitura, com o aviso.
  Trocar o rastreador do projeto descarta o cache. Nada é escrito no rastreador.
- **Área Demandas e "Analisar impacto" (#323):** a seção Demandas mostra numa lista só as issues
  acima e as demandas criadas à mão, com busca, filtros, painel da issue (descrição, link para o
  rastreador, o que há na base, itens parecidos) e o frescor ("conferido há 5 min", "Atualizar").
  A ação da linha acompanha a situação: "Analisar impacto" (sem análise), "Abrir a análise" (em
  andamento ou em revisão) ou "Abrir a demanda" (aprovada). O comando `analisar_impacto_da_issue_app`
  parte da issue da última leitura, cria a demanda da base com a chave e o endereço dela ou reaproveita
  a que existe (pela chave), cria a análise de impacto ligada a ela, com o título e a descrição como
  entrada, e a tela abre a análise com o agente. Issue encerrada no rastreador não é analisada. O
  vínculo com as funcionalidades parte da própria área ("Vincular a funcionalidades"); o Mapa não
  tem mais o botão. No formulário de impacto, "Issue do projeto" busca nesta mesma lista.
- **Issue da demanda pelo agente (#351):** a ferramenta MCP `ler_issue_da_demanda` (só leitura) lê
  no rastreador a issue de onde a análise nasceu (`demanda.chave` do frontmatter, quando a `origem`
  é deste rastreador): título, descrição em Markdown, situação, responsáveis, rótulos, comentários
  (autor, data e texto, do mais antigo ao mais recente; no Jira, o wiki vira Markdown), anexos
  (nome e tipo, sem baixar; no GitHub e no GitLab, os links de upload do texto) e issues ligadas
  (Jira e GitLab). Usa `jira issue view --raw --comments`, `gh issue view --json …,comments` e
  `glab api …/issues/N`, `…/notes` (sem as notas do sistema) e `…/links`, com o token do resolvedor
  só no ambiente da CLI: o agente recebe o conteúdo, nunca a credencial, e os erros passam pelo
  mascaramento. Limites: os 30 comentários mais recentes (o agente pode pedir até 100), 4 mil
  caracteres por comentário, 40 mil nos comentários somados (saem os mais antigos) e 20 mil na
  descrição, sempre com aviso do corte. Ela só lê a issue da análise informada; outra issue do
  projeto não é lida. Sem rede ou sem token, a recusa diz o motivo e o agente segue com a entrada.
  Com issue no rastreador, o roteiro da análise (`iniciar_analise` e `retomar_analise`) termina com a
  orientação de ler os comentários no início e citar o comentário (autor e data) que mudar o
  entendimento do pedido.
- **Evidências exportadas:** cada nota vira um link para o arquivo e as linhas no commit da
  evidência (ou, na falta dele, o `base.commit` da análise), no repositório cuja pasta contém o
  arquivo (a mais funda, se um estiver dentro do outro); o caminho no link é relativo à pasta do
  repositório. Repositório sem pasta (cadastro antigo) vale para o projeto inteiro. Sem commit ou sem
  repositório, a nota fica como texto.
- **Documentação publicada:** consultada pelos agentes sob demanda, como pista
  ([Consulta pelos agentes](#consulta-pelos-agentes-278)).

Escrever no rastreador (comentário ou anexo na issue) fica para a fase 2.

## Documentação publicada (#273)

A exportação grava um arquivo avulso (Markdown, Word, HTML ou CSV) para mandar a alguém. A
publicação leva a base aprovada do projeto para um repositório git como páginas legíveis e
navegáveis na forja (README, funcionalidades, regras, glossário e análises, com permalinks para o
código), revisadas por PR/MR, e a traz de volta para as análises seguintes, de qualquer analista do
projeto. O guia do analista está em [`guia-publicacao.md`](guia-publicacao.md). O ciclo:

1. **Destino** cadastrado no projeto (#274).
2. **Publicar** os documentos aprovados numa branch própria, com PR/MR (#275, #276).
3. **Revisar e mesclar** o PR/MR, fora do app, como qualquer mudança no repositório.
4. **Sincronizar** o repositório de docs com a base, como fonte externa só leitura (#277).
5. **Consultar:** os agentes pedem a documentação publicada quando ela ajuda, e a evidência
   continua sendo o código (#278).

### Destino (#274)

O destino fica em `repositorios.docs` (opcional; projetos antigos abrem sem ele), no cartão "Onde a
documentação é publicada" da aba "Código e demandas":

- **Pasta num repositório de código** (`modo: subpasta`): `codigo` é o `host/caminho` (minúsculas)
  de um repositório de código já cadastrado e `subpasta` a pasta dentro dele (`docs/negocio`),
  relativa, sem `..` e sem `.git`. A branch base é a do repositório de código.
- **Repositório próprio** (`modo: repositorio`): os mesmos campos de um repositório de código
  (forja, host, caminho, URL de clone, pasta do clone, branch base e origem do token) e uma
  subpasta opcional. Não pode ser um dos repositórios de código. O clone usa a mesma pasta base dos
  outros.

A validação não usa a rede. Ao trocar o destino, o inventário é refeito; ao removê-lo, a cópia em
`externos/` é apagada.

A pasta da documentação não é código-fonte: fica fora do inventário e das evidências (motivo
"documentação publicada", que uma reinclusão não desfaz) e do aviso de alteração. O clone do
repositório próprio conta como repositório cadastrado para o aviso, e nenhuma pasta do projeto pode
ficar dentro da pasta da documentação.

### Escrita no repositório (#275)

`cba_repos::escrita::publicar` grava os arquivos numa branch própria e abre um PR (GitHub,
`gh api`) ou MR (GitLab, `glab api`), devolvendo o link:

- **Branches protegidas:** `develop`, `main`, `master` e a própria branch base nunca são branch de
  escrita (sem diferenciar maiúsculas); o push é sem `--force` e só avança a branch.
- **O clone do analista fica como está:** o commit é montado num índice temporário a partir da
  branch remota, sem checkout, sem trocar a branch atual e sem criar branch local. Alterações não
  comitadas fora dos arquivos publicados não atrapalham; num arquivo publicado, a publicação é
  recusada ("clone sujo"). O autor é o do git do analista; sem `user.name`/`user.email`,
  `ctower-business-agents`. Os hooks de pre-push não rodam (`--no-verify`).
- **Republicar:** com PR/MR aberto, o commit novo vai em cima da ponta da branch remota, inclusive
  dos commits de revisores, e o PR/MR é o mesmo. Sem PR/MR, uma branch já mesclada na base recomeça
  da base e uma não mesclada é preservada (e ganha o PR/MR). Se a branch anda no remoto durante a
  publicação, o push é recusado e o erro pede para publicar de novo.
- **Sem mudanças efetivas** (o conteúdo já está na branch do PR/MR aberto ou na base), não há
  commit, push nem PR/MR novo, e o resultado diz isso.
- **Credencial:** a do clone; o token precisa de permissão de escrita no repositório e de abrir
  PR/MR. As CLIs recebem o token só no ambiente (`GH_TOKEN`/`GITLAB_TOKEN`), e as mensagens trocam
  o token por `***`. Falta de permissão de escrita (no push ou na API) e CLI ausente têm erro
  próprio.

### Publicar os documentos aprovados (#276, #289)

O botão "Publicar", ao lado do "Exportar" na tela da análise, publica a base aprovada do projeto
inteiro (não só a análise aberta) no destino (`cba_tools::publicacao`), nos dois modos:

- **Só o aprovado:** análises, documentos de funcionalidade e mapa com `status: aprovada`, regras
  `confirmada`, termos do glossário e sugestões `aceita`. Demandas, atas, análises de reunião,
  registros da demanda, pessoas, memórias e o `projeto.md` ficam de fora.
- **Páginas legíveis (#288, #289):** o que vai para a subpasta do destino são as páginas do
  renderizador (`cba_docs::gerar_documentacao`), com os marcadores de ida e volta de cada documento
  e os permalinks das evidências (pelo escopo do projeto):

  ```text
  README.md                               entrada: índice por área → funcionalidade, contagens
  funcionalidades/<área>/<id>-<slug>.md   funcionalidade aceita no mapa: descrição, "Como funciona"
                                          (o negocial), regras, termos, análises, pontos de entrada
  funcionalidades/<área>/<tema>-<doc>.md  documento negocial sem funcionalidade aceita
  regras/<área>.md                        regras sem funcionalidade aceita
  glossario.md                            termos, com "Usado em"
  analises/README.md, analises/<id>.md    análises por tipo e cada análise legível
  ```

  Cada documento fica numa página só (a canônica): a regra na página da primeira funcionalidade
  aceita dela (pelo campo `funcionalidades`, #287, ou herdado da análise de origem) ou, sem ela, em
  `regras/<área>.md`; o termo no glossário; a análise na dela. Nas outras páginas, só link. Cada
  documento tem uma âncora (`#rn-cob-001`), e as evidências viram permalinks no commit analisado
  (`/blob/<commit>/<arquivo>#L12-L14` no GitHub, `/-/blob/<commit>/<arquivo>#L12-14` no GitLab).
  O marcador de cada documento é um comentário HTML, invisível na forja, com o tipo, o id, o
  caminho na base e o frontmatter completo em JSON:
  `<!-- cba:regra id=RN-COB-001 {"caminho":"regras/RN-COB-001.md","frontmatter":{…}} -->` …
  `<!-- /cba -->` (`cba_kb::marcador`, reexportado em `cba_docs::marcador`). O README traz a data da última mudança registrada nos documentos aprovados (`criado`,
  `atualizado`), não a da publicação: publicar de novo sem mudança não muda nada.
- **Prévia antes de confirmar:** por documento ("2 regras novas, 1 alterada; 1 termo novo",
  comparando os blocos marcados das páginas com os da branch base) e as páginas afetadas, lidas do
  remoto depois de um fetch, sem tocar no checkout (`cba_repos::escrita::ler_remoto`). Com um PR/MR
  pendente, a prévia compara também com a ponta da branch da publicação e avisa quando ele já tem
  esta versão (nada a enviar). Sem documento aprovado, diz que ainda não há aprovados. Antes do
  botão, o aviso pede a conferência de segredos e dados pessoais.
- **Manifesto:** a pasta publicada leva `.cba-publicacao.json` (`versao: 2`), com o projeto e a
  lista das páginas que o app publicou. Só os arquivos listados nele (na base ou na branch da
  publicação) são removidos quando deixam de ser gerados. Um arquivo que já está na pasta sem ter
  sido publicado pelo app nunca é apagado nem sobrescrito: a publicação para com um conflito.
  Manifesto de outro projeto na mesma pasta, ou de uma versão mais nova do app, também é recusado.
- **Migração do formato antigo:** sobre uma publicação da #276 (manifesto sem `versao`, com os `.md`
  da base), os arquivos listados nele saem e as páginas entram; os arquivos alheios ficam. Na
  prévia, cada `.md` antigo conta como o documento dele: com o mesmo frontmatter, "só muda de
  formato".
- **Branch e PR/MR:** `docs/cba-<projeto>`, pela escrita acima, com o link do PR/MR no resultado e,
  no corpo do PR/MR, o resumo por documento e o link do README na branch. Sem mudança na base nem
  publicação pendente, não há commit nem PR/MR.

### Sincronização (#277)

`cba_tools::docs_externos::sincronizar` traz a documentação publicada de volta para a base como fonte
externa rotulada:

- **Quando:** pelo botão "Sincronizar agora" no cartão do destino e, em segundo plano, ao iniciar
  uma análise (se a última tentativa tem mais de 10 minutos). A análise nunca espera: se a cópia
  tem mais de 24 horas, nunca foi sincronizada ou a última tentativa falhou (rede, credencial,
  clone), o agente recebe um aviso, e as cópias anteriores continuam valendo.
- **O quê:** a atualização do clone e os `.md` da subpasta lidos do commit da branch base do remoto
  (`git ls-tree`/`cat-file`, sem checkout), copiados para `<projeto>/externos/<repositório>/` na
  base com o tipo `doc-externo` (as páginas com marcadores, um documento por bloco: veja
  [Páginas da documentação legível](#páginas-da-documentação-legível-290)): frontmatter gerado com a origem (repositório, caminho, commit,
  branch), a confiança (`publicada` para documento com `tipo` do app, `externa` para o resto) e o
  frontmatter original, se havia. Documento sem frontmatter entra (o título vem do `# título` ou do
  nome do arquivo). Só o que mudou (pelo blob) é lido de novo; o que saiu do repositório sai da
  base e do índice. O estado fica em `externos/.sincronizacao.json`.
- **Por que copiar:** o índice continua sendo o espelho reconstruível da base (sem rede nem clone),
  o conteúdo é o do commit mesmo com o clone noutra branch, e o que entra já está tratado.
- **Não duplica a base:** um bloco (ou um `.md` no formato antigo) com `tipo` e `id` (ou `termo`)
  de um documento desta base (o que ela mesma publicou) não é copiado. Por isso a documentação publicada faz diferença nas bases dos
  outros analistas do projeto, ou numa base nova; na base de quem publicou, os documentos já estão.
- **Segredos e dados pessoais:** caminhos com padrão de segredo do inventário e arquivos com chave
  privada ficam de fora; credencial em URL, tokens conhecidos, valor de campo de senha, CPF válido
  e e-mail são mascarados. O que foi pulado ou mascarado aparece no cartão.
- **Só leitura e sem efeito na base:** o documento externo não se edita nem se aprova no app, não
  vira regra nem termo e fica fora da busca padrão do índice (`Externos::Excluir`); a tela da base
  o mostra com o rótulo da origem e o filtro "Externos".

#### Páginas da documentação legível (#290)

As páginas geradas pela #288 juntam vários documentos (as regras de uma funcionalidade, o
glossário). A sincronização as recorta pelos marcadores (`<!-- cba:<tipo> id=<id> {...} -->` …
`<!-- /cba -->`, módulo `cba_kb::marcador`):

- **Um documento externo por bloco**, em
  `externos/<repositório>/<página sem .md>/<tipo>-<id>.md`, com o `tipo` e o `id` do marcador, o
  frontmatter original (o do marcador) e o texto legível do bloco como corpo. A origem aponta para o
  trecho: caminho da página com a âncora (`…/f-cob-001-cobrar-multa.md#rn-cob-001`) e o link na
  forja no commit sincronizado; o `conhecimento` mostra os dois. O texto fora dos blocos (cabeçalho,
  índice) não vira documento.
- **Por bloco**: bloco que mudou é regravado (o que não mudou fica com a cópia e o commit de antes),
  bloco que saiu da página sai da base; mascaramento e chave privada valem por bloco; o bloco cujo
  `tipo` e `id` (ou, sem `id`, como no negocial, o caminho na base de origem) esta base já tem não é
  copiado.
- **Sem marcador** (README, documentação escrita à mão) ou no **formato antigo** (um `.md` por
  documento, com frontmatter): um documento externo por arquivo, como antes. Quando o repositório
  passa ao formato novo, as cópias antigas saem na sincronização seguinte.
- **Marcador malformado** (sem fechamento, aninhado, metadados ilegíveis): a página entra inteira,
  como um documento só, e o cartão do destino mostra o aviso. A sincronização não falha.

### Consulta pelos agentes (#278)

Os agentes consultam a documentação publicada como pista, sob demanda, sem que ela substitua a
evidência de código:

- **`conhecimento`:** o parâmetro `externos` escolhe a fonte: `excluir` (padrão, só a base
  própria), `incluir` (as duas) ou `somente`. Sem pedir, a resposta diz quantos documentos da
  documentação publicada casam com o termo (`externos_disponiveis`). Cada documento externo vem
  rotulado `[doc. publicada]`, com o repositório, o commit e a confiança (o bloco recortado de uma
  página, também com o trecho `página#âncora` e o link dele na forja), e nos dados com
  `fonte: "externa"` e a `origem` (com `ancora` e `url`); os da base própria têm `fonte: "base"`. Os limites são os de
  sempre (20 itens por página, até 6.000 caracteres). O filtro `tipos` vale para a base própria; a
  documentação publicada vem inteira quando pedida.
- **Peso no ranking:** com `incluir`, a relevância (BM25) de um documento externo é multiplicada
  por 0,5 (`cba_kb::indice::PESO_DOS_EXTERNOS`): com relevância parecida, a base própria vem antes.
- **Contexto de `iniciar_analise`:** as listas de regras, termos, memórias e análises continuam só
  com a base própria. À parte, no máximo 3 documentos da documentação publicada
  (`LIMITE_DE_EXTERNOS`), só o título, a origem e o caminho, com o total encontrado e o lembrete de
  que são pista; o resto o agente pede com `conhecimento`.
- **Evidência é código:** `afirmar` (e as demais ferramentas que conferem evidência) recusa a pasta
  de docs do projeto e a cópia em `externos/` com um motivo próprio, que manda citar o código que o
  documento descreve. `confirmado` continua exigindo ao menos uma evidência de código.
- **Roteiros:** perguntar, explicar, impacto e revisar orientam a consultar a documentação
  publicada (um conceito por chamada, porque a busca exige todas as palavras), a confirmar no código, a tratar divergência como pergunta e a dizer ao analista,
  no chat, as fontes externas usadas (repositório e commit); o de reunião cita a fonte na ligação
  com o código.

### Avaliação do ciclo (#279)

O cenário `publicacao` da avaliação (`cargo run -p cba-avaliacao -- rodar --fluxos publicacao`)
roda o ciclo inteiro no Faturamento Exemplo: analisa, aprova, publica num repositório bare local
(PR numa forja falsa, dado como mesclado), sincroniza numa base nova e analisa de novo, conferindo
que o agente recebeu da documentação publicada uma regra recortada de página, com a âncora (#291). Veja
[Ciclo de publicação](avaliacao.md#ciclo-de-publicação), na avaliação.
