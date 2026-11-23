# Spike: `.claude/settings.local.json` no CLI e no Desktop (#303)

Épico #302. Pergunta: se o preparo passar os hooks e a proteção do `~/.claude/settings.json` para o `.claude/settings.local.json` da pasta cadastrada, as sessões do analista continuam recebendo tudo, e só elas?

**Resposta curta:** sim, com um ajuste no desenho. O Claude Code lê o `settings.local.json` de **dois lugares**: a pasta em que a sessão começou e a **raiz do repositório principal** (a mesma chave que o app já usa para o MCP e a confiança). Não sobe pelas pastas intermediárias. O app deve gravar na raiz da chave (`Alvo::chave`), não na pasta cadastrada.

## Versões e método

- Claude Code **2.1.330** (CLI), macOS 26 (Darwin 25.4).
- Claude Desktop **6.28899.6**.
- Script reproduzível: [`scripts/spike-settings-local.sh`](../../scripts/spike-settings-local.sh). Cria repositórios descartáveis numa pasta temporária e roda o `claude` com `CLAUDE_CONFIG_DIR` temporário, sem tocar na configuração do analista. Cada `settings.local.json` tem um hook `SessionStart` com um rótulo próprio, que o hook grava num log; o log diz quais arquivos a sessão leu.
- Sem login na configuração temporária, o `claude -p` termina com "Not logged in", mas o `SessionStart` e o `UserPromptSubmit` disparam antes disso. Os itens 1, 2, 3, 5 e 7 não precisam de chamada ao modelo. O item 6 (`--com-modelo`) faz uma chamada curta ao Haiku com a configuração logada do analista, numa pasta descartável.
- Item 5 em modo interativo, com `expect`.

Saída da execução de 2026-11-23:

```text
Claude Code 2.1.330 (Claude Code)
== 1. git
git status: []  check-ignore: ~/.config/git/ignore:1:**/.claude/settings.local.json	.claude/settings.local.json
== 2. pasta dentro de repositório (lê a pasta da sessão e a raiz do repositório)
sessão em r/modulo: modulo raiz
sessão em r/modulo/sub: raiz
sem git, sessão em fora/modulo: fora-modulo
sem git, sessão em fora/modulo/sub: (nenhum hook)
== 3. worktree (lê a pasta da sessão e a raiz do repositório principal)
worktree sem arquivo próprio: raiz
worktree com arquivo próprio: raiz wt
subpasta da worktree: raiz wt-modulo
== 7. convivência com o escopo de usuário
hook do analista no usuário + local: fora-modulo usuario
mesmo comando no usuário e no local: fora-modulo
disableAllHooks no usuário: (nenhum hook)
== 5. confiança (interativo)
sem confiança: (nenhum hook: o diálogo de confiança aparece antes)
com hasTrustDialogAccepted na pasta: fora-modulo
worktree, com hasTrustDialogAccepted na raiz principal: wt-modulo raiz
== 6. proteção (configuração logada do analista, Haiku)
resposta: NEGADO (...) | negadas: ['Edit', 'Write']
a.txt depois: x
```

## Resultados

### 1. Git

O arquivo criado por fora (como o app fará) **não aparece no `git status`** nesta máquina, mas por causa do ignore global do git (`~/.config/git/ignore`, com a linha `**/.claude/settings.local.json`), não do `.git/info/exclude` (que ficou intocado) nem do `.gitignore`. A documentação do Claude Code diz que, quando ele próprio cria o `settings.local.json`, configura o git para ignorá-lo; a linha no ignore global é o rastro disso (o arquivo tem só essa linha). Não foi possível reproduzir a criação pelo próprio Claude no ambiente isolado (exige login e uma ação interativa de permissão), então o mecanismo exato fica como observado, não como garantido.

**Consequência:** o app não pode contar com isso. Uma máquina em que o Claude nunca criou um `settings.local.json` (ou com `core.excludesFile` em outro lugar) não tem a linha. O app confere com `git check-ignore -q .claude/settings.local.json` na raiz; se não estiver ignorado, acrescenta `/.claude/settings.local.json` ao `info/exclude` do **diretório comum** do git (`git rev-parse --git-common-dir`, que vale também para as worktrees) e registra que foi ele, para tirar no desfazer. Nunca no `.gitignore`.

### 2. Pasta dentro de repositório maior

A sessão aberta em `repo/modulo` lê `repo/modulo/.claude/settings.local.json` **e** `repo/.claude/settings.local.json`. A sessão aberta em `repo/modulo/sub` lê **só** o da raiz: as pastas intermediárias não são lidas. Fora de repositório, só a pasta da sessão é lida; uma subpasta não lê nada de cima.

### 3. Worktree

A sessão numa worktree lê o arquivo da própria pasta da sessão e o da **raiz do repositório principal**, não o da raiz da worktree (a subpasta `wt/modulo` leu `wt-modulo` e `raiz`, não `wt`). Um arquivo só na raiz principal cobre todas as worktrees.

### 4. Desktop

A aba de código do Desktop 6.28899.6 roda o mesmo executável do Claude Code pelo Agent SDK. No pacote do app (`app.asar`), a sessão da aba de código é aberta com `settingSources: ["user","project","local"]`, o que inclui o `settings.local.json`; a carga com o diretório do projeto também usa as três fontes. Com o mesmo executável, valem os resultados 2, 3, 5 e 7.

Não confirmado ao vivo: abrir `claude://code/new?folder=<pasta>` só abre a aba, sem iniciar o processo do Claude até a primeira mensagem, e o teste exigiria dirigir a interface do Desktop na tela do analista. **A conferência ao vivo fica para a validação do PR final do épico** (abrir a pasta no Desktop, mandar uma mensagem, ver o estado ao vivo no app).

### 5. Confiança da pasta

Em modo interativo, sem confiança, o diálogo "Quick safety check" aparece antes de qualquer hook e avisa que a pasta "configures hooks that can run commands without asking, declared in .claude/settings.local.json". Os hooks só disparam depois do "Yes, I trust this folder". O `hasTrustDialogAccepted: true` que o app já grava em `projects["<chave>"]` basta: com ele, não há diálogo e os hooks disparam; numa worktree, a chave da raiz principal vale. No modo `-p` não há diálogo e os hooks disparam de qualquer forma.

### 6. Proteção

`Edit(//<pasta>/**)` em `permissions.deny` do arquivo local **nega** `Edit` e `Write` mesmo com `allow` de `Edit` e `Write` em outro escopo (`--settings`, de precedência maior que a do usuário), `--allowedTools Edit Write` e `--permission-mode acceptEdits`. O arquivo não mudou. O `deny` vale em qualquer escopo.

### 7. Convivência

- Hook do analista no escopo de usuário e o do app no local, no mesmo evento: **os dois rodam**.
- **O mesmo comando** nos dois escopos roda **uma vez só** (o Claude Code tira as duplicatas pelo comando). Na migração (#305), o período com a entrada global e a local ao mesmo tempo não duplica eventos, desde que o comando seja idêntico; e um arquivo na raiz do repositório e outro na pasta da sessão com o mesmo comando também não duplicam.
- `disableAllHooks: true` no escopo de usuário **desliga** os hooks locais. Um `disableAllHooks: false` no local os religa, mas o app não deve gravar isso: é escolha do analista. O app deve detectar o `disableAllHooks` e dizer, no `verificar`, que os hooks estão desligados.

## Recomendação para o preparo (#304)

Gravar os hooks e a proteção em **`<raiz>/.claude/settings.local.json`, onde `<raiz>` é o caminho da chave do escopo local** (`Alvo::chave`, calculado por `Alvo::da_pasta`): a raiz do repositório principal, ou a própria pasta fora do git. É a mesma regra do MCP e da confiança, que o app já segue.

| Caso de pasta cadastrada | Onde gravar | O que cobre |
|---|---|---|
| Raiz do repositório | `<repo>/.claude/settings.local.json` | sessões na raiz, em qualquer subpasta e em qualquer worktree |
| Pasta dentro de repositório (`repo/modulo`) | `<repo>/.claude/settings.local.json` (raiz) | sessões em `modulo` e nas subpastas dele (só a raiz é lida nas subpastas), e também em outras pastas do mesmo repositório: o receptor ignora as de fora do cadastro pelo `cwd`, como hoje |
| Worktree | `<repo principal>/.claude/settings.local.json` | a worktree lê a raiz do repositório principal; nada é gravado na worktree |
| Pasta fora de repositório | `<pasta>/.claude/settings.local.json` | só sessões abertas na própria pasta (como o MCP, que também é chaveado por ela) |

Por que não na pasta cadastrada (`repo/modulo/.claude/...`): só cobriria sessões abertas exatamente em `modulo`, não nas subpastas. Gravar nos dois lugares não ajuda: a raiz já cobre a pasta.

Consequências para a #304:

- **Um arquivo por raiz, não por pasta.** Projetos (ou pastas de um mesmo projeto) que compartilham o repositório compartilham o arquivo. O desfazer tira as entradas do app só quando nenhuma outra pasta preparada por esta instalação usa a mesma raiz (a mesma regra de hoje para os hooks globais, agora por raiz).
- **Proteção:** a regra continua `Edit(//<pasta cadastrada>/**)`, com o caminho da pasta, gravada no arquivo da raiz. Assim o `deny` não se estende ao repositório inteiro.
- **Git:** conferir com `git check-ignore` e, se preciso, usar o `info/exclude` do diretório comum (item 1).
- **Confiança:** sem mudança; o `hasTrustDialogAccepted` já está na mesma chave.
- **`verificar`:** avisar quando o `disableAllHooks` do usuário (ou de outro escopo) desliga os hooks.
- **Migração (#305):** gravar o comando idêntico ao global; a sobreposição não duplica eventos.

Nenhum item invalida o desenho do épico; a única mudança é o lugar do arquivo (raiz da chave em vez da pasta cadastrada).
