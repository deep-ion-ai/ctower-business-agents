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
repositório. Só o clone inicial: atualizar (fetch/pull) fica com o analista.

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
