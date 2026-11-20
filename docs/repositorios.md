# Repositórios do projeto (código e issues)

Cada projeto pode ter, opcionalmente, **onde fica o código** (um ou vários repositórios, no GitHub
e no GitLab) e **onde ficam as issues** (GitHub Issues, GitLab Issues ou Jira), em qualquer
combinação. Sem eles o app funciona como antes, só com as pastas locais.

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

## Clone inicial

O app clona os repositórios (HTTPS ou SSH) numa pasta base configurável (preferências do app; padrão
`Documentos/ctower-business-repos`), uma subpasta por repositório, com progresso e erro claro por
repositório. Depois do clone, o app só atualiza o clone e escreve nele para publicar a documentação
(veja [Escrita no repositório](#escrita-no-repositório-275)); o resto (pull do código) fica com o
analista.

O token não vai para a URL, para os argumentos nem para o `.git/config`: o `git` recebe com `-c` um
credential helper que vale só para aquela execução e lê o token de uma variável de ambiente. URL de
clone com senha ou token embutido é recusada. Nenhuma execução pede nada no terminal
(`GIT_TERMINAL_PROMPT=0`, SSH em `BatchMode`); no SSH valem as chaves do analista.

"Testar acesso" roda `git ls-remote --heads` com o token digitado (ou o guardado) e confirma a
permissão de leitura antes de clonar.

## Destino da documentação (#274)

O projeto pode cadastrar onde a documentação aprovada é publicada (`repositorios.docs`, opcional;
projetos antigos abrem sem ele), na aba "Código e demandas":

- **Pasta num repositório de código** (`modo: subpasta`): `codigo` é o `host/caminho` (minúsculas)
  de um repositório de código já cadastrado e `subpasta` a pasta dentro dele (`docs/negocio`),
  relativa, sem `..` e sem `.git`. A branch base é a do repositório de código.
- **Repositório próprio** (`modo: repositorio`): os mesmos campos de um repositório de código
  (forja, host, caminho, URL de clone, pasta do clone, branch base e origem do token) e uma
  subpasta opcional. Não pode ser um dos repositórios de código. O clone usa a mesma pasta base dos
  outros.

A validação não usa a rede. O destino pode ser trocado ou removido a qualquer momento; ao mudar, o
inventário é refeito. A prontidão traz o destino por último: no repositório próprio, os passos de
um repositório de código; na subpasta, que o repositório de código continua cadastrado e que a
subpasta, se já existe, é uma pasta. O resultado aparece na saúde do projeto.

A pasta da documentação não é código-fonte: fica fora do inventário e das evidências (motivo
"documentação publicada", que uma reinclusão não desfaz) e do aviso de alteração. O clone do
repositório próprio conta como repositório cadastrado para o aviso, e nenhuma pasta do projeto pode
ficar dentro da pasta da documentação.

## Escrita no repositório (#275)

A publicação grava arquivos numa branch própria e abre um PR (GitHub, `gh api`) ou MR (GitLab,
`glab api`), devolvendo o link (`cba_repos::escrita::publicar`):

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
- **Credencial:** a mesma do clone. O `git` recebe o token pelo credential helper de `-c` e
  `CBA_GIT_TOKEN`; as CLIs, por `GH_TOKEN`/`GITLAB_TOKEN`. Nada na URL, nos argumentos nem no
  `.git/config`, e as mensagens trocam o token por `***`. Falta de permissão de escrita (no push ou
  na API) e CLI ausente têm erro próprio.

`cba_repos::escrita::atualizar_clone` busca o remoto e avança a branch base local só por
fast-forward: com a base no checkout, atualiza a pasta e recusa alterações não comitadas em
arquivos rastreados; base local e remota divergentes são conflito, resolvido pelo analista.

## Publicar os documentos aprovados (#276)

O botão "Publicar", ao lado do "Exportar" na tela da análise, publica a base aprovada do projeto no
destino da documentação (`cba_tools::publicacao`), nos dois modos:

- **Só o aprovado:** análises, documentos de funcionalidade e mapa com `status: aprovada`, regras
  `confirmada`, termos do glossário e sugestões `aceita`. Demandas, atas, análises de reunião,
  registros da demanda, pessoas, memórias e o `projeto.md` ficam de fora. Os `.md` vão como estão
  (com frontmatter), no layout da base, sob a subpasta do destino.
- **Prévia antes de confirmar:** o que muda na branch base do destino (novos, alterados e
  removidos), lido do remoto depois de um fetch, sem tocar no checkout (`cba_repos::escrita::ler_remoto`).
  Antes do botão, o aviso pede a conferência de segredos e dados pessoais.
- **Manifesto:** a pasta publicada leva `.cba-publicacao.json`, com o projeto e a lista do que o app
  publicou. Só os arquivos listados nele (na base ou na branch da publicação) são removidos quando
  deixam de estar aprovados. Um arquivo que já está na pasta sem ter sido publicado pelo app nunca é
  apagado nem sobrescrito: a publicação para com um conflito. Manifesto de outro projeto na mesma
  pasta também é recusado.
- **Branch e PR/MR:** `docs/cba-<projeto>`, pela escrita acima, com o link do PR/MR no resultado.
  Sem mudança na base nem publicação pendente, não há commit nem PR/MR.

## Regras que não mudam depois do cadastro

Cada repositório de código já cadastrado (forja, host e caminho) não pode ser removido nem trocado;
outros podem ser acrescentados. No de issues, o tipo e, no GitLab, o host e o caminho. O que ainda
não foi cadastrado pode ser acrescentado depois.

## Prontidão

Em Configurações, cada projeto mostra os passos de cada integração e a orientação de quem falhou.
Repositório de código: `git` instalado, token disponível e acesso de leitura (`git ls-remote`).
Jira: `jira` instalado, configuração presente, `server` igual à URL, comando de credencial, `jira me`
e `jira serverinfo`. GitLab de issues: `glab version`, token e `glab api version`. GitHub de issues:
`gh` e token (chaveiro ou `gh auth token`). A prontidão não bloqueia o app.

## Uso nas análises

- **Análise a partir de uma issue:** o campo “Issue do projeto” (`PROJ-123`, `45`) busca título e
  descrição como entrada. O link vai em `demanda.origem` e a chave em `demanda.chave`.
- **Evidências exportadas:** cada nota vira um link para o arquivo e as linhas no commit da
  evidência (ou, na falta dele, o `base.commit` da análise), no repositório cuja pasta contém o
  arquivo (a mais funda, se um estiver dentro do outro); o caminho no link é relativo à pasta do
  repositório. Repositório sem pasta (cadastro antigo) vale para o projeto inteiro. Sem commit ou sem
  repositório, a nota fica como texto.

Escrever no rastreador fica para a fase 2.
