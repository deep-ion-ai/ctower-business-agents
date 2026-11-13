# Repositórios do projeto (código e issues)

Cada projeto pode ter, opcionalmente, **onde fica o código** (GitHub ou GitLab) e **onde ficam as
issues** (GitHub Issues, GitLab Issues ou Jira), em qualquer combinação. Sem eles o app funciona
como antes, só com as pastas locais.

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

O tipo do repositório de código e do de issues, e o host e o caminho no GitLab. O que ainda não foi
cadastrado pode ser acrescentado depois.

## Prontidão

Em Configurações, cada projeto mostra os passos de cada integração e a orientação de quem falhou.
Jira: `jira` instalado, configuração presente, `server` igual à URL, comando de credencial, `jira me`
e `jira serverinfo`. GitLab: `glab version` e `glab api version`. GitHub: `gh` e `gh auth token`. A
prontidão não bloqueia o app.

## Uso nas análises

- **Análise a partir de uma issue:** o campo “Issue do projeto” (`PROJ-123`, `45`) busca título e
  descrição como entrada. O link vai em `demanda.origem` e a chave em `demanda.chave`.
- **Evidências exportadas:** cada nota vira um link para o arquivo e as linhas no commit da
  evidência (ou, na falta dele, o `base.commit` da análise). Sem commit, a nota fica como texto.

Escrever no rastreador fica para a fase 2.
