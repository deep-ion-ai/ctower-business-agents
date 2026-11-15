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

O cadastro guarda só o **comando de credencial** (por exemplo `pass show jira | head -1`), nunca o
token. O comando roda com `/bin/sh -c`, sem entrada, sem terminal e com prazo de 10 s; vale a
primeira linha da saída. O token chega ao `jira`, ao `glab` e ao `gh` só no ambiente do processo
filho. Nada vai a disco, a log ou à configuração do Claude.

## Regras que não mudam depois do cadastro

Cada repositório de código já cadastrado (forja, host e caminho) não pode ser removido nem trocado;
outros podem ser acrescentados. No de issues, o tipo e, no GitLab, o host e o caminho. O que ainda
não foi cadastrado pode ser acrescentado depois.

## Prontidão

Em Configurações, cada projeto mostra os passos de cada integração e a orientação de quem falhou.
Jira: `jira` instalado, configuração presente, `server` igual à URL, comando de credencial, `jira me`
e `jira serverinfo`. GitLab: `glab version` e `glab api version`. GitHub: `gh` e `gh auth token`. A
prontidão não bloqueia o app.

## Uso nas análises

- **Análise a partir de uma issue:** o campo “Issue do projeto” (`PROJ-123`, `45`) busca título e
  descrição como entrada. O link vai em `demanda.origem` e a chave em `demanda.chave`.
- **Evidências exportadas:** cada nota vira um link para o arquivo e as linhas no commit da
  evidência (ou, na falta dele, o `base.commit` da análise), no repositório cuja pasta contém o
  arquivo (a mais funda, se um estiver dentro do outro); o caminho no link é relativo à pasta do
  repositório. Repositório sem pasta (cadastro antigo) vale para o projeto inteiro. Sem commit ou sem
  repositório, a nota fica como texto.

Escrever no rastreador fica para a fase 2.
