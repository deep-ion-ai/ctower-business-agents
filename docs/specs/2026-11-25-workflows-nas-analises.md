# Workflows do Claude Code nas análises: desenho e spike (#352)

Pergunta: dá para conduzir as análises grandes (impacto, revisão de requisito) por um workflow do Claude Code, com fases, agentes em paralelo e verificação independente, sem mudar o documento nem a revisão no app? E como o usuário customiza esses workflows sem perder a customização nas atualizações?

**Resposta curta:** sim. No Claude Code 2.1.331, um workflow nomeado em `.claude/workflows/<nome>.js` roda na sessão da análise, e os subagentes dele usam o mesmo servidor MCP `cba` da sessão. Os registros (`afirmar`, `registrar_achado`...) chegam como hoje, ligados à sessão pelos hooks. Os hooks também mostram o início, o `runId`, cada subagente e o fim, o que basta para o app acompanhar o progresso e retomar. O spike mostrou quatro restrições que moldam o desenho:

1. O pedido de rodar o workflow precisa estar **na mensagem do analista** (o `prompt_de_inicio`), não no roteiro devolvido pela ferramenta.
2. O preparo precisa liberar **`Workflow(<nome>)` e `Skill(<nome>)`**: o modelo às vezes chama o workflow pela ferramenta Skill.
3. A retomada só funciona **na mesma sessão** e **pelo nome** com `resumeFromRunId`. Pelo `scriptPath` ela cai na revisão de script dinâmico.
4. O `Stop` dispara com o workflow ainda rodando em segundo plano. Hoje o app marcaria a sessão como concluída no meio da análise.

## 1. Spike

### Versões e método

- Claude Code **2.1.331** (CLI), Linux. O Desktop não foi testado.
- Script reproduzível: [`scripts/spike-workflows.sh`](../../scripts/spike-workflows.sh) (`--interromper` para o teste de retomada). Ele cria um projeto descartável com dois workflows (`teste-analise`: duas fases, um agente por arquivo, cada um chama uma ferramenta MCP; `teste-lento`: uma fase rápida e uma lenta, para interromper), um servidor MCP mínimo que anota cada chamada e hooks que anotam o payload de cada evento. O `claude -p` roda com Haiku, com a configuração logada do analista e com `--settings`/`--mcp-config` isolados. Nada da configuração do analista é alterado. Cada execução custa menos de US$ 0,02, e o spike todo, menos de US$ 0,15.

### Resultados

| # | O que se verificou | Resultado |
|---|---|---|
| R1 | Workflow nomeado em `<projeto>/.claude/workflows/` | Encontrado pelo nome com `--setting-sources` contendo `project`. Ele aparece também como skill de mesmo nome. Só com `local`, não aparece (só os embutidos, como `deep-research`). |
| R2 | Pasta da sessão | Uma sessão numa subpasta do repositório encontra os workflows da raiz. Numa **worktree**, os da raiz do repositório principal **não** são encontrados, ao contrário do `settings.local.json` (spike da #303). |
| R3 | Permissões | Sem regra: `Review dynamic workflow before running` (ou o pedido de liberar a skill). `Workflow(teste-analise)` libera a chamada pelo nome. Só `Skill(teste-analise)` não basta. Com o pedido em texto livre, o modelo às vezes tenta a ferramenta Skill antes: com as duas regras, roda sempre. |
| R4 | Pedido no roteiro em vez de na mensagem | Não testado de propósito: a ferramenta Workflow só aceita ser chamada a pedido do usuário, nas palavras dele. O texto de um resultado de ferramenta não conta. |
| R5 | MCP nos subagentes | Os subagentes (`agent_type: workflow-subagent`) chamam o MCP da sessão. O mesmo processo atendeu as chamadas (mesmo pid), e os hooks vêm com o `session_id` da sessão principal e um `agent_id`. A ligação sessão → análise pelos hooks (#25) continua valendo. |
| R6 | O que os hooks mostram | `PreToolUse`/`PostToolUse` da ferramenta `Workflow`: o nome e os args. A resposta traz `runId`, `taskId`, `transcriptDir` e `scriptPath`. Depois vêm `SubagentStart`/`SubagentStop` de cada agente. O **`Stop` chega logo depois do lançamento, com `background_tasks: [{type: workflow, status: running}]`**. O fim chega como `UserPromptSubmit` com `<task-notification>` (status, resultado e uso: agentes, tokens, duração), seguido de outro `Stop`. |
| R7 | Fases | O `journal.jsonl` em `transcriptDir` traz uma linha `started` por agente (com `label` e `phase`) e uma `result` por agente. É formato interno do Claude Code, sem documentação. |
| R8 | Interrupção e retomada | Com a sessão morta no meio da fase lenta e reaberta com `claude --resume <sessão>`, a chamada `Workflow {name, resumeFromRunId}` devolveu do cache os dois agentes que tinham terminado, **sem chamar o MCP de novo**. Só o agente interrompido rodou outra vez (1 chamada MCP). A retomada pelo `scriptPath` (o que o modelo faz se não for orientado) cai em `Review dynamic workflow before running` com a regra estreita. |
| R9 | `-p` | O `claude -p` espera o workflow terminar e devolve o resultado, o que serve à avaliação (`cba-avaliacao`). |
| R10 | 2.1.329 | Não conclusivo: o modelo tentou a ferramenta Skill sem a regra. A versão mínima fica na verificada (2.1.331), com a detecção em tempo de execução da seção 3.8 como garantia. |

## 2. Princípios

- O workflow é **outra forma de conduzir**, não outro tipo de análise. O documento, a revisão, a aprovação e as ferramentas de registro continuam as mesmas. A análise guarda só qual condução foi usada e o estado da execução.
- A **condução simples continua sendo a referência**. Todo tipo com workflow precisa funcionar sem ele, e a falta do recurso nunca bloqueia a análise.
- O **app controla o custo**: os limites (número de frentes, esforço) vão nos args, e o script respeita esses limites.
- O que o usuário customiza é **dele**: o app nunca sobrescreve uma cópia, só avisa quando o padrão mudou.

## 3. Desenho

### 3.1 Workflows padrão

Embutidos no binário (`crates/cba-tools/textos/workflows/<tipo>.js`, com `include_str!`, como os roteiros), com o nome `cba-<tipo>` para não colidir com os workflows do projeto.

**`cba-impacto`** (Analisar impacto):

| Fase | Agentes | O que faz | Registra com |
|---|---|---|---|
| Preparar | 1 | `retomar_analise` e a demanda com os comentários (`ler_demanda`, #351). Separa as premissas do solicitante, localiza as funcionalidades afetadas (`mapa`, `conhecimento`, `referencias`) e devolve as frentes de investigação (schema), no máximo `limites.frentes`. Na retomada, devolve só as frentes sem `investigado` na memória. | `memorizar` (frentes) |
| Investigar | 1 por frente (pipeline) | Verifica no código as premissas e as regras da frente e levanta o que muda (funcionalidades, dados, integrações, documentos e telas), os dados pessoais e os riscos. | `afirmar`, `perguntar`, `propor_termo`, `memorizar` (`investigado`) |
| Verificar | 1 por frente (pipeline, logo depois da investigação dela) | Cético independente: relê a evidência de cada afirmação da frente e tenta refutá-la. O que não se sustenta é contestado. | `contestar_afirmacao` (nova, ver abaixo) |
| Consolidar | 1 | Agrupa o que se repete, registra a abrangência e os riscos transversais, e atualiza `demanda_como_entendida` e `onde_parou`. | `afirmar`, `memorizar` |

A sessão principal conclui. Quando o workflow termina, ela chama `concluir_analise`, trata as pendências e dá a resposta no chat como o roteiro manda hoje. Assim, a conversa com o analista continua na sessão, e o fim é o mesmo da condução simples.

**`cba-revisar`** (Revisar requisito): Separar (1 agente: os critérios de aceite e as afirmações do texto, até `limites.frentes`) → Avaliar (1 por critério: ambiguidade, testabilidade e regra conflitante na base e no código, com `registrar_achado`, `perguntar` e `afirmar`) → Verificar (1 por critério: refuta cada achado, com `contestar_achado`) → Consolidar (1: INVEST com `registrar_invest` e veredito com `registrar_veredito`).

**Histórias e cenários:** ficam para a sub-issue D, com o mesmo molde (gerar → revisar a cobertura dos critérios → consolidar). **Perguntar e explicar** continuam sem workflow.

**Ferramentas novas** (as únicas mudanças no MCP): `contestar_afirmacao` e `contestar_achado` (`analise`, id, `motivo`, evidência opcional). Elas marcam o item como **a revalidar**, com a origem "verificação independente", no mesmo estado que a conferência de hash do `concluir_analise` já usa. O item aparece como pendência na resposta do `concluir_analise` e na revisão do app. Nada é apagado: quem decide é o analista.

**Contrato dos args** (o mesmo para o padrão e para as cópias):

```js
// args: "A-0042" (só o id, no início) ou o objeto abaixo (montado pelo app)
{ analise: "A-0042", retomada: false, limites: { frentes: 6, esforco: "medium", esforcoVerificacao: "low" } }
```

O script aceita os dois formatos (`typeof args === 'string'`), porque o `prompt_de_inicio` não pode ter aspas (`claude.cmd` do Windows, spec 6.4). Os limites ausentes usam os padrões do próprio script. O modelo dos agentes não é fixado: eles herdam o da sessão.

### 3.2 Como a sessão chama o workflow

Pela regra R4, o pedido fica no `prompt_de_inicio`, que é a mensagem do analista que o app digita no terminal ou passa no link:

> Inicie a análise A-0042 pelo ctower: chame a ferramenta iniciar_analise do cba com analise A-0042 e conducao workflow. Depois chame a ferramenta Workflow com name cba-impacto e args A-0042 e siga o roteiro devolvido.

Ele continua sem aspas, `%`, `!`, parênteses e quebras de linha. Nomear a ferramenta Workflow evita o desvio pela ferramenta Skill (R3).

O `iniciar_analise` ganha `conducao: simples | workflow` (padrão `simples`). Com `workflow`, devolve um **roteiro curto**: o workflow faz a investigação; ao fim, confira as pendências, chame `concluir_analise` e responda no chat como no roteiro do tipo (a seção "Resposta no chat" é reaproveitada). Ele traz também o caminho de volta: se a ferramenta Workflow não existir nesta sessão ou for recusada, chame `iniciar_analise` de novo com `conducao simples` e siga o roteiro completo (seção 3.8).

### 3.3 Instalação pelo preparo

- **Onde:** `<raiz>/.claude/workflows/cba-<tipo>.js`, em que `<raiz>` é o `git rev-parse --show-toplevel` de **cada pasta cadastrada**, ou a própria pasta fora do git. Pela R2, a raiz do repositório principal não cobre as worktrees: é diferente da chave usada para o `settings.local.json`.
- **O que se grava:** a versão **efetiva** do tipo (seção 3.5), com um cabeçalho de comentário que diz que o arquivo é gerado pelo app, a origem (padrão, global ou projeto) e o SHA-256 do conteúdo. O arquivo instalado é um artefato, não o original.
- **Git:** uma linha `/.claude/workflows/cba-*.js` no `info/exclude` do diretório comum, com o mesmo mecanismo e registro da #304. Nunca no `.gitignore`.
- **Permissões:** `Workflow(cba-<tipo>)` e `Skill(cba-<tipo>)` no `allow` do `settings.local.json` da raiz da chave (o mesmo arquivo dos hooks), uma regra por tipo instalado. Não se usa `Workflow` sem nome, que liberaria qualquer script dinâmico.
- **Hooks:** acrescentar `PostToolUse` (com `matcher: "Workflow"`), `SubagentStart` e `SubagentStop` aos `EVENTOS`. É o mesmo comando de hoje, e o receptor ignora o que não reconhece.
- **Consentimento:** os arquivos, as linhas e as regras entram no plano do `planejar` como itens novos (`Item::Workflow`, `Item::PermissaoWorkflow`). O `desfazer` tira exatamente o que pôs.
- **Divergência:** antes de regravar, o `verificar` compara o arquivo instalado com o hash do cabeçalho. Se o analista editou o arquivo instalado direto, o preparo **não sobrescreve**: avisa e oferece levar a edição para uma cópia customizada (seção 3.5).
- **Reinstalação:** a versão efetiva muda quando o app é atualizado (padrão novo), quando a cópia é salva ou restaurada e no preparo. Ao abrir uma análise com workflow, o app confere o hash e reinstala o que estiver defasado, sem novo consentimento se os itens do plano forem os mesmos já consentidos.

### 3.4 Execução, progresso e estado da sessão

- **Estado da análise:** `Analise.conducao` (`simples` por padrão, com `serde(default)`, sem subir o formato) e `Analise.workflow: Option<ExecucaoWorkflow { nome, origem, hash, run_id, sessao, situacao, fases: [{titulo, iniciados, concluidos}], uso }>`.
- **Receptor de hooks:** no `PostToolUse` de `Workflow`, grava `run_id`, `sessao` e `transcriptDir`, ligados à análise da sessão. `SubagentStart`/`SubagentStop` com `agent_type: workflow-subagent` atualizam as contagens. O `UserPromptSubmit` com `<task-notification>` de `type` workflow fecha a execução (situação e uso).
- **Estado da sessão (R6):** um `Stop` com `background_tasks` contendo um workflow `running` mantém a sessão **ativa**, e não concluída, em `destino()`. É a única mudança no mapeamento da #25.
- **Fases na tela:** o painel da análise mostra as fases do `meta.phases` com "n de m agentes". O app lê o `journal.jsonl` do `transcriptDir` quando ele muda, só as linhas `started`/`result` com `phase`. Como o formato é interno (R7), uma leitura que falhe cai nas contagens dos hooks sem as fases, e nunca é erro. Os registros continuam aparecendo no documento ao vivo, como hoje.
- **Continuar no Desktop:** fica bloqueado enquanto houver um workflow `running`, com o motivo. O `/desktop` encerra o CLI (#28), e a tarefa em segundo plano iria junto.

### 3.5 Customização pelo usuário

- **Onde ficam as cópias:**
  - projeto: `<base>/_templates/workflows/<tipo>.js` (versionada com a base, como os roteiros editáveis);
  - todos os projetos: `<dados do app>/workflows/<tipo>.js`.
- **Precedência:** projeto > global > padrão embutido. É o mesmo molde do `editavel.rs`, com uma camada a mais.
- **Configurações › Workflows:** lista por tipo de análise, com a origem efetiva em cada projeto. Ações: ver o padrão (só leitura), **duplicar** (para o projeto ou para todos), editar a cópia no editor embutido (#106), em modo JavaScript, com prévia das fases em vez da prévia de Markdown, **restaurar o padrão** (apaga a cópia, depois de confirmar) e, para a cópia, ver a diferença com o padrão atual.
- **Atualizações do app:** o padrão vem do binário e a cópia nunca é tocada. A cópia guarda num comentário inicial o hash do padrão de que partiu. Quando o padrão muda, a tela avisa ("o padrão mudou desde a sua cópia") e mostra a diferença. Não há mesclagem automática.
- **Validação ao salvar** (Rust, sem Node). Erros com linha e mensagem clara:
  - o arquivo começa por `export const meta = {...}` e o objeto é literal puro (extraído e lido como JSON5);
  - `name` é igual a `cba-<tipo>` (o app preenche ao duplicar e recusa a troca), e `description` não é vazia;
  - `phases` não é vazio, os títulos são únicos e toda chamada `phase('...')` com literal tem título em `meta.phases`;
  - não há `Date.now()`, `Math.random()` nem `new Date()` sem argumento (quebram a retomada);
  - o script lê `args` (aviso, não erro);
  - não há anotações de tipo TypeScript óbvias (`: string`, `interface`): aviso;
  - tamanho de até 256 KiB.

  O que a análise estática não pega (erro de sintaxe JavaScript) aparece na execução. Nesse caso, a análise cai na condução simples com aviso (seção 3.8) e mostra o erro na tela de Configurações.

### 3.6 Escolha ao iniciar e custo

- Em **Nova análise** (e em "Analisar esta demanda"), nos tipos com workflow, aparece **Condução: Simples | Aprofundada (workflow)**, com a estimativa ao lado. A escolha inicial é a última que o usuário fez para o tipo; na primeira vez, Simples.
- **Limites padrão** (Configurações › Workflows, por tipo): `frentes` 6 (no máximo 12), `esforco` `medium` para investigar, `low` para verificar. Com 6 frentes, o impacto sobe no máximo 1 + 6 + 6 + 1 = **14 agentes**.
- **Estimativa:** `agentes = 2 + 2 × frentes` (o pior caso). Os tokens por agente vêm de uma tabela embutida por tipo e fase, medida com o `cba-avaliacao` (`rodar --conducao workflow`, que o `-p` permite, R9). A tela mostra uma faixa de tokens e de tempo e a razão em relação à condução simples do mesmo tipo ("cerca de 3× a simples"). Não mostra valor em dinheiro, porque o plano e o modelo do analista variam. Antes da medição, a tela mostra só o número máximo de agentes.

### 3.7 Retomada

- **Mesma sessão** (`--resume`, o caminho padrão de reabertura): o `prompt_de_retomada` com workflow pede `retomar_analise` e, se `Analise.workflow.situacao` não for `concluido`, a chamada `Workflow` com `name cba-<tipo>` e `resumeFromRunId <run_id>` gravado pelo hook. Pela R8, o que terminou volta do cache sem registro duplicado e só o que faltava roda de novo.
- **Conversa nova** (outra sessão, Desktop pelo link, sessão perdida): o `resumeFromRunId` não vale ("same-session only"). O workflow roda de novo com `args {analise, retomada: true}`. A fase Preparar lê a memória e devolve só as frentes sem `investigado`, e as frentes concluídas não são refeitas. Os registros que já existem ficam e não são duplicados.
- **Script mudou** (cópia editada ou padrão novo, ou seja, o hash difere do de `Analise.workflow`): o cache só vale até a primeira chamada alterada. A retomada avisa e segue como conversa nova.
- **Análise retomada pela condução simples:** é sempre possível. A tela oferece "Continuar sem workflow", que chama `retomar_analise` com o roteiro completo.

### 3.8 Versão mínima e falta do recurso

- **No app, antes de oferecer:** a opção Aprofundada fica desabilitada, com o motivo, quando a versão do `claude` é conhecida e é anterior a **2.1.331** (`VERSAO_MINIMA_WORKFLOWS`, no padrão do `terminal::desktop`), quando o preparo do projeto não tem os workflows instalados ("Preparar de novo") ou quando o cliente escolhido é o Desktop por link, que não foi verificado.
- **Na sessão:** se a ferramenta Workflow não existir, for recusada ou falhar ao carregar o script (`--safe-mode`, política da organização, erro de sintaxe na cópia), o roteiro curto manda chamar `iniciar_analise` com `conducao simples`. A análise passa a `conducao: simples` com o aviso "Conduzida sem workflow: <motivo>", visto na tela da análise.
- **Rede de segurança do app:** se a sessão chega ao primeiro `Stop` sem `background_tasks` de workflow, sem nenhum `PreToolUse` de `Workflow` e com a análise ainda em `conducao: workflow`, o app mostra o aviso e oferece "Continuar sem workflow".

## 4. Sub-issues propostas

Na ordem de execução. A ordem da issue muda: a instalação vem antes da execução, que depende dela.

**A. Instalação pelo preparo** (o item 2 da issue)
- O preparo instala `cba-impacto` (padrão embutido, ainda sem customização) nas raízes da seção 3.3, com o cabeçalho e o hash, a linha no `info/exclude` e as regras `Workflow(...)`/`Skill(...)`.
- Os hooks `PostToolUse` (matcher `Workflow`), `SubagentStart` e `SubagentStop` entram nos `EVENTOS`. A migração da #305 acrescenta os eventos às instalações existentes.
- O plano mostra os itens novos, e o `desfazer` tira exatamente o que pôs (teste byte a byte, como na #24).
- O `verificar` acusa um arquivo instalado divergente e não o sobrescreve.

**B. Execução do workflow de impacto** (o item 1 da issue)
- O script `cba-impacto.js` da seção 3.1, as ferramentas `contestar_afirmacao` e `contestar_achado`, e o `iniciar_analise` com `conducao`, com o roteiro curto.
- O `prompt_de_inicio` e o `prompt_de_retomada` com workflow. `Analise.conducao` e `Analise.workflow`, gravados pelo receptor de hooks.
- O `Stop` com workflow `running` mantém a sessão ativa. O painel mostra as fases. O "Continuar no Desktop" fica bloqueado durante a execução.
- A escolha em Nova análise, com os limites padrão e o número máximo de agentes, e o fallback da seção 3.8.
- O `cba-avaliacao rodar --conducao workflow`: uma rodada nos casos de impacto do gabarito, comparando com a condução simples (cobertura, contestações e tokens), e a tabela de estimativa a partir dela. O relatório fica em `docs/avaliacao/`.

**C. Tela de customização** (o item 3 da issue)
- Configurações › Workflows: ver, duplicar (projeto ou global), editar no editor embutido, restaurar, ver a diferença e o aviso de padrão novo.
- A validação da seção 3.5, testada com scripts quebrados de propósito, e a reinstalação quando a versão efetiva muda.

**D. Revisar requisito e demais tipos** (o item 4 da issue)
- `cba-revisar` com o mesmo molde. A avaliação no caso REV-FAT-001 decide se ele vira opção.
- Histórias e cenários só se a medição da B mostrar ganho no impacto.

> **Resultado (#373):** o `cba-revisar` foi implementado e medido no REV-FAT-001 ([relatório](../avaliacao/2026-11-25-workflow-revisar-sonnet.md)): mesma cobertura da condução simples, que já atinge o teto do gabarito, com cerca de 2,9 vezes os tokens. A opção Aprofundada não aparece em Nova análise para a revisão; o workflow continua instalado e customizável. A medição da B ([relatório](../avaliacao/2026-11-25-workflow-impacto-sonnet.md)) não mostrou ganho no impacto, então Histórias e Cenários ficam sem workflow. Detalhes em `docs/avaliacao.md`, seção "Condução pelo workflow".

## 5. Fora do escopo e riscos

- **Formato interno:** o `journal.jsonl` e o texto do `<task-notification>` não são documentados. O app só os usa para o progresso, com o recurso às contagens dos hooks, e nunca para o resultado da análise, que está nos registros do MCP.
- **Desktop:** não verificado. A primeira versão oferece o workflow só no terminal do app.
- **Custo em contas com limite de uso:** um agente recusado por limite de uso para a fase. A retomada pelo nome refaz só o que faltou (R8).
- **Windows:** o caminho do `.claude/workflows/` e o `claude.cmd` com o `prompt_de_inicio` novo precisam de um teste na sub-issue A/B, como o `desktop_pty`.
