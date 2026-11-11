# ctower-business-agents: especificação do MVP standalone

- **Data:** 2026-11-11
- **Status:** proposta, aguardando revisão
- **Repositório:** `deep-ion-ai/ctower-business-agents`

## 1. Contexto e objetivo

Analistas de requisitos e de negócio que atuam em sistemas legados trabalham sem acesso prático à única documentação confiável desses sistemas: o código-fonte e o banco de dados. Documentos de especificação estão desatualizados ou não existem, e as pessoas que conheciam as regras saíram.

Hoje o uso de IA por esse público costuma ser um chat genérico: cola-se um trecho de documento ou de e-mail e recebe-se uma resposta convincente, sem nenhuma relação verificável com o que o sistema faz de fato. Não há gestão do que foi perguntado, nem rastreabilidade, nem reaproveitamento do que foi descoberto.

**Objetivo:** um aplicativo desktop, instalado e configurado pelo próprio analista, que conduz análises sobre um sistema legado (perguntas, regras de negócio de uma funcionalidade, impacto de uma demanda) **a partir do código**, com **evidência citada para cada afirmação**, e que guarda o que foi descoberto numa base de conhecimento local.

O projeto nasce da experiência com o [ctower-agents](https://github.com/deep-ion-ai/ctower-agents) (orquestração de agentes de código), mas é um produto separado, para outro público, e começa deliberadamente simples.

### Critérios de sucesso

1. Um analista sem experiência com terminal, que já tem uma assinatura de IA pessoal, instala o app, cadastra o sistema e recebe a primeira resposta com evidências em menos de 15 minutos, sem ajuda de alguém de TI.
2. Toda afirmação de uma análise aponta para pelo menos um trecho de código, schema ou documento que a sustenta, ou está marcada explicitamente como "a confirmar".
3. Uma análise de impacto de uma demanda típica sai como documento revisável e exportável (Markdown e Word) sem que o analista escreva prompt.
4. Uma análise iniciada no app pode continuar no Claude Desktop e volta ao app para revisão, sem perder afirmações nem evidências.

### Princípios

- **Simples antes de completo.** Um usuário, uma máquina, sem servidor nosso, sem conta nossa. Cada passo de instalação ou configuração a mais é um ponto de abandono.
- **Usa a assinatura que o analista já tem.** A maioria desse público usa contas pessoais de IA; cobrar por API inviabiliza a adoção. Chave de API e provedores de nuvem ficam para o uso corporativo.
- **Fluxos guiados, não prompt livre.** O analista escolhe o que quer fazer ("analisar impacto", "explicar funcionalidade"); o roteiro é do produto.
- **Sem evidência, não é fato.** O agente distingue o que viu no código do que está inferindo, e a interface deixa essa diferença visível.
- **O produto analisa, não desenvolve.** O agente é orientado a não alterar código nem publicar nada. A garantia contra push é o token de acesso ao repositório configurado pelo analista (somente leitura recomendado); configurá-lo com escrita é responsabilidade dele.
- **O estado é do app, não da conversa.** Análises, afirmações, evidências e base de conhecimento ficam no app. A conversa pode acontecer em mais de um cliente de IA sem perder nada.
- **O analista assina o resultado.** O agente produz rascunhos; o documento final é revisado e aprovado por uma pessoa.

## 2. Público

- **Analista de requisitos / analista de sistemas:** recebe demandas de mudança em sistemas legados e precisa descrever o que muda, levantar regras existentes e escrever histórias com critérios de aceite. Lê pouco ou nenhum código.
- **Analista de negócio / product owner:** precisa entender como o sistema trata um caso de negócio hoje ("o que acontece com um contrato cancelado no meio do ciclo de faturamento?").
- **Usuário secundário:** desenvolvedor que valida uma análise e confere as evidências.

Ambiente típico: notebook pessoal ou corporativo (Windows ou macOS), assinatura pessoal de IA (Claude Pro/Max ou ChatGPT Plus/Pro), acesso de leitura ao repositório do sistema.

## 3. Escopo

### MVP

- App desktop (Windows e macOS), instalação por usuário, sem permissão de administrador.
- Cadastro de **sistemas**: um sistema é uma ou mais pastas locais com código-fonte, scripts de banco e documentos. O analista aponta para pastas que já existem na máquina.
- **Mapeamento estrutural** do sistema (inventário, linguagens, rotinas, tabelas, referências), sem IA, atualizado de forma incremental.
- **Servidor MCP local** do app, com as ferramentas de análise, a validação de evidências e o acesso à base de conhecimento.
- **Cliente principal: Claude Code** instalado e logado pelo próprio analista, aberto pelo app num terminal embutido, já com o servidor MCP e o roteiro do fluxo.
- **Continuação no Claude Desktop** (seção 6.4).
- Três fluxos guiados:
  1. **Perguntar ao sistema:** pergunta livre, resposta com evidências.
  2. **Explicar funcionalidade:** regras de negócio, entradas, saídas, validações e casos de exceção de uma funcionalidade.
  3. **Analisar impacto de uma demanda:** a partir do texto da demanda (colado), o que muda, onde, quais regras existentes são afetadas, riscos e perguntas em aberto.
- **Painéis ao vivo** no app: afirmações, evidências e documento sendo montados enquanto a conversa acontece.
- **Base de conhecimento local:** regras de negócio e termos do glossário confirmados pelo analista, usados nas análises seguintes.
- **Documento da análise:** editável no app, com evidências navegáveis, revisão e aprovação, exportação para Markdown e Word.
- **Aviso de token com escrita:** se o token do repositório informado tiver permissão de escrita, o app avisa uma vez. Não bloqueia.

### Fase 2 (ainda standalone)

- **Cliente ChatGPT:** laço de agente próprio no app com "Entrar com ChatGPT" (Plus/Pro), usando as mesmas ferramentas (seção 6.5).
- **Clientes corporativos:** o mesmo laço próprio com chave de API da Anthropic, Amazon Bedrock ou Google Vertex AI.
- **Resumos por IA** no mapeamento (por arquivo e por módulo), gerados sob demanda pelo cliente em uso.
- Fluxos **revisar história** e **gerar cenários de teste**.
- Integração com Jira (ler a demanda pela chave e publicar a análise como comentário ou anexo).
- Clone de repositório Git pelo app.
- Conexão a um banco de dados para extrair o schema, além de arquivos DDL.
- Exportação para Confluence.

### Edição comercial (EE), futura e condicionada à adoção

Fora deste repositório. Só registrada aqui para orientar decisões do núcleo:

- Servidor compartilhado, multiusuário, com a mesma lógica do núcleo.
- Base de conhecimento compartilhada por equipe e por sistema.
- Controle de acesso, trilha de auditoria centralizada, políticas de uso.
- Uso de assinatura pessoal em produto pago exige acordo com o provedor (seção 9); a EE deve assumir chave de API ou provedor de nuvem como padrão.

### Fora de escopo

- Desenvolvimento: gerar ou alterar código, abrir PR, fazer push.
- Execução do sistema analisado ou de testes.
- Uso multiusuário ou sincronização entre máquinas.
- Embutir, instalar ou modificar o binário do Claude Code, ou intermediar o login do analista em qualquer provedor (seção 9).

## 4. Fluxos

Os exemplos usam um sistema fictício de faturamento ("Faturamento Exemplo").

### 4.1 Primeiro uso

1. O analista instala o app e o abre.
2. O app verifica se o Claude Code está instalado e logado. Se não estiver, mostra o passo a passo oficial de instalação (botão de copiar o comando e link para a página oficial) e pede que o analista faça o login pelo próprio Claude Code. O app não instala, não embute e não lê credenciais do Claude Code.
3. O analista cadastra o sistema: nome e uma ou mais pastas. O app mostra o que vai analisar (contagem de arquivos por tipo) e o que vai ignorar (binários, dependências, arquivos com segredos), e permite ajustar.
4. O app registra o seu servidor MCP no Claude Code com escopo local daquela pasta (`claude mcp add --scope local`), com consentimento, para que o CLI e a aba de código do Claude Desktop o enxerguem. Nenhum arquivo é criado dentro do repositório do sistema.
5. Mapeamento estrutural roda com barra de progresso (sem IA, sem custo).
6. Ao final, mostra o mapa do sistema e sugere uma primeira pergunta.

### 4.2 Perguntar ao sistema

Exemplo: "Como é calculada a multa por atraso de pagamento?"

1. O analista escolhe "Perguntar ao sistema" e digita a pergunta no app.
2. O app abre o Claude Code no terminal embutido, na pasta do sistema, com o roteiro do fluxo e a pergunta já enviados.
3. O agente consulta o mapa e a base de conhecimento pelas ferramentas MCP, lê os trechos relevantes e registra cada afirmação com `afirmar`.
4. Ao lado do terminal, o painel da análise mostra as afirmações chegando, cada uma com suas evidências. Clicar abre o trecho no visualizador do app.
5. O analista pode continuar a conversa no terminal ou marcar afirmações como **confirmadas** (vão para a base de conhecimento), **incorretas** (com comentário) ou pedir aprofundamento (botão que envia a instrução ao terminal).

### 4.3 Explicar funcionalidade

O analista escolhe a funcionalidade no mapa ou a descreve. O resultado é um documento com seções fixas: objetivo, atores e telas, entradas, regras de negócio (numeradas, cada uma com evidência), validações, mensagens ao usuário, efeitos em dados (tabelas lidas e gravadas), integrações, casos de exceção e perguntas em aberto.

### 4.4 Analisar impacto de uma demanda

Exemplo de demanda: "Permitir parcelamento da fatura em até 3 vezes para clientes pessoa jurídica."

1. O analista cola o texto da demanda (e, se quiser, anexa um documento).
2. O agente identifica os conceitos da demanda, localiza no sistema onde eles vivem e levanta as regras existentes relacionadas.
3. Resultado em seções fixas: resumo da demanda como entendida, funcionalidades afetadas, regras existentes afetadas (com evidência), dados afetados, integrações afetadas, conflitos e riscos, perguntas para o solicitante, estimativa qualitativa de abrangência (pequena, média, grande, com justificativa).
4. O analista revisa, edita, aprova e exporta.

### 4.5 Continuar no Claude Desktop

Para o analista que prefere não usar o terminal, cada análise tem o botão **"Continuar no Claude Desktop"** (detalhes na seção 6.4). A análise continua lá e as afirmações registradas voltam ao app.

## 5. Arquitetura

```
┌──────────────────────────────── App desktop (Tauri) ────────────────────────────────┐
│  UI (web): fluxos, mapa, painéis da análise, documento, base de conhecimento        │
│     │ comandos Tauri                                                                │
│  cba-app (casca: janelas, terminal embutido, diálogos, cofre de credenciais)        │
│     │                                                                               │
│  cba-core ── domínio, casos de uso, persistência (SQLite), eventos                  │
│     ├── cba-tools ── ferramentas de análise e validação de evidências               │
│     ├── cba-mcp ──── servidor MCP local (expõe cba-tools)                           │
│     ├── cba-index ── inventário, extratores, mapa do sistema, busca                 │
│     ├── cba-session ─ abre e acompanha o Claude Code (PTY), handoff para o Desktop  │
│     └── cba-export ─ Markdown e Word                                                │
└─────────────────────────────────────────────────────────────────────────────────────┘
        │ MCP (stdio)                    │ MCP (stdio)                 │ fase 2
  Claude Code (CLI do analista)   Claude Desktop (do analista)   laço próprio (ChatGPT, API)
        │ leitura                        │
  pastas do sistema analisado ───────────┘
```

- **O servidor MCP é o centro do produto.** As ferramentas, a validação de evidências e a base de conhecimento vivem nele. Cada cliente de IA é só uma porta de entrada, e todos escrevem na mesma análise.
- **Um app só.** O servidor MCP é o próprio executável do app iniciado em modo `mcp` pelo cliente de IA; os dois processos compartilham o mesmo banco SQLite (modo WAL) e o app é notificado das mudanças para atualizar os painéis.
- **O núcleo não depende do Tauri.** `cba-core` e as crates abaixo dele não conhecem a UI, o que permite testes sem interface e, no futuro, um servidor EE com o mesmo núcleo.
- **Linguagem:** Rust no backend e a mesma pilha web de UI do ctower-agents.
- **Reaproveitamento do ctower-agents:** sem dependência entre os repositórios. Trechos úteis (PTY, hooks e retomada de sessão do Claude Code, execução de processo, segredos) são copiados e adaptados, com comentário de origem no topo do arquivo.

## 6. Agente e clientes

### 6.1 Ferramentas MCP

Operam só dentro das pastas cadastradas do sistema, respeitam a lista de exclusões e têm limites de tamanho. O Claude Code também usa as suas ferramentas próprias de leitura (`Read`, `Grep`, `Glob`); as nossas acrescentam o que ele não tem.

| Ferramenta | O que faz |
|---|---|
| `mapa` | Retorna o mapa do sistema ou de um módulo. |
| `estrutura` | Lista rotinas, procedures, classes e telas de um arquivo (extratores da seção 7). |
| `schema` | Consulta tabelas, colunas e relacionamentos extraídos dos scripts de banco. |
| `referencias` | Onde um identificador (rotina, tabela, campo) é usado. |
| `conhecimento` | Consulta a base de conhecimento local (regras confirmadas, glossário). |
| `iniciar_analise` | Cria uma análise a partir de um fluxo e de uma entrada; devolve o roteiro do fluxo. |
| `retomar_analise` | Devolve o estado de uma análise: entrada, afirmações, evidências, perguntas em aberto. |
| `afirmar` | Registra uma afirmação com suas evidências (ver 6.2). |
| `perguntar` | Registra uma pergunta em aberto para o solicitante. |

Os fluxos guiados são expostos como ferramentas (`iniciar_analise`), e não só como prompts MCP, porque o suporte a prompts de servidores locais no Claude Desktop não está confirmado.

### 6.2 Evidências

- O resultado de uma análise é composto de **afirmações** registradas pela ferramenta `afirmar`, não de texto livre. Cada afirmação tem: texto, seção do documento, nível de confiança e lista de evidências.
- Uma **evidência** é `(arquivo, linha inicial, linha final, trecho citado)`. O servidor valida no registro: o arquivo existe dentro das pastas do sistema, o intervalo é válido e o trecho citado confere com o conteúdo atual. Guarda o hash do trecho. Evidência inválida é recusada com o motivo, para o agente corrigir.
- **Níveis de confiança:**
  - **confirmado no código:** evidência direta;
  - **inferido:** dedução a partir de evidências, que ficam listadas;
  - **a confirmar:** sem evidência suficiente; vira pergunta em aberto no documento.
- Se o arquivo mudar depois da análise, o hash denuncia e a evidência aparece como **desatualizada**.

### 6.3 Cliente principal: Claude Code no terminal embutido

- O app abre o `claude` instalado pelo analista, **sem modificação**, num terminal embutido (PTY), na pasta do sistema, em modo interativo. O analista vê e usa o Claude Code como ele é.
- Parâmetros passados pelo app: o roteiro do fluxo (`--append-system-prompt`), as ferramentas permitidas e o primeiro pedido.
- **Orientação contra desenvolvimento:** o roteiro instrui o agente a não editar arquivos, não executar comandos de build, commit ou push e, se o analista pedir uma alteração, registrá-la como sugestão na análise. As ferramentas de escrita do Claude Code (`Edit`, `Write`) ficam fora da lista permitida; `Bash` fica sujeito à confirmação padrão do Claude Code. Não há garantia técnica adicional (seção 1, princípios).
- Hooks do Claude Code (como no ctower-agents) informam ao app o estado da sessão e o identificador da sessão, para retomada.
- O consumo é o da assinatura do analista, dentro dos limites do plano dele.

### 6.4 Continuação no Claude Desktop

Dois caminhos, nesta ordem de preferência:

1. **Mover a sessão do Claude Code para o Desktop:** o botão "Continuar no Claude Desktop" envia `/desktop` ao terminal embutido. O Claude Code salva a sessão, abre-a na aba de código do Claude Desktop e encerra o CLI. A conversa continua com o mesmo histórico, e o servidor MCP registrado com escopo local da pasta continua disponível. Documentado para macOS e Windows x64.
2. **Abrir um chat no Claude Desktop:** para quem prefere o chat comum, o app abre `claude://claude.ai/new?q=<instrução>` com a instrução de continuar a análise ("Continue a análise 0193… do Faturamento Exemplo"). O texto fica preenchido para o analista revisar e enviar. Requer o servidor MCP instalado no Claude Desktop como extensão (`.mcpb`), que o app gera e o analista instala com um clique; o agente chama `retomar_analise` e segue.

Em ambos os casos a análise continua sendo a mesma no app: as afirmações registradas no Desktop aparecem nos painéis e no documento. O caminho inverso (Desktop para o app) não depende de handoff de sessão: o analista volta ao app, e uma nova sessão no terminal embutido chama `retomar_analise`.

### 6.5 Laço próprio (fase 2)

Para clientes que não são o Claude Code nem o Claude Desktop, o app tem um laço de agente próprio que chama o modelo e executa as mesmas ferramentas de `cba-tools`, numa interface de chat do próprio app:

- **ChatGPT Plus/Pro** pelo programa "Sign in with ChatGPT" da OpenAI (em preview, liberado para apps open-source e locais): login OAuth pelo navegador, consumo do plano do usuário, chamadas à Responses API com ferramentas próprias.
- **Chave de API da Anthropic, Amazon Bedrock, Google Vertex AI:** uso corporativo e base para a EE.

## 7. Mapeamento do sistema

Objetivo: dar ao agente um ponto de partida barato, para que cada pergunta não comece explorando o sistema do zero.

1. **Inventário:** arquivos, tamanho, linguagem (por extensão e conteúdo), exclusões aplicadas.
2. **Extração estrutural** (sem IA, por extratores por linguagem): rotinas, procedures, classes, telas, tabelas e colunas (DDL), chamadas e referências entre elas, quando o extrator consegue identificar.
3. **Resumos** (fase 2): por arquivo e por módulo, em linguagem de negócio, gerados sob demanda pelo cliente de IA em uso e guardados no app.
4. **Glossário candidato:** termos de negócio recorrentes, para o analista confirmar. Nasce das análises (o agente propõe termos) e não de uma passada própria.

Tudo é guardado por hash de arquivo. Na reabertura, só o que mudou é reprocessado.

### Extratores no MVP

Extrator genérico (texto, identificadores, comentários) para qualquer linguagem, mais extratores específicos para: SQL e PL/SQL (DDL, procedures, triggers), COBOL (divisões, parágrafos, copybooks, `CALL`), Delphi/Pascal (units, forms `.dfm`), Visual Basic 6 (módulos, forms), Java e C#. A lista final depende dos sistemas usados no piloto (ver seção 13).

## 8. Modelo de dados

SQLite local, um arquivo por instalação, identificadores UUID v7. Mudanças relevantes geram um registro em `evento` (quem, quando, o quê, por qual cliente), pensando em auditoria e numa futura sincronização com a EE.

| Entidade | Conteúdo principal |
|---|---|
| `sistema` | nome, pastas, exclusões |
| `arquivo` | caminho, hash, linguagem, resumo |
| `simbolo` | rotina, procedure, tabela, tela, com arquivo e intervalo |
| `referencia` | símbolo de origem → símbolo de destino |
| `analise` | fluxo, entrada (pergunta ou demanda), estado (rascunho, em revisão, aprovada) |
| `sessao` | análise, cliente (Claude Code, Claude Desktop, laço próprio), identificador da sessão do cliente, início e fim |
| `afirmacao` | análise, sessão, seção, texto, confiança, estado da revisão |
| `evidencia` | afirmação, arquivo, intervalo, trecho, hash |
| `pergunta` | análise, texto, estado |
| `regra` | regra de negócio confirmada, com evidências e análise de origem |
| `termo` | termo do glossário, definição, estado |
| `envio` | sessão, ferramenta, horário, conteúdo devolvido ao cliente de IA |
| `evento` | registro de mudanças |

## 9. Provedores, credenciais e privacidade

- **Claude Code e Claude Desktop:** o analista instala e faz login pelos fluxos oficiais da Anthropic. O app não oferece login do Claude, não lê, não guarda e não intermedeia credenciais ou tokens, não embute nem instala o binário e não o modifica. Isso segue a página de questões legais e de conformidade do Claude Code, que proíbe terceiros de oferecer o login do Claude.ai ou intermediar credenciais, mas não impede o usuário de usar o binário original com a própria assinatura. Antes de distribuir, pedir confirmação por escrito à Anthropic de que o desenho da seção 6.3 está dentro das regras.
- **ChatGPT (fase 2):** "Sign in with ChatGPT" está em preview e liberado para apps open-source e locais; app pago ou hospedado depende de aprovação da OpenAI. Afeta a EE, não o MVP.
- **Token do repositório:** o app recomenda um token somente leitura e avisa quando o token informado tiver escrita. O token fica no cofre do sistema operacional.
- **Exclusões padrão:** dependências de terceiros, binários, arquivos com padrão de segredo (`.env`, chaves, `*.pfx`, configuração com senha detectada). O analista vê e ajusta a lista. As ferramentas MCP respeitam as exclusões; as ferramentas próprias do Claude Code não, e o roteiro orienta o agente a evitá-las.
- **Registro de envios:** o servidor MCP registra tudo o que as nossas ferramentas devolveram ao cliente de IA. Não cobre o que o Claude Code leu com as ferramentas próprias dele; os transcripts do Claude Code ficam na máquina do analista.
- **Sem telemetria** no MVP.

## 10. Interface

- **Início:** sistemas cadastrados, análises recentes, botões dos fluxos, estado do Claude Code (instalado, logado).
- **Sistema:** mapa (módulos, funcionalidades, tabelas), estado do mapeamento, glossário.
- **Análise:** terminal embutido com o Claude Code de um lado; do outro, painéis com as afirmações chegando (selo de confiança), a evidência selecionada (trecho com destaque) e as perguntas em aberto. Botões: confirmar, marcar incorreta, aprofundar, continuar no Claude Desktop.
- **Documento da análise:** texto por seções, revisão, aprovação, exportação.
- **Base de conhecimento:** regras confirmadas e glossário, com busca e a análise de origem.
- **Configurações:** sistemas e exclusões, token do repositório, extensão do Claude Desktop, registro de envios.

Toda a interface em português no MVP, com textos preparados para tradução.

## 11. Exportação

- **Markdown:** documento com as evidências como notas de rodapé (`arquivo:linhas`).
- **Word (.docx):** mesmo conteúdo, com estilos simples e apêndice de evidências com os trechos.
- O selo de confiança de cada afirmação aparece nos dois formatos.

## 12. Distribuição e licença

- **Instaladores assinados:** MSI ou NSIS por usuário no Windows (sem administrador) e DMG notarizado no macOS. Atualização automática pelo atualizador do Tauri, desligável.
- **Licença:** Apache-2.0 neste repositório. A EE, se existir, fica em repositório separado e fechado.
- **Contribuições externas:** exigem aceite de um CLA antes da primeira contribuição, para preservar a possibilidade da EE. Definir o texto do CLA antes de abrir o repositório para contribuições.
- **Nome e marca:** o produto pode dizer em texto que usa o Claude Code, mas não usa nomes ou logotipos da Anthropic ou da OpenAI no nome ou na identidade visual.
- **Branches:** `develop` para integração e `main` para releases, como no ctower-agents.

## 13. Riscos e questões em aberto

**Riscos**

- **Resposta errada com cara de autoridade.** O público não consegue auditar o código. Mitigação: afirmações estruturadas, evidência validada, nível de confiança visível, revisão obrigatória antes de aprovar.
- **Mudança nas regras dos provedores** sobre o uso de assinaturas por apps de terceiros. Mitigação: o app nunca toca em credenciais; confirmação por escrito antes de distribuir; laço próprio com API como alternativa.
- **Instalação do Claude Code** ainda passa por um comando no terminal. Mitigação: passo a passo guiado com um único comando para copiar; o critério de sucesso 1 mede se é aceitável.
- **O agente sair desenvolvendo.** Mitigação: roteiro, ferramentas de escrita fora da lista permitida, token somente leitura recomendado e aviso de token com escrita. Responsabilidade final do analista.
- **Limites do plano pessoal** em sistemas grandes. Mitigação: mapeamento estrutural sem IA e ferramentas que devolvem pouco e preciso.
- **Regra de negócio fora do código analisável:** parâmetros em tabela no banco de produção, jobs agendados, sistemas vizinhos. Mitigação: o agente declara o limite como pergunta em aberto.
- **Adoção.** O concorrente é o chat genérico, com atrito zero. O primeiro uso precisa ser rápido (critério de sucesso 1).

**Questões em aberto**

1. Extensões locais (`.mcpb`) e servidores MCP locais estão disponíveis em todos os planos pessoais do Claude Desktop? Confirmar na documentação oficial.
2. O servidor MCP registrado com escopo local no Claude Code é visto pela aba de código do Claude Desktop depois do `/desktop`? Verificar na prática.
3. Sessões da aba de código do Claude Desktop podem voltar ao CLI? A documentação descreve só o caminho do CLI para o Desktop.
4. Quais linguagens legadas priorizar nos extratores? Depende do primeiro piloto.
5. Conexão a banco de dados (somente schema) entra no MVP ou fica para a fase 2?
6. Precisamos de um modo "pacote de código", em que um desenvolvedor prepara a pasta do sistema para um analista sem acesso ao repositório?

## 14. Critérios de aceite do MVP

1. Instalação no Windows sem permissão de administrador e no macOS, com o primeiro uso concluído dentro do critério de sucesso 1, verificada com pelo menos três analistas de fora do time.
2. Em um sistema de teste com regras conhecidas (fixture do repositório), os três fluxos encontram as regras esperadas, e nenhuma afirmação marcada como "confirmado no código" fica sem evidência válida.
3. Uma análise iniciada no terminal embutido e continuada no Claude Desktop, pelos dois caminhos da seção 6.4, termina no app com todas as afirmações e evidências das duas partes.
4. O app não lê nem grava credenciais do Claude Code ou do Claude Desktop (verificado por revisão de código e por teste que confirma que nenhum arquivo de credencial é aberto).
5. Reabrir um sistema após alterar arquivos reprocessa só os arquivos alterados, e as evidências afetadas aparecem como desatualizadas.
6. Exportação em Markdown e Word preserva seções, confiança e evidências.
