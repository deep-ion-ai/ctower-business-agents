# ctower-business-agents: especificação do MVP standalone

- **Data:** 2026-11-11
- **Status:** proposta, aguardando revisão
- **Repositório:** `deep-ion-ai/ctower-business-agents`

## 1. Contexto e objetivo

Analistas de requisitos e de negócio que atuam em sistemas legados trabalham sem acesso prático à única documentação confiável desses sistemas: o código-fonte e o banco de dados. Documentos de especificação estão desatualizados ou não existem, e as pessoas que conheciam as regras saíram.

Hoje o uso de IA por esse público costuma ser um chat genérico: cola-se um trecho de documento ou de e-mail e recebe-se uma resposta convincente, sem nenhuma relação verificável com o que o sistema faz de fato. Não há gestão do que foi perguntado, nem rastreabilidade, nem reaproveitamento do que foi descoberto.

**Objetivo:** um aplicativo desktop, instalado e configurado pelo próprio analista, que organiza e governa análises sobre um sistema legado (perguntas, regras de negócio de uma funcionalidade, impacto de uma demanda), conduzidas pela IA que o analista já assina, **a partir do código**, com **evidência citada para cada afirmação**, e que guarda o resultado em **documentos padronizados** numa base de conhecimento local.

O projeto nasce da experiência com o [ctower-agents](https://github.com/deep-ion-ai/ctower-agents) (orquestração de agentes de código) e de bases de conhecimento de engenharia reversa mantidas à mão em Markdown. É um produto separado, para outro público, e começa deliberadamente simples.

### Critérios de sucesso

1. Um analista sem experiência com terminal, com o Claude Desktop instalado e uma assinatura pessoal, instala o app, cadastra o sistema e recebe a primeira análise com evidências em menos de 15 minutos, sem ajuda de alguém de TI. Um analista que prefere o Claude Code no terminal faz o mesmo sem abrir mão do terminal.
2. Toda afirmação de uma análise aponta para pelo menos um trecho de código, schema ou documento que a sustenta, ou está marcada explicitamente como "a confirmar".
3. Uma análise de impacto de uma demanda típica sai como documento padronizado, guardado na base do analista e exportável (Markdown e Word), sem que o analista escreva prompt.
4. Uma análise retomada dias depois, em outra sessão de IA, continua de onde parou, sem perder afirmações, evidências ou perguntas.

### Princípios

- **Simples antes de completo.** Um usuário, uma máquina, sem servidor nosso, sem conta nossa. Cada passo de instalação ou configuração a mais é um ponto de abandono.
- **Usa a assinatura que o analista já tem.** A maioria desse público usa contas pessoais de IA; cobrar por API inviabiliza a adoção. A conversa acontece no aplicativo oficial do provedor; o nosso app não oferece login nem intermedeia credenciais.
- **O app governa, a IA conversa.** Demandas, análises, documentos e base de conhecimento são do app. A conversa pode acontecer em mais de um cliente de IA sem perder nada.
- **O analista escolhe como conversar, e o app recomenda o CLI.** Claude Code no terminal embutido é a **experiência completa**: conversa, documento ao vivo, evidências e governança numa janela só, no nosso app. Claude Desktop atende quem não quer terminal, ao custo de alternar entre dois aplicativos. Os dois têm as mesmas ferramentas, o mesmo roteiro e os mesmos documentos.
- **Fluxos guiados, não prompt livre.** O analista escolhe o que quer fazer ("analisar impacto", "explicar funcionalidade"); o roteiro é do produto.
- **Sem evidência, não é fato.** O agente distingue o que viu no código do que está inferindo, e a interface deixa essa diferença visível.
- **O produto analisa, não desenvolve; o analista é responsável pelo repositório.** O prompt orienta o agente a não alterar código e a registrar sugestões de alteração. O app não bloqueia ferramentas por padrão: avisa quando a pasta do sistema foi alterada. A proteção técnica existe como opção.
- **Documentos padronizados por construção.** O agente registra dados estruturados; o app monta os documentos a partir de templates. O padrão não depende da obediência do agente.
- **Arquivos são a fonte da verdade.** A base de conhecimento é uma pasta de Markdown com frontmatter, legível e útil mesmo sem o app. O banco local é só índice.
- **O analista assina o resultado.** O agente produz rascunhos; o documento final é revisado e aprovado por uma pessoa.

## 2. Público

- **Analista de requisitos / analista de sistemas:** recebe demandas de mudança em sistemas legados e precisa descrever o que muda, levantar regras existentes e escrever histórias com critérios de aceite. Lê pouco ou nenhum código.
- **Analista de negócio / product owner:** precisa entender como o sistema trata um caso de negócio hoje ("o que acontece com um contrato cancelado no meio do ciclo de faturamento?").
- **Usuário secundário:** desenvolvedor que valida uma análise e confere as evidências.

Ambiente típico: notebook (Windows ou macOS), assinatura pessoal de IA, acesso de leitura ao repositório do sistema.

## 3. Escopo

### MVP

- App desktop (Windows x64 e macOS), instalação por usuário, sem permissão de administrador.
- Cadastro de **sistemas**: um sistema é uma ou mais pastas locais com código-fonte, scripts de banco e documentos. O analista aponta para pastas que já existem na máquina.
- **Base de conhecimento** em Markdown padronizado, numa pasta escolhida pelo analista (seção 7).
- **Mapeamento estrutural** do sistema (inventário, linguagens, rotinas, tabelas, referências), sem IA.
- **Servidor MCP local** do app, com as ferramentas de análise, a validação de evidências e o acesso à base de conhecimento.
- **Dois clientes, à escolha do analista** (preferência padrão nas configurações, trocável a cada análise), ambos com o ambiente já preparado:
  - **Claude Code CLI no terminal embutido (recomendado):** experiência completa num aplicativo só (seção 6.4); também no terminal do próprio analista;
  - **Claude Desktop** (aba de código), aberto pelo app por link, para quem não quer terminal; a conversa fica no Desktop e o documento no nosso app (seção 6.3).
- Três fluxos guiados:
  1. **Perguntar ao sistema:** pergunta livre, resposta com evidências.
  2. **Explicar funcionalidade:** regras de negócio, entradas, saídas, validações e casos de exceção de uma funcionalidade.
  3. **Analisar impacto de uma demanda:** a partir do texto da demanda, o que muda, onde, quais regras existentes são afetadas, riscos e perguntas para o solicitante.
- **Sugestão de alteração** como saída de primeira classe: quando o analista pede "corrige isso", o agente registra a proposta para o time de desenvolvimento em vez de alterar o código.
- **Documentos padronizados** por tipo, gerados pelo app a partir de templates editáveis, com revisão, aprovação e exportação para Markdown e Word.
- **Governança das sessões** pelos hooks do Claude Code: estado (em andamento, aguardando o analista, concluída), identificador da sessão e retomada.
- **Aviso de alteração no repositório:** se a pasta do sistema tiver arquivos alterados depois de uma sessão, o app mostra quais, o diff e a opção de desfazer.

### Fase 2 (ainda standalone)

- **Laço de agente próprio** no app, com as mesmas ferramentas: ChatGPT Plus/Pro pelo programa "Sign in with ChatGPT" da OpenAI e, para uso corporativo, chave de API da Anthropic, Amazon Bedrock ou Google Vertex AI.
- Integração com Jira e GitLab/GitHub para ler a demanda pela chave e publicar o documento como comentário ou anexo.
- Fluxos **revisar história** e **gerar critérios de aceite e cenários de teste**.
- **Importação de bases existentes** em Markdown (catálogos de regras, casos de uso, memórias de engenharia reversa), com revalidação das evidências contra o código atual.
- Conexão a um banco de dados para extrair o schema, além de arquivos DDL.
- Busca semântica (embeddings) sobre a base de conhecimento, só se a busca textual se mostrar insuficiente (seção 7.5).

### Edição comercial (EE), futura e condicionada à adoção

Fora deste repositório. Só registrada aqui para orientar decisões do núcleo:

- Servidor compartilhado, multiusuário, com a mesma lógica do núcleo.
- Base de conhecimento compartilhada por equipe e por sistema.
- Controle de acesso, trilha de auditoria centralizada, políticas de uso.
- Uso de assinatura pessoal em produto pago exige acordo com o provedor (seção 9); a EE deve assumir chave de API ou provedor de nuvem como padrão.

### Fora de escopo

- Desenvolvimento: gerar ou alterar código, abrir PR, fazer push.
- Execução do sistema analisado ou de testes.
- Uso multiusuário ou sincronização entre máquinas (a base pode ficar numa pasta sincronizada ou num repositório git, por conta do analista).
- Embutir, instalar ou modificar o binário do Claude Code, ou intermediar o login do analista em qualquer provedor.

## 4. Fluxos

Os exemplos usam um sistema fictício de faturamento ("Faturamento Exemplo").

### 4.1 Primeiro uso

1. O analista instala o app e o abre.
2. O analista escolhe como prefere conversar com a IA. O app apresenta o Claude Code no terminal embutido como recomendado ("tudo numa janela só") e o Claude Desktop como opção para quem não quer usar terminal ("conversa no Desktop, documento aqui"). O app verifica se o cliente escolhido está instalado e logado; se não estiver, mostra o passo a passo oficial (página de download do Desktop ou comando de instalação do Claude Code) e pede que o analista faça login com a própria conta, pelo fluxo do próprio cliente.
3. O analista escolhe a pasta da **base de conhecimento** (padrão sugerido: `Documentos/ctower-business`). Pode ser uma pasta sincronizada ou um repositório git próprio.
4. O analista cadastra o sistema: nome e uma ou mais pastas. O app mostra o que vai analisar (contagem de arquivos por tipo) e o que vai ignorar (binários, dependências, arquivos com segredos), e permite ajustar.
5. O app prepara o ambiente do sistema, com consentimento (seção 6.2): registra o servidor MCP e os hooks. Nenhum arquivo é criado dentro do repositório do sistema.
6. O mapeamento estrutural roda com barra de progresso (sem IA, sem custo).
7. O app mostra o mapa do sistema e sugere uma primeira análise.

### 4.2 Iniciar uma análise

1. O analista escolhe o fluxo e informa a entrada (pergunta, funcionalidade ou texto da demanda).
2. O app cria a análise (identificador, fluxo, entrada) e o documento em rascunho na base.
3. O app abre o cliente escolhido, na pasta do sistema, com um pedido curto já preenchido ("Inicie a análise A-0193 pelo ctower"):
   - **Claude Desktop:** por link; o analista confirma a pasta e envia (duas confirmações do próprio Desktop, que não podem ser puladas);
   - **Claude Code no terminal embutido:** o app inicia a sessão já com o pedido;
   - **Claude Code no terminal do analista:** o app oferece o comando pronto para copiar (`cd <pasta> && claude "Inicie a análise A-0193 pelo ctower"`) ou o link `claude-cli://`, que abre o terminal com o pedido preenchido.
4. Em qualquer caso, a sessão recebe o mesmo ambiente: o MCP e os hooks estão na configuração do Claude para aquela pasta, não no jeito de abrir.
5. O agente chama `iniciar_analise`, recebe o roteiro do fluxo e o contexto da base, e trabalha.
6. Enquanto isso, o app acompanha pelos hooks (sessão ativa, aguardando o analista, concluída) e atualiza o documento a cada afirmação registrada.

### 4.3 Perguntar ao sistema

Exemplo: "Como é calculada a multa por atraso de pagamento?"

O agente consulta a base e o mapa, lê os trechos que decidem o comportamento e registra cada regra com `afirmar`. No chat, o analista recebe um resumo com as regras numeradas, um exemplo com números e as perguntas em aberto, mais o link para o documento completo. No app, o analista marca afirmações como **confirmadas** (viram regras na base), **incorretas** (com comentário) ou pede aprofundamento.

### 4.4 Explicar funcionalidade

O resultado é um documento negocial da funcionalidade: objetivo, atores e telas, fluxo principal, fluxos alternativos, exceções, regras de negócio (cada uma com evidência), dados lidos e gravados, integrações e perguntas em aberto.

### 4.5 Analisar impacto de uma demanda

Exemplo de demanda: "Permitir parcelamento da fatura em até 3 vezes para clientes pessoa jurídica."

O roteiro pede ao agente que:

- separe o que o solicitante afirmou como fato do que ele supôs ("acho que", "salvo engano") e verifique as afirmações no código;
- descreva como o sistema trata o assunto hoje;
- levante o que muda em funcionalidades, dados, integrações, documentos e telas;
- registre riscos, como dados já gravados no formato antigo, integrações que recebem o formato atual e relatórios que agrupam pelos dados alterados;
- estime a abrangência (pequena, média ou grande) com justificativa e faça as perguntas para o solicitante.

Uma análise de impacto bem feita costuma **corrigir a premissa da demanda**: mostrar que parte do que o solicitante pediu já existe, ou que o impacto real é menor (ou maior) do que ele supôs. O documento deve deixar isso explícito na seção "Demanda como entendida".

### 4.6 Pedido de alteração

Quando o analista pede "corrige", "muda" ou "implementa", o agente não altera o código. Ele explica o que mudaria e registra uma **sugestão de alteração** com o trecho atual, o trecho proposto, o motivo e o impacto, incluindo exemplos com números. A sugestão entra no documento da análise, para o time de desenvolvimento. Se o analista insistir, a decisão é dele: o agente diz exatamente o que alterou e o app mostra o aviso de alteração no repositório.

### 4.7 Retomar uma análise

- **Mesma conversa:** o app reabre a sessão pelo identificador capturado nos hooks, no cliente escolhido: `claude --resume <id>` no terminal ou `claude --desktop --resume <id>` no Claude Desktop (este exige o Claude Code de linha de comando instalado e a sessão fechada no outro cliente). As sessões do CLI e do Desktop ficam no mesmo histórico local, então uma conversa iniciada num cliente pode continuar no outro.
- **Conversa nova (caminho garantido):** o app abre uma sessão nova com o pedido "Retome a análise A-0193". O agente chama `retomar_analise` e recebe a entrada, as afirmações, as evidências, as perguntas e as sugestões registradas até ali.

## 5. Arquitetura

```
┌──────────────────────────────── App desktop (Tauri) ─────────────────────────────────┐
│  UI (web): sistemas, fluxos, análises, documentos, base de conhecimento               │
│     │ comandos Tauri                                                                 │
│  cba-app (casca: janelas, terminal embutido opcional, diálogos)                      │
│     │                                                                                │
│  cba-core ── domínio, casos de uso, eventos                                          │
│     ├── cba-kb ────── base de conhecimento: leitura e escrita dos Markdown, índice   │
│     ├── cba-docs ──── templates e montagem dos documentos, exportação (MD, Word)     │
│     ├── cba-tools ─── ferramentas de análise e validação de evidências               │
│     ├── cba-mcp ───── servidor MCP local (expõe cba-tools)                           │
│     ├── cba-index ─── inventário, extratores, mapa do sistema                        │
│     └── cba-session ─ preparo do ambiente, links do Desktop, hooks, retomada         │
└──────────────────────────────────────────────────────────────────────────────────────┘
        │ MCP (stdio) + hooks                │ MCP (stdio) + hooks           │ fase 2
  Claude Desktop, aba de código        Claude Code CLI (terminal         laço próprio
                                       embutido ou do analista)
        │ leitura                            │
  pastas do sistema analisado ───────────────┘          base de conhecimento (Markdown)
```

- **O servidor MCP é o centro do produto.** Ferramentas, validação de evidências e acesso à base vivem nele. Cada cliente de IA é só uma porta de entrada, e todos escrevem na mesma análise.
- **Um executável só.** O servidor MCP e o receptor dos hooks são o próprio executável do app, iniciado em modo `mcp` ou `hook` pelo cliente de IA. Os processos compartilham o índice SQLite (modo WAL) e o app é notificado das mudanças.
- **O núcleo não depende do Tauri.** `cba-core` e as crates abaixo dele não conhecem a UI, o que permite testes sem interface e, no futuro, um servidor EE com o mesmo núcleo.
- **Linguagem:** Rust no backend e a mesma pilha web de UI do ctower-agents.
- **Reaproveitamento do ctower-agents:** sem dependência entre os repositórios. Trechos úteis (PTY, hooks, retomada de sessão, execução de processo) são copiados e adaptados, com comentário de origem no topo do arquivo.

## 6. Agente e clientes

### 6.1 Ferramentas MCP

Operam só dentro das pastas cadastradas do sistema, respeitam a lista de exclusões e têm limites de tamanho. O cliente de IA usa também as suas próprias ferramentas de leitura e busca; as nossas acrescentam o que ele não tem.

| Ferramenta | O que faz |
|---|---|
| `iniciar_analise` | Devolve o roteiro do fluxo, a entrada e o contexto da base (regras, termos e memória da área relacionados). Recebe a análise criada pelo app (`analise`) ou, quando o analista pede direto no chat, cria uma a partir de `fluxo` e `entrada`. |
| `retomar_analise` | Devolve o estado de uma análise: entrada, afirmações, evidências, perguntas, sugestões. |
| `listar_analises` | Lista as análises do sistema (identificador, título, fluxo, estado, documento), para achar a que o analista citou pelo assunto. |
| `conhecimento` | Busca na base: regras confirmadas, glossário, análises anteriores. |
| `mapa` | Retorna o mapa do sistema ou de um módulo. |
| `estrutura` | Lista rotinas, procedures, classes e telas de um arquivo. |
| `schema` | Consulta tabelas, colunas e relacionamentos extraídos dos scripts de banco. |
| `referencias` | Onde um identificador (rotina, tabela, campo, componente) é usado. |
| `afirmar` | Registra uma afirmação: `secao`, texto, confiança e evidências (6.5). |
| `perguntar` | Registra uma pergunta em aberto para o solicitante. |
| `registrar_sugestao` | Registra uma sugestão de alteração: arquivo, trecho atual, trecho proposto, motivo, impacto. |
| `propor_termo` | Propõe uma entrada para o glossário (termo de negócio, sinônimos, onde vive no código). |
| `concluir_analise` | Fecha a rodada: confere as pendências (sem afirmações ou seção obrigatória do fluxo vazia recusam; evidência que não confere mais com o código e afirmação inferida sem evidência voltam como pendência para o analista), o app monta o documento e devolve o caminho, que o agente cita na resposta, com as perguntas em aberto. |

Os fluxos guiados são ferramentas (`iniciar_analise`), e não prompts MCP, porque o suporte a prompts de servidores locais no Claude Desktop não está confirmado.

O servidor identifica o sistema pela variável `CBA_SISTEMA`, que o preparo do ambiente grava no registro do servidor (6.2), ou, sem ela, pelo diretório de trabalho do processo (a pasta em que o cliente o iniciou): é o sistema cadastrado que tem uma pasta contendo esse diretório. Numa worktree do git, vale o caminho correspondente no repositório principal, e as ferramentas leem e conferem as evidências nos arquivos da worktree, que são os que o agente vê. Fora de um sistema cadastrado, o servidor conecta e lista as ferramentas, e cada chamada responde com um erro que diz ao agente o que pedir ao analista. Argumentos fora do schema e recusas (evidência que não confere, análise inexistente) voltam como resultado de erro da ferramenta, com o motivo, para o agente corrigir a chamada.

### 6.2 Preparo do ambiente

Feito pelo app no cadastro do sistema, com consentimento, sem criar arquivos no repositório do sistema:

- **Servidor MCP:** registrado na configuração do usuário do Claude Code com escopo local da pasta do sistema. A aba de código do Claude Desktop e o Claude Code de linha de comando leem a mesma configuração. Atenção: numa worktree do git, o escopo local é gravado na entrada do repositório principal, não da worktree. O app deve registrar pelo caminho que o Claude Code usa como chave e remover o registro quando o sistema for descadastrado.
- **Hooks:** `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `Notification`, `Stop` e `SessionEnd`, apontando para o executável do app. Também são compartilhados entre o Desktop e o CLI. Ficam no escopo do usuário e disparam em todas as sessões; o receptor ignora, sem gravar nada, as que não estão na pasta de um sistema cadastrado (pelo `cwd` do evento ou pelo `CLAUDE_PROJECT_DIR`; uma worktree conta como o repositório principal). Nas demais, grava o estado da sessão em `<dados>/sessoes/<session_id>.json` (sistema, análise, estado, pasta para a retomada) e, quando a sessão já está ligada a uma análise, também no estado da análise. A ligação acontece no `PreToolUse` da primeira ferramenta do `cba` chamada com o argumento `analise` (normalmente `iniciar_analise` ou `retomar_analise`).
- **Edição da configuração do usuário:** a gravação preserva o que já existe e é desfeita no descadastro. O app nunca lê nem grava credenciais.

### 6.3 Cliente: Claude Desktop

- O app abre `claude://code/new?folder=<pasta do sistema>&q=<pedido>`. O Desktop abre uma sessão na aba de código com o pedido preenchido.
- O Desktop **sempre** pede confirmação de pasta recebida por link e **nunca** envia o pedido sozinho. São duas ações do analista, aceitas como custo do desenho.
- O roteiro não vai no link: os limites entram pelo hook `SessionStart` e o roteiro do fluxo pela resposta de `iniciar_analise` (seção 8).
- A aba de código roda a própria cópia do Claude Code, com versão que pode diferir da do CLI. O app não depende de recursos de uma versão específica.
- Funciona só com assinatura do Claude (não com chave de API nem provedores de nuvem) e só no macOS e no Windows x64.
- **Dois aplicativos:** a conversa acontece no Desktop e o documento, as evidências, a revisão e a aprovação ficam no nosso app. O analista alterna entre as janelas; o app minimiza isso trazendo o documento atualizado ao vivo e um botão para voltar à conversa.

### 6.4 Cliente: Claude Code CLI (recomendado)

É a experiência completa do produto. O app usa o `claude` instalado e logado pelo próprio analista, **sem modificação**, em modo interativo, de duas formas:

- **Terminal embutido no app** (PTY), na pasta do sistema, ao lado dos painéis da análise (documento ao vivo, evidências, perguntas, sugestões), como no ctower-agents. É a forma recomendada: **conversa, documento e governança numa janela só**, sem alternar entre aplicativos. Clicar numa evidência abre o trecho ao lado da conversa; confirmar uma afirmação ou pedir aprofundamento acontece no mesmo lugar. O app inicia a sessão com o pedido e acompanha pelos hooks.
- **Terminal do próprio analista** (o que ele já usa, com seu shell, tema e atalhos): o app oferece o comando pronto ou o link `claude-cli://` com o pedido preenchido. O app não controla esse terminal, mas acompanha a sessão pelos hooks e atualiza o documento pelo MCP do mesmo jeito.

Diferenças em relação ao Desktop:

- **Um aplicativo só:** no terminal embutido, tudo acontece no nosso app; com o Desktop, são dois.
- **Sem as confirmações do link:** no terminal embutido a sessão já começa com o pedido.
- **Botões que agem na conversa:** "aprofundar esta afirmação", "resumir para o solicitante" e os demais prompts prontos são enviados direto ao terminal embutido; com o Desktop, viram um link novo com o pedido preenchido.
- **Mais provedores:** o Claude Code CLI aceita assinatura, chave de API e provedores de nuvem (Bedrock, Vertex). O app não configura nem lê essas credenciais; usa o que o analista já configurou no Claude Code. Isso abre o caminho corporativo antes do laço próprio da fase 2.
- **Mais plataformas:** funciona onde o Claude Code roda, inclusive Linux e WSL.
- **Passagem para o Desktop:** o botão "Continuar no Claude Desktop" envia `/desktop`, que move a sessão para a aba de código do Desktop com o mesmo histórico (macOS e Windows x64).

Como o terminal embutido inicia e conversa com o `claude` (`cba_session::terminal`), só com opções documentadas da linha de comando:

- O terminal só abre com o ambiente preparado (servidor MCP e hooks, seção 6.2) e com o Claude Code instalado; senão, mostra o que falta e o passo a passo oficial.
- Conversa nova com `--session-id <uuid>` gerado pelo app, que guarda a última sessão de cada sistema e, ao reabrir, oferece retomá-la com `--resume <id>` (ou `--continue`, se o identificador não for conhecido). O pedido inicial vai como prompt posicional.
- A pasta do `claude` vai na frente do `PATH` do processo, porque o app aberto pelo Finder ou pelo menu Iniciar tem `PATH` mínimo.
- Prompts prontos (e o `/desktop`) são digitados na sessão viva: o texto, uma pausa curta e o Enter. Quebras de linha vão como `Ctrl+J`, a nova linha documentada do Claude Code.
- Com o Claude Code instalado pelo npm no Windows (`claude.cmd`), o prompt inicial não pode ter caracteres que o `cmd.exe` interpretaria (aspas, `%`, `!`, quebras de linha e, numa palavra solta, `& | < > ^ ( )`); o instalador nativo não tem essa limitação.

### 6.5 Evidências

- O conteúdo de uma análise é composto de **afirmações** registradas pela ferramenta `afirmar`, não de texto livre. Cada afirmação tem: seção do documento, texto, nível de confiança e evidências.
- Uma **evidência** é `(arquivo, linha inicial, linha final, trecho citado)`, com o caminho relativo à raiz do sistema. O servidor valida no registro: o arquivo existe dentro das pastas do sistema (também depois de seguir links simbólicos) e fora das exclusões, é texto, o intervalo é válido (até 300 linhas) e existe no arquivo, e o trecho aparece dentro do intervalo. A conferência ignora os espaços nas pontas de cada linha, os espaços repetidos e as linhas em branco, e mais nada. Guarda o hash do intervalo (SHA-256 das linhas normalizadas do mesmo jeito, para mudança de indentação ou de fim de linha não marcar a evidência como a revalidar) e o commit. Evidência inválida é recusada com o motivo e, quando possível, com a correção (as linhas em que o trecho aparece ou o conteúdo atual do intervalo), para o agente corrigir. Se uma das evidências de uma afirmação for recusada, nada é registrado.
- **Níveis de confiança:** `confirmado` (evidência direta), `inferido` (dedução a partir de evidências listadas) e `a_confirmar` (sem evidência suficiente; vira pergunta).
- Se o arquivo mudar, o hash denuncia e a evidência, e as regras que dependem dela, aparecem como **a revalidar**.

### 6.6 Laço próprio (fase 2)

Para clientes que não são o Claude Desktop nem o Claude Code, o app terá um laço de agente que chama o modelo e executa as mesmas ferramentas de `cba-tools`, numa interface de chat do próprio app:

- **ChatGPT Plus/Pro** pelo programa "Sign in with ChatGPT" da OpenAI (em preview, liberado para apps open-source e locais): login OAuth pelo navegador, consumo do plano do usuário, ferramentas próprias pela Responses API.
- **Chave de API da Anthropic, Amazon Bedrock, Google Vertex AI:** uso corporativo e base para a EE.

## 7. Base de conhecimento e documentos

### 7.1 Estrutura

```
<base>/
  _templates/                        # templates dos documentos (editáveis)
    roteiros/<fluxo>.md              # roteiro de cada fluxo (editável, seção 8)
    prompts/<id>.md                  # prompts prontos (editáveis, seção 8)
  <sistema>/
    sistema.md                       # visão, áreas, estado, próximo passo
    regras-do-sistema.md             # regras do sistema para o agente (seção 8)
    glossario/<termo>.md             # negócio → código
    regras/RN-<AREA>-<NNN>.md        # uma regra por arquivo
    areas/<area>/memoria.md          # memória de uma área funcional
    funcionalidades/<area>/<tema>/   # documento negocial, casos de uso, rastreabilidade
    analises/<id>.md                 # análises de impacto, perguntas, explicações
    analises/<id>.estado.json        # estado da análise: sessões, afirmações, evidências, perguntas, sugestões
    sugestoes/<id>.md                # sugestões de alteração
```

- **Fora do repositório do sistema** e fora de qualquer pasta temporária. A pasta é do analista e sobrevive à reinstalação do app.
- **Caminhos relativos** em todos os documentos, nunca absolutos, para a base continuar válida em outra máquina ou sistema operacional.
- **Gravação contínua:** o documento de uma análise é atualizado a cada registro, não só no fim. Uma sessão interrompida não perde o que já foi apurado.
- **Estado da análise:** os registros (sessões, afirmações com evidências, perguntas, sugestões) ficam em `analises/<id>.estado.json`, gravado a cada mudança sob uma trava de arquivo, porque o app, o servidor MCP de cada sessão e o receptor de hooks gravam na mesma análise. O documento Markdown é montado a partir dele. Cada mudança é anotada no diário de eventos do app (no diretório de dados, só com identificadores), que o app acompanha para atualizar os painéis ao vivo: uma vigia por sistema consulta o tamanho do arquivo a cada 100 ms, lê só as linhas novas e relê a análise citada (menos de 1 s do registro à tela). Como o estado completo está na base, o diário não guarda histórico: depois de ler tudo, a vigia o esvazia quando passa de 1 MiB, sob a mesma trava de quem acrescenta linhas.

### 7.2 Frontmatter

Todo documento começa com frontmatter YAML, que o app usa para indexar, filtrar e validar. Exemplo de análise:

```yaml
---
tipo: analise-impacto
id: A-0193
titulo: Parcelamento da fatura para pessoa jurídica
sistema: faturamento-exemplo
demanda: {origem: texto, chave: null}
status: rascunho            # rascunho | em-revisao | aprovada
abrangencia: media
base: {branch: develop, commit: 1a2b3c4d}
criado: 2026-11-11
analista: Ana Exemplo
---
```

Exemplo de regra:

```yaml
---
tipo: regra
id: RN-FAT-007
titulo: Juros de atraso contam a partir do primeiro dia
area: cobranca
status: confirmada          # confirmada | a-revalidar | obsoleta
evidencias:
  - arquivo: db/multa.sql
    linhas: 10-10
    hash: 9f3c2a…
    commit: 1a2b3c4d
origem: analises/A-0193.md
---
```

### 7.3 Tipos de documento e padronização

O agente registra dados estruturados pelas ferramentas MCP, e o app monta cada documento a partir do template do seu tipo. O texto do agente fica só dentro dos campos.

| Tipo | Seções fixas |
|---|---|
| Análise de impacto | demanda como veio · demanda como entendida · como é hoje · funcionalidades · dados · integrações · documentos e telas · riscos · abrangência · perguntas para o solicitante · sugestões · evidências |
| Pergunta ao sistema | pergunta · resposta · regras encontradas · exemplo · perguntas em aberto · sugestões · evidências |
| Documento negocial da funcionalidade | objetivo · atores e telas · fluxo principal · fluxos alternativos · exceções · regras · dados · integrações · perguntas · sugestões · evidências |
| Caso de uso | identificação · objetivo · gatilho · pré-condições · fluxo principal · alternativos · exceções · regras · evidências |
| Regra de negócio | identificador estável · enunciado · área · evidências com hash · origem |
| Termo do glossário | termo · sinônimos · definição de negócio · onde vive no código (entidades, tabelas, telas, serviços, documentos) |
| Sugestão de alteração | contexto · trecho atual · trecho proposto · motivo · impacto · exemplo |

- **Templates editáveis:** ficam em `<base>/_templates/` como Markdown com marcadores. O app traz um conjunto padrão; cada organização pode ajustar ao seu (por exemplo, um padrão de documento negocial já adotado).
  - Um arquivo por tipo, com o nome do campo `tipo` (`_templates/analise-impacto.md`). Sem o arquivo, vale o padrão embutido; o app copia o padrão para a base sem sobrescrever o que foi editado.
  - Marcadores `{{nome}}`: de campo (preenchidos pelo frontmatter, como `{{id}}` e `{{titulo}}`) e de seção (`{{como_e_hoje}}`, `{{perguntas}}`, `{{sugestoes}}`, `{{evidencias}}`). Um comentário HTML no início do template é instrução para quem edita e não vai para o documento.
  - Marcador desconhecido gera aviso e fica no texto. Seção com registros cujo marcador foi removido vai para o fim do documento, com aviso, para nada registrado se perder.
- **Evidências no documento:** cada afirmação traz o selo de confiança e uma nota de rodapé com `arquivo:linhas`. No app, a nota abre o trecho. As notas são numeradas na ordem em que aparecem; o mesmo trecho citado de novo usa a mesma nota, e a seção de evidências lista as definições.
- **O chat não é o documento.** No chat, o agente dá um resumo e o link do documento (devolvido por `concluir_analise`). O documento completo se lê no app ou em qualquer editor de Markdown.

### 7.4 Como a base cresce

- **Análise aprovada alimenta a base:** as afirmações confirmadas pelo analista viram regras (com identificador estável) e os termos propostos viram entradas do glossário.
- **Glossário negócio → código** é a peça que mais acelera as análises seguintes: o maior custo de uma análise é localizar no código os conceitos da demanda.
- **Revalidação:** ao reabrir o sistema, o app confere os hashes das evidências contra o código atual e marca como "a revalidar" o que mudou.

### 7.5 Índice e busca

- **SQLite é só índice,** reconstruído a partir dos arquivos: busca textual (FTS5), ligações entre regras, termos e análises, estado das evidências. Apagá-lo não perde nada.
- **Sem RAG vetorial no MVP.** Para código, a busca do próprio agente (termos, leitura, referências) é mais precisa do que trechos recuperados por similaridade, e a evidência exige o trecho exato. A base em linguagem de negócio é pequena e curada, e a busca textual basta. Embeddings entram só se a prática mostrar falhas de busca, e sobre a base, não sobre o código.

## 8. Prompts

Modelo em camadas, como no ctower-agents:

| Camada | Conteúdo | Como chega ao agente |
|---|---|---|
| Limites fixos | papel (apoia analista, não desenvolve), sugestão em vez de alteração, evidência obrigatória e honesta, consultar a base antes, declarar o que está fora do código, segredos, linguagem de negócio | hook `SessionStart`, em toda sessão, inclusive na retomada e em qualquer cliente |
| Regras do sistema | convenções do sistema e da organização, editáveis e versionadas na base, em `<sistema>/regras-do-sistema.md` (criado com um padrão só de comentários, que não chega ao agente) | junto dos limites, no `SessionStart` |
| Roteiro do fluxo | um template por fluxo, com variáveis (identificador, entrada, contexto da base) | resposta de `iniciar_analise` |
| Prompts prontos | aprofundar, resumir para o solicitante, gerar critérios de aceite, conferir evidências, retomar | botões no app, que abrem o cliente com o pedido preenchido |

Roteiros e prompts prontos são editáveis na base, como os templates (7.3): `_templates/roteiros/<fluxo>.md` (`perguntar`, `explicar`, `impacto`) e `_templates/prompts/<id>.md`. Sem o arquivo, vale o padrão embutido; o app copia o padrão para a base sem sobrescrever o que foi editado (dos roteiros, só os já revisados). Variáveis são `{{nome}}`: no roteiro, `{{id}}`, `{{titulo}}`, `{{fluxo}}`, `{{documento}}`, `{{entrada}}`, `{{secoes}}`, `{{secoes_obrigatorias}}` e `{{contexto}}`; nos prompts, `{{id}}`, `{{titulo}}`, `{{fluxo}}` e `{{documento}}`. Variável desconhecida gera aviso e fica no texto; variável essencial ausente (no roteiro, entrada, seções e contexto; nos prompts, o identificador) vai para o fim, com aviso; arquivo ilegível gera aviso e usa o padrão. O prompt pronto vai ao cliente numa linha só.

O texto dos limites fica em `crates/cba-session/prompts/limites.md`, embutido no executável. O `SessionStart` responde em `hookSpecificOutput.additionalContext` com os limites, as regras do sistema (sem comentários HTML, cortadas em 5.000 caracteres) e uma nota da sessão (sistema e análise ligada), em toda origem (`startup`, `resume`, `compact`, `clear`), abaixo do limite de 10.000 caracteres do Claude Code. Fora dos sistemas cadastrados, nada é acrescentado.

Pontos validados na prova de conceito (seção 13):

- os limites injetados pelo `SessionStart` são seguidos na aba de código do Claude Desktop;
- com o prompt de limites, um pedido explícito de correção, com o analista aprovando todas as permissões, resultou em sugestão de alteração e nenhum arquivo alterado;
- sem o prompt de limites, o mesmo pedido levou o agente a editar o arquivo por comando de shell, contornando a negação das ferramentas de edição.

## 9. Provedores, credenciais e privacidade

- **Claude Desktop e Claude Code:** o analista instala e faz login pelos fluxos oficiais da Anthropic. O app não oferece login do Claude, não lê, não guarda e não intermedeia credenciais ou tokens, e não embute, instala ou modifica nenhum binário. Isso segue a página de questões legais e de conformidade do Claude Code, que proíbe terceiros de oferecer o login do Claude.ai ou intermediar credenciais, mas não impede o usuário de usar os aplicativos oficiais com a própria assinatura. Antes de distribuir, pedir confirmação por escrito à Anthropic de que o desenho das seções 6.2 a 6.4 está dentro das regras.
- **Detecção dos clientes (primeiro uso e tela inicial):** o app procura o `claude` no `PATH` e nos locais padrão de instalação e o Claude Desktop pela existência do aplicativo, consultando só metadados dos caminhos. O login do Claude Code é verificado apenas pelo comando oficial `claude auth status --json`, do qual o app usa só `loggedIn`, `authMethod` e `apiProvider` (e-mail e organização são descartados). O login do Claude Desktop não é verificado, porque não há comando de status documentado; a tela pede ao analista que confira no próprio Desktop.
- **ChatGPT (fase 2):** "Sign in with ChatGPT" está em preview e liberado para apps open-source e locais; app pago ou hospedado depende de aprovação da OpenAI. Afeta a EE, não o MVP.
- **Repositório do sistema:** o app recomenda token somente leitura e avisa quando a pasta do sistema foi alterada. A responsabilidade pelo que vai para o repositório é do analista.
- **Exclusões padrão:** dependências de terceiros, binários, arquivos com padrão de segredo. As ferramentas MCP respeitam as exclusões; as ferramentas próprias do cliente de IA não, e os limites orientam o agente a evitá-las.
- **Dados do sistema analisado** ficam na máquina do analista: na base de conhecimento e nos históricos do próprio cliente de IA. O app não envia nada a serviços próprios.
- **Sem telemetria** no MVP.

## 10. Interface

- **Início:** sistemas cadastrados, análises recentes e em andamento (com estado vindo dos hooks), botões dos fluxos, estado dos clientes de IA (instalado, logado).
- **Sistema:** mapa, glossário, regras (com as que estão "a revalidar"), áreas.
- **Análise:** criada pelo app a partir da pergunta (o terminal abre com o pedido de `iniciar_analise` com o identificador, para a sessão ficar ligada à análise desde a primeira chamada); documento sendo montado ao vivo, afirmações com selo de confiança, evidência selecionada com o trecho destacado, perguntas, sugestões, aviso de alteração no repositório. Terminal embutido com o Claude Code, quando esse for o cliente escolhido. Botões: abrir no cliente escolhido (Desktop, terminal embutido, comando para o terminal do analista), retomar, continuar no Desktop, confirmar, marcar incorreta, aprovar, exportar.
- **Base de conhecimento:** busca em regras, termos e análises.
- **Configurações:** cliente de IA preferido (padrão: Claude Code no terminal embutido), pasta da base, sistemas e exclusões, templates, regras do sistema, ambiente do Claude (registro do MCP e dos hooks), proteção opcional contra edição.

Toda a interface em português no MVP, com textos preparados para tradução.

## 11. Modelo de dados do índice

SQLite local, reconstruível a partir da base, identificadores UUID v7 internos e identificadores legíveis nos documentos (`A-0193`, `RN-FAT-007`).

| Entidade | Conteúdo principal |
|---|---|
| `sistema` | nome, pastas, exclusões, chave do Claude Code para o registro do MCP |
| `arquivo`, `simbolo`, `referencia` | mapa estrutural, por hash |
| `documento` | caminho na base, tipo, frontmatter, hash do arquivo |
| `analise` | fluxo, entrada, estado, documento |
| `sessao` | análise, cliente, identificador da sessão do cliente, estado, início e fim |
| `afirmacao`, `evidencia` | seção, texto, confiança; arquivo, intervalo, trecho, hash, commit |
| `pergunta`, `sugestao` | registros da análise |
| `regra`, `termo` | espelho indexado dos documentos da base |
| `evento` | registro de mudanças (quem, quando, o quê, por qual cliente) |

## 12. Distribuição e licença

- **Instaladores assinados:** MSI ou NSIS por usuário no Windows (sem administrador) e DMG notarizado no macOS. Atualização automática pelo atualizador do Tauri, desligável.
- **Licença:** Apache-2.0 neste repositório. A EE, se existir, fica em repositório separado e fechado.
- **Contribuições externas:** exigem aceite de um CLA antes da primeira contribuição, para preservar a possibilidade da EE.
- **Nome e marca:** o produto pode dizer em texto que funciona com o Claude, mas não usa nomes ou logotipos da Anthropic ou da OpenAI no nome ou na identidade visual.
- **Branches:** `develop` para integração e `main` para releases, como no ctower-agents.

## 13. Prova de conceito (2026-11-11)

Feita com um servidor MCP mínimo, hooks e o Claude Desktop numa máquina macOS, primeiro com o sistema fictício e depois com um sistema real de grande porte (cujos dados não são registrados aqui).

| Verificação | Resultado |
|---|---|
| Link `claude://code/new` com pasta e pedido | abre a sessão; o analista confirma a pasta e envia |
| MCP com escopo local visto pela aba de código do Desktop | sim, sem arquivo no repositório |
| Limites pelo hook `SessionStart` no Desktop | seguidos |
| Roteiro vindo de `iniciar_analise` | seguido, inclusive "separar fato de suposição do solicitante" |
| Afirmações com evidência validada | sim; evidência recusada pelo servidor foi corrigida pelo agente na hora |
| Hooks no Desktop | início, cada ferramenta, aguardando permissão e fim, com o identificador da sessão |
| Sessões do CLI e do Desktop | no mesmo histórico local; `claude --desktop --resume <id>` abre no Desktop se a sessão estiver fechada no outro lado e houver terminal (PTY) |
| Pedido de correção, analista aprovando tudo, sem prompt de limites | arquivo alterado por comando de shell |
| O mesmo, com prompt de limites | nenhum arquivo alterado; sugestão de alteração registrada com exemplo numérico |
| Análise de impacto num sistema real | cerca de 6 minutos, 35 afirmações (todas com evidência válida), 8 perguntas pertinentes; encontrou a causa técnica que o solicitante só intuía e corrigiu a estimativa de abrangência da demanda |

Ajustes que vieram da prova de conceito e já estão nesta spec: campo `secao` em `afirmar`; `concluir_analise` e documento fora do chat; gravação contínua do documento na base; escopo local do MCP em worktrees; base de conhecimento alimentada pela análise aprovada.

## 14. Riscos e questões em aberto

**Riscos**

- **Resposta errada com cara de autoridade.** Mitigação: afirmações estruturadas, evidência validada, confiança visível, revisão antes de aprovar.
- **Mudança nas regras dos provedores** sobre o uso de assinaturas com apps de terceiros. Mitigação: o app nunca toca em credenciais e só abre os aplicativos oficiais; confirmação por escrito antes de distribuir; laço próprio na fase 2 como alternativa.
- **Mudança no Claude Desktop ou no Claude Code** (formato dos links, configuração compartilhada, hooks). Mitigação: dependência só de recursos documentados; dois clientes, um como alternativa ao outro; testes de fumaça a cada versão nova.
- **Alteração indevida do repositório.** Mitigação: prompt de limites (validado), aviso de alteração, proteção técnica opcional. Responsabilidade final do analista.
- **Base desatualizada.** Mitigação: hash e commit em toda evidência, revalidação ao reabrir o sistema.
- **Limites do plano pessoal** em sistemas grandes. Mitigação: base e glossário reduzem a exploração repetida; mapeamento estrutural sem IA.
- **Regra de negócio fora do código analisável:** parâmetros em tabela, jobs agendados, sistemas vizinhos. Mitigação: o agente declara o limite como pergunta.

**Questões em aberto**

1. Extensões e servidores MCP locais no chat comum do Claude Desktop (fora da aba de código) estão disponíveis em todos os planos pessoais? Só importa se quisermos oferecer também o chat comum.
2. `claude --desktop --resume` funciona para sessões criadas no próprio Desktop, depois de fechadas lá? A prova de conceito só confirmou para sessões criadas no CLI.
3. Formato da base: uma regra por arquivo escala bem para centenas de regras, ou convém um catálogo por área com entradas?
4. Quais linguagens legadas priorizar nos extratores? Depende do primeiro piloto.
5. Como importar bases de conhecimento já existentes, e com que regras de revalidação?

## 15. Critérios de aceite do MVP

1. Instalação no Windows sem permissão de administrador e no macOS, com o primeiro uso concluído dentro do critério de sucesso 1, verificada com pelo menos três analistas de fora do time.
2. Em um sistema de teste com regras conhecidas (fixture do repositório), os três fluxos encontram as regras esperadas, e nenhuma afirmação marcada como `confirmado` fica sem evidência válida.
3. Todo documento gerado segue o template do seu tipo, tem frontmatter válido e caminhos relativos, e é gravado na base durante a análise.
4. Uma análise interrompida no meio é retomada numa sessão nova com todas as afirmações, evidências, perguntas e sugestões.
5. Os três fluxos funcionam de ponta a ponta pelos dois clientes (Claude Desktop e Claude Code CLI, no terminal embutido e no terminal do analista), com o mesmo documento final, e uma análise iniciada num cliente pode continuar no outro.
6. Descadastrar um sistema remove o registro do MCP e dos hooks da configuração do Claude, preservando o restante dela.
7. O app não lê nem grava credenciais do Claude (verificado por revisão de código e teste).
8. Reabrir um sistema após alterar arquivos marca como "a revalidar" as evidências e regras afetadas.
9. Exportação em Markdown e Word preserva seções, confiança e evidências.
