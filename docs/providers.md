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

As conexões de banco têm as opções `usuario` (obrigatória), `esquema`, `tempo_limite_s` (padrão 30), `limite_linhas` (padrão 200), `tls` (`exigir`, `preferir` ou `desligar`; padrão `preferir`) e `ambiente`.
