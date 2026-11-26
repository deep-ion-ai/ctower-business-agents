# Providers: como criar um tipo de conexão novo

O app fala com sistemas externos por **conexões**: um GitHub, um GitLab, um Jira, um banco PostgreSQL ou MySQL/MariaDB. Cada conexão é cadastrada **uma vez**, nas Configurações do app, e os projetos a referenciam pelo identificador. A base do projeto, que pode ser publicada, nunca guarda a conexão nem a credencial.

Tudo o que depende do **tipo** da conexão fica num **descritor** (`DescritorProvider`), um por tipo, em `crates/cba-repos/src/providers/`. O registro (`providers::todos()`) é a única lista de tipos que o app conhece. A UI recebe os descritores do backend (`ResumoDescritor`) e monta a lista de tipos e o formulário a partir deles.

Adicionar um tipo novo (Oracle, SQL Server, Azure DevOps…) é **implementar um descritor e registrá-lo**. Se você se pegar escrevendo um `match` por tipo em outro lugar, pare e leia a seção [O que não fazer](#o-que-não-fazer).

## O modelo

```text
Conexao { id, nome, tipo, endereco, credencial: FonteCredencial, opcoes }
FonteCredencial = Chaveiro { conta } | Comando { cmd } | Cli | Arquivo { caminho } | Nenhuma
```

- `id`: identificador estável, que os projetos referenciam.
- `nome`: o que o analista vê ("GitLab interno", "Banco de homologação"). Único, sem diferenciar maiúsculas.
- `tipo`: o `tipo()` de um descritor registrado (`gitlab`, `postgres`).
- `endereco`: no formato do descritor. Host para forjas, URL para o Jira, `host:porta/banco` para bancos.
- `credencial`: **de onde** vem a credencial, nunca o valor.
- `opcoes`: mapa de texto com as chaves declaradas em `campos_opcoes()`.

## Resolvedor de credenciais

A credencial de uma conexão sai sempre pelo resolvedor (`providers::Resolvedor::global()`), nunca lida direto do chaveiro ou de um comando:

- `resolver(conexao, chaveiro, cli)` lê a fonte e devolve um `Secret`. O resultado fica em cache em memória durante a vida do processo (menos a fonte `Arquivo`, lida a cada vez). Gravar ou apagar um token no chaveiro esvazia o cache.
- `resolver_com_sobrescrita` aceita a conta do token próprio de um repositório, que vale no lugar da credencial da conexão.
- `com_nova_tentativa(obter, operacao)`: se a operação falhar com credencial recusada (401, 403, `Unauthorized`, `Forbidden`), a credencial sai do cache e a operação roda mais uma vez, com a credencial lida de novo. O erro do consumidor diz se é recusa pelo trait `Recusada`.
- Todo segredo resolvido entra no **mascaramento central** (`providers::mascara`): o terminal embutido, o log em arquivo e as mensagens de erro (`process::cite`) trocam o valor por `***`. Uma senha de banco que não passa pelo resolvedor entra com `mascara::registrar_texto`.

## Onde as conexões ficam

As conexões cadastradas em **Configurações › Conexões** ficam em `<dados do app>/config/conexoes.json` (com o resultado do último **Testar** em `conexoes_testes.json`), nunca na base do projeto. A janela e o servidor MCP apontam o catálogo do processo (`providers::Catalogo::global()`) para esse arquivo na abertura, e ele é relido quando muda: uma conexão editada na janela vale também para as ferramentas do agente.

O projeto referencia a conexão pelo `id` no campo `conexao` do rastreador (`GithubConfig`, `GitlabConfig`, `JiraConfig`), de cada repositório de código e do repositório próprio da documentação, e guarda só o que é dele (chave do projeto Jira, JQL, campos, `owner/repo` ou caminho, pasta local, branch, subpasta). Ao gravar o projeto, o host e a URL vêm da conexão e a credencial antiga (`credential_cmd`, comando do repositório) sai. O token próprio de um repositório (`repo:<host>/<caminho>` no chaveiro) continua valendo como **sobrescrita** da credencial da conexão.

Uma conexão em uso não pode ser removida, nem trocar de tipo ou de endereço: o app diz quais projetos a usam. Trocar o token na conexão vale para todos os projetos que a usam, inclusive para os clones e a publicação.

Enquanto um cadastro ainda não referencia uma conexão nomeada, `providers::legado` monta uma conexão implícita a partir do cadastro de hoje (`credential_cmd`, token global por host, token próprio do repositório) e resolve por esse mesmo caminho.

## Passo a passo

O exemplo é um provider fictício, o **Rastreador Exemplo**, que existe só nos testes (`crates/cba-repos/src/providers/tests.rs`, `DescritorFicticio`).

### 1. Implementar o descritor

Crie o descritor em `crates/cba-repos/src/providers/descritores.rs` (ou num arquivo próprio no mesmo módulo, se for grande) e acrescente-o em `Registro::padrao()`:

```rust
pub struct DescritorRastreadorExemplo;

impl DescritorProvider for DescritorRastreadorExemplo {
    fn tipo(&self) -> &'static str { "rastreador_exemplo" }
    fn rotulo(&self) -> &'static str { "Rastreador Exemplo" }
    fn icone(&self) -> &'static str { "list-checks" }
    fn capacidades(&self) -> &'static [Capacidade] { &[Capacidade::Rastreador] }
    fn endereco_padrao(&self) -> &'static str { "rastreador.example.com" }
    fn formato_endereco(&self) -> &'static str { "host, como rastreador.example.com" }
    fn normalizar_endereco(&self, e: &str) -> Result<String, String> { /* ... */ }
    fn fontes_aceitas(&self) -> &'static [TipoFonte] { &[TipoFonte::Chaveiro, TipoFonte::Comando] }
    fn variaveis_ambiente(&self, c: &Conexao) -> Vec<VariavelAmbiente> {
        vec![
            VariavelAmbiente::credencial("EXEMPLO_TOKEN"),
            VariavelAmbiente::texto("EXEMPLO_HOST", host_de(&c.endereco)),
        ]
    }
    fn campos_opcoes(&self) -> &'static [CampoOpcao] { CAMPOS }
    fn passos_prontidao(&self) -> &'static [PassoProntidao] { PASSOS }
}
```

- `tipo()` é gravado no cadastro: escolha uma vez e não mude. Use minúsculas e `_`.
- `normalizar_endereco()` valida e devolve a forma canônica (host em minúsculas, sem barra final). Recuse endereço com usuário e senha embutidos (`https://u:senha@host`, `postgres://u:senha@host/db`): a credencial vai pela fonte, nunca no endereço. A mensagem de erro não pode repetir o endereço recusado quando ele pode conter uma senha.
- `campos_opcoes()` declara cada opção com rótulo, ajuda, tipo (`Texto`, `Numero { min, max }`, `Escolha`, `Booleano`), padrão e obrigatoriedade. A validação padrão (`DescritorProvider::validar`) já confere cada campo e recusa chaves desconhecidas. Para uma regra que envolve mais de um campo, implemente `validar_extra()`.

### 2. Fontes de credencial aceitas

`fontes_aceitas()` lista as fontes que fazem sentido para o tipo, a primeira é a padrão de uma conexão nova:

| Fonte | Quando aceitar |
| --- | --- |
| `Chaveiro { conta }` | Sempre que houver credencial. É o padrão. |
| `Comando { cmd }` | Quando o analista guarda o token num gerenciador (`pass`, `op`). Roda sem TTY, com prazo, e vale a primeira linha do `stdout`. |
| `Cli` | Só quando a CLI do provider guarda o login dela e o devolve com segurança (`gh auth token`). Hoje, só o GitHub. |
| `Arquivo { caminho }` | Quando o token vem de um arquivo gerenciado por outra ferramenta. |
| `Nenhuma` | Quando o acesso sem credencial é possível (repositório público, banco local). |

### 3. Prontidão

`passos_prontidao()` declara, na ordem, o que o botão **Testar** da conexão verifica, cada passo com a orientação mostrada quando ele falha. Os passos típicos:

- a CLI instalada, com a versão e o caminho do executável (para tipos com CLI);
- a credencial disponível;
- "quem sou eu" no servidor, que prova que a credencial vale;
- para bancos: conectar, versão do servidor, sessão só de leitura confirmada e esquemas visíveis.

A orientação diz **o que fazer**, não só o que deu errado.

Cada passo é executado pelo próprio descritor, em `executar_passo(id, ctx)`, que devolve o detalhe (versão e caminho da CLI, "conectado como ana") ou o motivo da falha. O `ContextoProntidao` traz a conexão, as CLIs e a credencial: `ctx.resolver_credencial(cli)` é o passo de credencial pronto (lê a fonte pelo resolvedor e diz de onde ela veio, nunca o valor), `ctx.comando(programa)` monta o comando com as variáveis de ambiente do descritor e `ctx.saida(cmd)` roda e mascara o erro. `providers::prontidao::versao_e_caminho` é o passo da CLI. Um passo que o descritor ainda não executa (`None`) aparece como pulado.

O mesmo caminho serve ao **Testar** das Configurações (`providers::prontidao::testar`) e à saúde do projeto: a integração de um repositório ou do rastreador que referencia uma conexão começa pelos passos dela e acrescenta só o que é do projeto (acesso ao repositório, issues ligadas).

### 4. Capacidades

`capacidades()` diz onde a conexão pode ser usada:

- `Rastreador`: o rastreador de issues de um projeto;
- `Forja`: repositórios de código e o destino da documentação;
- `Banco`: consultas só de leitura numa análise.

A tela do projeto oferece, em cada lugar, só as conexões com a capacidade certa (`Registro::com_capacidade`).

### 5. Testes obrigatórios

Um tipo novo entra com:

- **descritor**: endereço normalizado e recusado, fontes aceitas e recusadas, opções válidas e inválidas, e o `ResumoDescritor` serializado como a UI espera;
- **resolvedor**: a credencial de cada fonte aceita chega só ao ambiente do processo filho ou ao driver, e nunca aparece em argumento, mensagem nem log;
- **prontidão**: cada passo com sucesso e com falha, usando uma CLI falsa (um script em `tempfile`, como em `demanda.rs`) ou um servidor falso. Os bancos usam contêiner nos testes de integração.

### 6. Documentação

Acrescente o tipo na tabela de tipos abaixo e, se ele tiver CLI, as variáveis de ambiente que ela lê.

## O que não fazer

- **`match` por tipo fora do descritor.** Nada de `if conexao.tipo == "gitlab"` nem de `match` em literais de tipo nos crates ou na UI. Se um comportamento depende do tipo, ele vira um método do descritor.
- **Lista de tipos na UI.** A UI usa os descritores que o backend devolve. Literais `"jira"`, `"gitlab"` e `"github"` não entram em componentes.
- **Token em argumento ou URL.** A credencial vai só no ambiente do processo filho (`aplicar_ambiente`) ou direto para o driver. Nunca em `argv`, em `https://token@host`, em `.git/config` nem num arquivo temporário legível por outros.
- **Token no cadastro.** A conexão guarda a fonte. O valor fica no chaveiro, no comando ou no arquivo do analista.
- **Token em log ou mensagem.** Toda credencial resolvida passa pelo mascaramento central. Mensagens de erro citam o comando ou a conta, nunca a saída que pode conter o segredo.
- **Conexão na base do projeto.** O projeto referencia a conexão pelo `id` e guarda só o que é dele (chave do projeto Jira, `owner/repo`, pasta local, branch).

## Tipos conhecidos

| Tipo | Rótulo | Capacidades | Endereço | Fontes | Ambiente da CLI |
| --- | --- | --- | --- | --- | --- |
| `github` | GitHub | forja, rastreador | host (`github.com`) | chaveiro, cli, comando, arquivo, nenhuma | `GH_TOKEN`, `GH_HOST` |
| `gitlab` | GitLab | forja, rastreador | host (`gitlab.com`) | chaveiro, comando, arquivo, nenhuma | `GITLAB_TOKEN`, `GITLAB_HOST` |
| `jira` | Jira | rastreador | URL (`https://jira.example.com`) | chaveiro, comando, arquivo | `JIRA_API_TOKEN`, `JIRA_CONFIG_FILE` |
| `postgres` | PostgreSQL | banco | `host:porta/banco` (porta 5432) | chaveiro, comando, arquivo, nenhuma | (driver nativo) |
| `mysql` | MySQL/MariaDB | banco | `host:porta/banco` (porta 3306) | chaveiro, comando, arquivo, nenhuma | (driver nativo) |

As conexões de banco têm as opções `usuario` (obrigatória), `esquema`, `tempo_limite_s` (padrão 30), `limite_linhas` (padrão 200), `tls` (`exigir`, `preferir` ou `desligar`; padrão `preferir`), `ambiente` e `confirmo_producao`. Um `ambiente` que indica produção (`produção`, `prod`, `PRD`, `production`) só é aceito com `confirmo_producao = true`; a mensagem recomenda uma réplica de leitura ou a base de sustentação.

## Bancos: acesso só de leitura

O acesso fica na crate `cba-banco`, com driver nativo (`sqlx`), sem cliente instalado na máquina. O descritor diz o dialeto (`DescritorProvider::dialeto_sql`); a crate escolhe o driver por ele, nunca pelo tipo. Um banco novo (Oracle, SQL Server) precisa do descritor com o dialeto e do driver em `cba-banco` (abertura da sessão só de leitura, versão, escrita de teste e consultas do catálogo).

A só leitura é imposta pelo app, além do que o usuário do banco permite:

- **validação antes de enviar** (`cba_banco::validar`), com o parser SQL do dialeto: uma instrução só, `SELECT` ou `WITH … SELECT`. Recusa escrita e DDL (também dentro de um `WITH`), `CALL`, `SET`, `LOCK`, `SELECT … INTO`, `FOR UPDATE`/`FOR SHARE`, funções com efeito fora da consulta (arquivos do servidor, travas consultivas, sequências, `dblink`, `query_to_xml`, `SLEEP`…), comentários executáveis do MySQL (`/*! … */`) e comentários com `;`;
- **PostgreSQL:** a sessão abre com `default_transaction_read_only = on`, `statement_timeout`, `lock_timeout` e `idle_in_transaction_session_timeout`, e cada consulta roda em `BEGIN READ ONLY … ROLLBACK`;
- **MySQL/MariaDB:** `SET SESSION TRANSACTION READ ONLY`, `max_execution_time` (MySQL) ou `max_statement_time` (MariaDB) e `lock_wait_timeout`, com cada consulta em `START TRANSACTION READ ONLY … ROLLBACK`;
- o app corta o resultado no `limite_linhas` (marcando `cortada`) e cancela a consulta que passar do tempo limite, mesmo se o servidor não responder.

A prontidão (`cba_banco::testar`) roda os passos do descritor: conectar, versão do servidor, só leitura (tenta criar uma tabela, temporária no PostgreSQL e comum no MySQL, porque a transação só de leitura do MySQL aceita tabelas temporárias, e espera a recusa do banco) e esquemas visíveis. O **Testar** da conexão nas Configurações usa esses passos de verdade (`cba_banco::prontidao::prontidao`, chamado pelo app para os tipos com a capacidade `banco`, porque `cba-repos` não depende do driver): cadastro, senha do banco, conectar, versão, só leitura e esquemas, com a orientação de quem falhou e os seguintes pulados.

A senha nunca monta uma string de conexão: vai só para as opções do driver. Ela entra no mascaramento do processo, o `Debug` da configuração mostra `***` e toda mensagem de erro passa pelo mascaramento.

A ferramenta `schema` lê o banco conectado à sessão com `origem: "banco"` (tabelas, colunas, tipos, chaves primárias e estrangeiras, pelo catálogo do servidor), com os indícios de dado pessoal pelos nomes e tipos reais. O banco chega às ferramentas por `Ambiente::banco` (o trait `cba_core::banco::BancoConectado`, implementado por `cba_banco::BancoDaConexao`).

## Bancos: `consultar_dados` numa análise (#440)

**Bancos do projeto.** Em Configurações › projeto › Código e demandas, a seção **Bancos de dados** escolhe quais conexões de banco o projeto usa (campo `bancos` do cadastro, com os `id`). A conexão em uso por um projeto aparece nos usos dela e não pode ser removida.

**Banco da análise.** No painel da análise, "Usar o banco <nome> nesta análise" liga uma dessas conexões na análise (`banco` no estado `A-NNNN.estado.json`). Só então a ferramenta MCP `consultar_dados` aparece para o agente. O servidor MCP guarda a análise da sessão (a do último argumento `analise`), relê o estado antes de cada chamada e preenche `Ambiente::banco` com o banco ligado, aberto por `cba_banco::FonteDoCatalogo` (catálogo de conexões e resolvedor de credenciais). Quando a lista de ferramentas visíveis muda, o servidor envia `notifications/tools/list_changed` e o cliente pede a lista de novo. Sem o banco ligado, o agente não vê a ferramenta, e a chamada direta é recusada.

**Aprovação por consulta.** A chamada (`analise`, `objetivo`, `sql`) valida o SQL (só leitura, uma instrução) e grava o pedido pendente na análise, com o objetivo, o SQL, a conexão e o ambiente. O painel da análise mostra "Consulta N ao banco — aguardando sua aprovação", e a conversa mostra "Consulta ao banco aguardando sua aprovação". O analista escolhe **Executar**, **Editar e executar** (roda o SQL editado; o registro guarda os dois) ou **Recusar**, com um motivo que volta ao agente na resposta. Sem decisão em 15 minutos, a consulta expira. Enquanto espera, o servidor MCP não atende outra requisição.

**Execução e mascaramento.** A consulta aprovada roda pela sessão só leitura da conexão, com a validação, o tempo limite e o limite de linhas dela. Antes de o resultado chegar ao agente, sempre e sem exceção (`cba_tools::mascaramento`):

- a coluna com indício de dado pessoal pelo nome e pelo tipo reais (`dados_pessoais.rs`) sai inteira como `[mascarado]`; as tabelas do `FROM`/`JOIN` fazem o papel da tabela da coluna (`nome` numa consulta a `cliente` é pessoal, a `produto` não);
- em qualquer coluna de texto, CPF, CNPJ, e-mail e telefone viram `[cpf]`, `[cnpj]`, `[e-mail]` e `[telefone]`, e nomes próprios ao lado de um documento (no mesmo texto ou na mesma linha) viram `[nome]`;
- os nulos continuam nulos; a resposta lista as colunas mascaradas e o motivo.

**Registro.** Todas as consultas, aprovadas, editadas, recusadas, expiradas ou com falha, ficam no estado da análise (`consultas`): o objetivo, a conexão, o SQL proposto e o aprovado, quem decidiu (o usuário do sistema), quando, o motivo da recusa, a quantidade de linhas, os nomes das colunas e as mascaradas. Os valores nunca são gravados. O painel mostra o registro em "Consultas ao banco nesta análise".

**Evidência de dados.** O agente cita o resultado em `afirmar` com `dados: [{consulta, resumo}]` (só de consulta executada). Uma afirmação confirmada pode ter só a evidência de dados. No documento, o resumo entra no texto da afirmação, "(em dados, AAAA-MM-DD: resumo)", com a nota `[^dados-N]` na seção de evidências: conexão, ambiente, data e hora (UTC), SQL e linhas. A seção avisa que a evidência vale para a data em que foi tirada: a revalidação confere o código e não reexecuta consultas. A exportação sem evidências tira as notas e fica só o resumo.

**Roteiro.** Com o banco ligado, o roteiro orienta o agente a confirmar o diagnóstico no caso relatado e medir a abrangência com agregados (`COUNT`, `GROUP BY`) e amostras pequenas, sem trazer linhas inteiras de pessoas. Na análise de impacto sem banco, orienta a deixar as consultas prontas (objetivo, SQL e significado do resultado) para quem tem acesso.

Os testes de integração sobem PostgreSQL, MySQL e MariaDB em contêiner:

```sh
cargo test -p cba-banco --test conteiner -- --ignored --test-threads=1
```
