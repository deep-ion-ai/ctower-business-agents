# ctower-business-agents: especificação do MVP standalone

- **Data:** 2026-11-11
- **Status:** proposta, aguardando revisão
- **Repositório:** `deep-ion-ai/ctower-business-agents`

## 1. Contexto e objetivo

Analistas de requisitos e de negócio que atuam em sistemas legados trabalham sem acesso prático à única documentação confiável desses sistemas: o código-fonte e o banco de dados. Documentos de especificação estão desatualizados ou não existem, e as pessoas que conheciam as regras saíram.

Hoje o uso de IA por esse público costuma ser um chat genérico: cola-se um trecho de documento ou de e-mail e recebe-se uma resposta convincente, sem nenhuma relação verificável com o que o sistema faz de fato. Não há gestão do que foi perguntado, nem rastreabilidade, nem reaproveitamento do que foi descoberto.

**Objetivo:** um aplicativo desktop, instalado e configurado pelo próprio analista, que responde perguntas sobre um sistema legado e produz análises (impacto de uma demanda, regras de negócio de uma funcionalidade, revisão de uma história) **a partir do código**, com **evidência citada para cada afirmação**, e que guarda o que foi descoberto numa base de conhecimento local.

O projeto nasce da experiência com o [ctower-agents](https://github.com/deep-ion-ai/ctower-agents) (orquestração de agentes de código), mas é um produto separado, para outro público, e começa deliberadamente simples.

### Critérios de sucesso

1. Um analista sem experiência com terminal instala o app, aponta para a pasta do sistema, informa a credencial da IA e recebe a primeira resposta com evidências em menos de 15 minutos, sem ajuda de alguém de TI (exceto para obter a credencial).
2. Toda afirmação de uma análise aponta para pelo menos um trecho de código, schema ou documento que a sustenta, ou está marcada explicitamente como "a confirmar".
3. Uma análise de impacto de uma demanda típica sai como documento revisável e exportável (Markdown e Word) sem que o analista escreva prompt.
4. O app nunca altera nenhum arquivo do sistema analisado.

### Princípios

- **Simples antes de completo.** Um usuário, uma máquina, sem servidor, sem daemon, sem conta. Cada passo de instalação ou configuração a mais é um ponto de abandono.
- **Fluxos guiados, não prompt livre.** O analista escolhe o que quer fazer ("analisar impacto", "explicar funcionalidade"); o prompt é do produto. O chat livre existe, mas é complemento.
- **Sem evidência, não é fato.** O agente distingue o que viu no código do que está inferindo. A interface deixa essa diferença visível.
- **Somente leitura por construção.** O agente não tem nenhuma ferramenta capaz de escrever, executar ou acessar a rede fora do provedor de IA.
- **Transparência sobre o que sai da máquina.** O analista e a área de segurança conseguem ver exatamente quais trechos foram enviados à IA.
- **O analista assina o resultado.** O agente produz rascunhos; o documento final é revisado e aprovado por uma pessoa.

## 2. Público

- **Analista de requisitos / analista de sistemas:** recebe demandas de mudança em sistemas legados e precisa descrever o que muda, levantar regras existentes e escrever histórias com critérios de aceite. Lê pouco ou nenhum código.
- **Analista de negócio / product owner:** precisa entender como o sistema trata um caso de negócio hoje ("o que acontece com um contrato cancelado no meio do ciclo de faturamento?").
- **Usuário secundário:** desenvolvedor que valida uma análise e confere as evidências.

Ambiente típico: Windows corporativo, sem permissão de administrador, proxy de rede, política restritiva sobre o envio de código a serviços externos.

## 3. Escopo

### MVP

- App desktop (Windows e macOS), instalação por usuário, sem permissão de administrador.
- Cadastro de **sistemas**: um sistema é uma ou mais pastas locais com código-fonte, scripts de banco e documentos. O analista aponta para pastas que já existem na máquina; o app não clona repositórios no MVP.
- **Mapeamento inicial** do sistema (inventário, linguagens, módulos, tabelas, pontos de entrada), feito uma vez e atualizado de forma incremental.
- Três fluxos guiados:
  1. **Perguntar ao sistema:** pergunta livre, resposta com evidências.
  2. **Explicar funcionalidade:** regras de negócio, entradas, saídas, validações e casos de exceção de uma funcionalidade.
  3. **Analisar impacto de uma demanda:** a partir do texto da demanda (colado), o que muda, onde, quais regras existentes são afetadas, riscos e perguntas em aberto.
- **Base de conhecimento local:** regras de negócio e termos do glossário confirmados pelo analista ficam guardados e são usados em análises seguintes.
- **Documento da análise:** editável no app, com evidências navegáveis, revisão e aprovação, exportação para Markdown e Word.
- **Provedor de IA configurável:** API da Anthropic, Amazon Bedrock ou Google Vertex AI.
- **Registro de envios:** tudo o que foi enviado ao provedor, por análise, consultável e exportável.
- **Estimativa e limite de custo** por análise.

### Fase 2 (ainda standalone)

- Fluxo **revisar história** (confronta uma história ou especificação com o código e aponta lacunas).
- Fluxo **gerar cenários de teste** a partir das regras de uma funcionalidade.
- Integração com Jira (ler a demanda pela chave da issue e publicar a análise como comentário ou anexo).
- Clone de repositório Git pelo app (HTTPS com credencial do sistema operacional).
- Conexão somente leitura a um banco de dados para extrair o schema, além de arquivos DDL.
- Exportação para Confluence.

### Edição comercial (EE), futura e condicionada à adoção

Fora deste repositório. Só registrada aqui para orientar decisões do núcleo:

- Servidor compartilhado, multiusuário, com a mesma lógica do núcleo.
- Base de conhecimento compartilhada por equipe e por sistema.
- Controle de acesso, trilha de auditoria centralizada, políticas de envio de dados.
- Gestão centralizada de credenciais e de custo.

### Fora de escopo

- Qualquer alteração no código ou no banco analisado.
- Geração de código.
- Execução do sistema analisado ou de testes.
- Uso multiusuário ou sincronização entre máquinas.
- Modelos de IA locais (avaliar depois; ver seção 13).

## 4. Fluxos

Os exemplos usam um sistema fictício de faturamento ("Faturamento Exemplo").

### 4.1 Primeiro uso

1. O analista instala o app e o abre.
2. Assistente de configuração: escolhe o provedor de IA e informa a credencial (guardada no cofre do sistema operacional).
3. Cadastra o sistema: nome e uma ou mais pastas. O app mostra o que vai analisar (contagem de arquivos por tipo) e o que vai ignorar (binários, dependências, arquivos de configuração com segredos), e permite ajustar.
4. O app estima o custo do mapeamento inicial e pede confirmação.
5. Mapeamento inicial roda com barra de progresso; pode ser interrompido e retomado.
6. Ao final, mostra o mapa do sistema e sugere uma primeira pergunta.

### 4.2 Perguntar ao sistema

Exemplo: "Como é calculada a multa por atraso de pagamento?"

1. O analista digita a pergunta.
2. O agente consulta o mapa e a base de conhecimento, procura e lê os trechos relevantes.
3. A resposta aparece com cada afirmação ligada às evidências (arquivo, linhas, trecho). Clicar abre o trecho no visualizador do app.
4. O analista pode marcar uma afirmação como **confirmada** (vai para a base de conhecimento como regra de negócio), **incorreta** (com comentário, também registrado) ou pedir aprofundamento.

### 4.3 Explicar funcionalidade

O analista escolhe a funcionalidade no mapa ou a descreve. O resultado é um documento com seções fixas: objetivo, atores e telas, entradas, regras de negócio (numeradas, cada uma com evidência), validações, mensagens ao usuário, efeitos em dados (tabelas lidas e gravadas), integrações, casos de exceção e perguntas em aberto.

### 4.4 Analisar impacto de uma demanda

Exemplo de demanda: "Permitir parcelamento da fatura em até 3 vezes para clientes pessoa jurídica."

1. O analista cola o texto da demanda (e, se quiser, anexa um documento).
2. O agente identifica os conceitos da demanda, localiza no sistema onde eles vivem e levanta as regras existentes relacionadas.
3. Resultado em seções fixas: resumo da demanda como entendida, funcionalidades afetadas, regras existentes afetadas (com evidência), dados afetados, integrações afetadas, conflitos e riscos, perguntas para o solicitante, estimativa qualitativa de abrangência (pequena, média, grande, com justificativa).
4. O analista revisa, edita, aprova e exporta.

## 5. Arquitetura

```
┌───────────────────────────── App desktop (Tauri) ─────────────────────────────┐
│  UI (web)                                                                     │
│     │ comandos Tauri                                                          │
│  cba-app (casca: janelas, cofre de credenciais, diálogos de arquivo)          │
│     │                                                                         │
│  cba-core ── domínio, casos de uso, persistência (SQLite), eventos            │
│     ├── cba-agent ── laço do agente, ferramentas somente leitura, provedores  │
│     ├── cba-index ── inventário, mapa do sistema, busca, extratores           │
│     └── cba-export ─ Markdown e Word                                          │
└───────────────────────────────────────────────────────────────────────────────┘
          │ leitura                                        │ HTTPS
   pastas do sistema analisado                       provedor de IA
```

- **Um processo só.** Sem daemon, sem servidor local, sem portas abertas. O backend roda dentro do processo do Tauri.
- **O núcleo não depende do Tauri.** `cba-core` e as crates abaixo dele não conhecem a UI. Isso permite testes sem interface e, no futuro, um servidor EE que reaproveite o núcleo.
- **Linguagem:** Rust no backend, como no ctower-agents. A UI segue a mesma pilha web usada lá.
- **Reaproveitamento do ctower-agents:** sem dependência entre os repositórios. Trechos úteis (execução de processo, segredos, cliente do Jira na fase 2) são copiados e adaptados, com comentário de origem no topo do arquivo.

## 6. Motor do agente

### 6.1 Laço próprio

O agente é um laço de chamadas à API do provedor com ferramentas definidas pelo app. Não usa Claude Code nem outro agente externo. Motivos:

- cabe no binário, sem runtime extra para instalar;
- somente leitura por construção: só existem as ferramentas abaixo;
- controle total sobre citações, custo e registro de envios.

### 6.2 Ferramentas

Todas operam só dentro das pastas cadastradas do sistema, respeitam a lista de exclusões e têm limites de tamanho.

| Ferramenta | O que faz |
|---|---|
| `mapa` | Retorna o mapa do sistema ou de um módulo (resumos gerados no mapeamento). |
| `listar` | Lista arquivos de uma pasta, com filtros por padrão e tipo. |
| `buscar` | Busca textual ou por expressão regular, com contexto, limitada em número de resultados. |
| `ler` | Lê um arquivo ou um intervalo de linhas. |
| `estrutura` | Lista as rotinas, procedures, classes e telas de um arquivo (extratores da seção 7). |
| `schema` | Consulta tabelas, colunas e relacionamentos extraídos dos scripts de banco. |
| `referencias` | Onde um identificador (rotina, tabela, campo) é usado. |
| `conhecimento` | Consulta a base de conhecimento local (regras confirmadas, glossário). |
| `afirmar` | Registra uma afirmação do resultado com suas evidências (ver 6.3). |

Não há ferramenta de escrita, de execução nem de rede.

### 6.3 Evidências

- O resultado de uma análise é composto de **afirmações** registradas pela ferramenta `afirmar`, não de texto livre. Cada afirmação tem: texto, seção do documento, nível de confiança e lista de evidências.
- Uma **evidência** é `(arquivo, linha inicial, linha final, hash do trecho)`. O app valida na hora do registro: o arquivo existe, o intervalo é válido e o trecho foi efetivamente lido pelo agente nesta análise. Evidência inválida é recusada e o agente é informado.
- **Níveis de confiança:**
  - **confirmado no código:** evidência direta;
  - **inferido:** dedução a partir de evidências, que ficam listadas;
  - **a confirmar:** sem evidência suficiente; vira pergunta em aberto no documento.
- Se o arquivo mudar depois da análise, o hash denuncia e a evidência aparece como **desatualizada**.

### 6.4 Modelos

Modelo configurável por papel: um modelo mais capaz para as análises e um mais barato e rápido para o mapeamento inicial e os resumos. Os padrões acompanham os modelos mais recentes do provedor e ficam numa configuração, não no código. Uso de cache de prompt para o mapa e a base de conhecimento, que se repetem entre chamadas.

### 6.5 Custo

- Antes de rodar: estimativa (faixa) com base no tamanho do mapa e no fluxo escolhido.
- Durante: limite por análise (padrão configurável). Ao atingir, o agente é instruído a concluir com o que tem e marcar o restante como "a confirmar".
- Depois: tokens e custo estimado guardados na análise.

## 7. Mapeamento do sistema

Objetivo: dar ao agente um ponto de partida barato, para que cada pergunta não comece explorando o sistema do zero.

1. **Inventário:** arquivos, tamanho, linguagem (por extensão e conteúdo), exclusões aplicadas.
2. **Extração estrutural** (sem IA, por extratores por linguagem): rotinas, procedures, classes, telas, tabelas e colunas (DDL), chamadas e referências entre elas, quando o extrator consegue identificar.
3. **Resumos** (com IA, modelo barato): um resumo por arquivo relevante e um por módulo (pasta ou agrupamento detectado), em linguagem de negócio.
4. **Glossário candidato:** termos de negócio recorrentes, para o analista confirmar.

Tudo é guardado por hash de arquivo. Na reabertura, só o que mudou é reprocessado.

### Extratores no MVP

Extrator genérico (texto, identificadores, comentários) para qualquer linguagem, mais extratores específicos para: SQL e PL/SQL (DDL, procedures, triggers), COBOL (divisões, parágrafos, copybooks, `CALL`), Delphi/Pascal (units, forms `.dfm`), Visual Basic 6 (módulos, forms), Java e C#. A lista final depende dos sistemas usados no piloto (ver seção 13).

## 8. Modelo de dados

SQLite local, um arquivo por instalação, identificadores UUID v7. Mudanças relevantes geram um registro em `evento` (quem, quando, o quê), pensando em auditoria e numa futura sincronização com a EE.

| Entidade | Conteúdo principal |
|---|---|
| `sistema` | nome, pastas, exclusões, configuração de modelos |
| `arquivo` | caminho, hash, linguagem, resumo |
| `simbolo` | rotina, procedure, tabela, tela, com arquivo e intervalo |
| `referencia` | símbolo de origem → símbolo de destino |
| `analise` | fluxo, entrada (pergunta ou demanda), estado (rascunho, em revisão, aprovada), custo |
| `afirmacao` | análise, seção, texto, confiança, estado da revisão |
| `evidencia` | afirmação, arquivo, intervalo, hash do trecho |
| `regra` | regra de negócio confirmada, com evidências e análise de origem |
| `termo` | termo do glossário, definição, estado |
| `envio` | análise, horário, provedor, modelo, conteúdo enviado (ou referência a ele), tokens |
| `evento` | registro de mudanças |

## 9. Provedores, credenciais e privacidade

- **Provedores:** API da Anthropic, Amazon Bedrock e Google Vertex AI. Em ambiente corporativo, um desses costuma já estar aprovado pela área jurídica e de segurança, o que pesa mais na adoção do que qualquer recurso.
- **Credenciais** no cofre do sistema operacional (Keychain no macOS, Credential Manager no Windows). Nunca no banco nem em arquivo de configuração.
- **Proxy:** respeita o proxy do sistema e permite configurar um manualmente, com certificado raiz da empresa.
- **Exclusões padrão:** dependências de terceiros, binários, arquivos com padrão de segredo (`.env`, chaves, `*.pfx`, arquivos de configuração com senha detectada). O analista vê e ajusta a lista.
- **Mascaramento:** padrões de segredo encontrados dentro de arquivos analisados são mascarados antes do envio.
- **Registro de envios:** tela que mostra, por análise, cada trecho enviado ao provedor. Exportável para que a área de segurança audite.
- **Sem telemetria** no MVP.

## 10. Interface

- **Início:** sistemas cadastrados, análises recentes, botões dos fluxos.
- **Sistema:** mapa (módulos, funcionalidades, tabelas), estado do mapeamento, glossário.
- **Nova análise:** escolha do fluxo, campo de entrada, estimativa de custo.
- **Análise em andamento:** progresso em linguagem simples ("lendo as regras de cálculo de multa"), sem expor chamadas de ferramenta, com opção de cancelar.
- **Documento da análise:** texto por seções, afirmações com selo de confiança, painel lateral com a evidência selecionada (trecho com destaque), ações de confirmar, marcar incorreta e comentar; aprovar; exportar.
- **Base de conhecimento:** regras confirmadas e glossário, com busca e a análise de origem.
- **Configurações:** provedor e credencial, modelos, limites de custo, exclusões, registro de envios.

Toda a interface em português no MVP, com textos preparados para tradução.

## 11. Exportação

- **Markdown:** documento com as evidências como notas de rodapé (`arquivo:linhas`).
- **Word (.docx):** mesmo conteúdo, com estilos simples e apêndice de evidências com os trechos.
- O selo de confiança de cada afirmação aparece nos dois formatos.

## 12. Distribuição e licença

- **Instaladores assinados:** MSI ou NSIS por usuário no Windows (sem administrador) e DMG notarizado no macOS. Atualização automática pelo atualizador do Tauri, desligável.
- **Licença:** Apache-2.0 neste repositório. A EE, se existir, fica em repositório separado e fechado.
- **Contribuições externas:** exigem aceite de um CLA antes da primeira contribuição, para preservar a possibilidade da EE. Definir o texto do CLA antes de abrir o repositório para contribuições.
- **Branches:** `develop` para integração e `main` para releases, como no ctower-agents.

## 13. Riscos e questões em aberto

**Riscos**

- **Resposta errada com cara de autoridade.** O público não consegue auditar o código. Mitigação: afirmações estruturadas, evidência validada, nível de confiança visível, revisão obrigatória antes de aprovar.
- **Sistemas muito grandes.** Mapeamento caro e lento. Mitigação: inventário e extração sem IA primeiro; resumos sob demanda por módulo; limite de custo.
- **Regra de negócio fora do código analisável:** no banco de produção (parâmetros em tabela), em jobs agendados, em sistemas vizinhos. Mitigação: o agente declara o limite ("a regra depende do valor de um parâmetro na tabela X, não disponível") como pergunta em aberto.
- **Bloqueio corporativo ao envio de código.** Mitigação: provedores corporativos, registro de envios, exclusões e mascaramento. Modelos locais ficam como avaliação futura.
- **Adoção.** O concorrente é o chat genérico, com atrito zero. O primeiro uso precisa ser rápido (critério de sucesso 1).

**Questões em aberto**

1. Quais linguagens legadas priorizar nos extratores? Depende do primeiro piloto.
2. Conexão a banco de dados (somente schema) entra no MVP ou fica para a fase 2?
3. Precisamos de um modo de "pacote de código": um desenvolvedor prepara a pasta do sistema e entrega ao analista que não tem acesso ao repositório?
4. Formato de entrega no Jira na fase 2: comentário, anexo ou campo próprio?
5. Vale avaliar modelos locais para clientes que não podem enviar código para fora?

## 14. Critérios de aceite do MVP

1. Instalação no Windows sem permissão de administrador e no macOS, com o primeiro uso concluído dentro do critério de sucesso 1, verificada com pelo menos três analistas de fora do time.
2. Em um sistema de teste com regras conhecidas (fixture do repositório), os três fluxos encontram as regras esperadas e nenhuma afirmação marcada como "confirmado no código" deixa de ter evidência válida.
3. Testes automatizados garantem que nenhuma ferramenta do agente consegue ler fora das pastas cadastradas nem escrever em disco fora do diretório de dados do app.
4. O registro de envios mostra 100% do conteúdo enviado ao provedor em uma análise.
5. Reabrir um sistema após alterar arquivos reprocessa só os arquivos alterados, e as evidências afetadas aparecem como desatualizadas.
6. Exportação em Markdown e Word preserva seções, confiança e evidências.
