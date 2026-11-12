# Contribuindo

Obrigado pelo interesse no ctower-business-agents. Este guia resume como propor mudanças, como compilar o projeto na sua máquina e o que esperamos de cada contribuição.

Ao participar, você concorda em seguir o [código de conduta](CODE_OF_CONDUCT.md). Vulnerabilidades não vão em issues públicas: veja a [política de segurança](SECURITY.md).

## Antes de começar

- **Abra uma issue** descrevendo o problema ou a proposta antes de mudanças grandes. Para correções pequenas (texto, typo, teste), pode ir direto ao PR.
- **Assine o CLA.** Toda contribuição externa exige o aceite do [Acordo de Licença de Contribuidor (CLA)](docs/cla/CLA.md) antes do merge. Veja [CLA](#cla).
- **Use só dados fictícios.** Veja [Dados nos exemplos](#dados-nos-exemplos).

## Branches

- `develop` é a branch de integração. Todo PR vai para ela.
- `main` recebe só o merge `develop → main`, que gera a release. Ninguém abre PR de trabalho contra a `main`.
- Crie a sua branch a partir da `develop`, com o tipo e o número da issue no nome: `feat/123-descricao`, `fix/123-descricao`, `docs/123-descricao`.
- Quem não tem acesso de escrita trabalha num fork e abre o PR do fork para a `develop` deste repositório.

## Commits e versões

Usamos [Conventional Commits](https://www.conventionalcommits.org/pt-br/v1.0.0/), em português: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`, `ci:`, com escopo opcional (`feat(mcp): ...`). Mudanças incompatíveis usam `!` ou o rodapé `BREAKING CHANGE:`.

- **O título do PR é validado pelo CI** e precisa seguir a convenção, porque é ele que fica no histórico quando o PR é mesclado com squash e que o `release-please` lê.
- **Versões seguem o [SemVer](https://semver.org/lang/pt-BR/)** e são geradas pelo `release-please` a partir dos commits: `fix` sobe o patch, `feat` sobe o minor e mudança incompatível sobe o major. O `release-please` também escreve o `CHANGELOG.md`.
- **Não altere versões à mão nem edite o `CHANGELOG.md`**: o PR de release faz isso.
- Referencie a issue no corpo do PR (`Resolve #123`).

## Dados nos exemplos

O produto lê o código de sistemas legados, e esses sistemas costumam ter regras de negócio, dados pessoais e segredos. Por isso:

- **Nunca** coloque no repositório código, nomes, regras, dados, logs, capturas de tela ou trechos de documento de um sistema real, seu ou de terceiros, nem anonimizados à mão.
- Exemplos, testes, prompts, capturas de tela e issues usam o sistema fictício **Faturamento Exemplo**, em `fixtures/faturamento-exemplo` (#5). Se o exemplo de que você precisa não existe nele, proponha a extensão do sistema fictício no mesmo PR.
- Ao relatar um bug encontrado num sistema real, reproduza-o no sistema fictício ou descreva-o sem copiar código nem dados.

PRs com dados de sistema real são fechados, e o histórico é reescrito para remover o conteúdo.

## Compilar localmente

### Instalação por script

O script de instalação `install.sh --from-source` (e o equivalente em PowerShell) vai instalar os pré-requisitos na pasta do usuário e compilar o projeto num passo só. Ele é tratado na issue #45 e ainda não está disponível; até lá, siga os passos manuais abaixo.

### Pré-requisitos em todos os sistemas

- **Rust** pelo [rustup](https://rustup.rs/). Não instale uma versão específica à mão: o arquivo `rust-toolchain.toml` na raiz fixa o toolchain, e o `rustup` o baixa sozinho no primeiro `cargo` dentro do repositório.
- **Node.js LTS** (pelo instalador oficial, `fnm`, `nvm` ou o gerenciador de pacotes do sistema), com `npm`.
- **git**.

### macOS

```sh
xcode-select --install
```

Isso instala as Xcode Command Line Tools (compilador, linker e SDK). O Xcode completo só é necessário para assinar e notarizar instaladores, o que o CI faz.

### Windows

- **Microsoft Visual Studio C++ Build Tools**, com a carga de trabalho "Desenvolvimento para desktop com C++" (instalar exige administrador).
- **WebView2**: já vem no Windows 11. No Windows 10, instale o "Evergreen Bootstrapper" da página oficial da Microsoft.
- Use o toolchain `msvc` do Rust (o padrão do `rustup` no Windows).

### Linux

O Linux não é plataforma de distribuição do MVP (que mira Windows e macOS), mas serve para desenvolver. O Tauri 2 precisa do WebKitGTK e de algumas bibliotecas do sistema.

Debian e Ubuntu:

```sh
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Fedora:

```sh
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel
sudo dnf group install "c-development"
```

Para outras distribuições, veja os [pré-requisitos do Tauri 2](https://v2.tauri.app/start/prerequisites/).

### Compilar e testar

```sh
git clone https://github.com/deep-ion-ai/ctower-business-agents.git
cd ctower-business-agents
(cd ui && npm ci && npm run build)   # o cba-app embute ui/dist no binário
cargo test --workspace
```

O núcleo (`cba-core` e as crates de baixo) não depende do Tauri. O app (`cba-app`, em `ui/src-tauri`) faz parte do workspace e só compila depois que `ui/dist` existe; para trabalhar só no núcleo sem construir a UI, use `cargo test --workspace --exclude cba-app`.

Interface e app:

```sh
cd ui
npm run tauri dev     # app com recarga da UI
npm run typecheck
npm test              # Vitest
npm run tauri build   # instalador do sistema atual (sem assinatura)
```

O executável `cba` tem os modos: sem argumentos abre o app; `cba mcp` sobe o servidor MCP em stdio; `cba hook <evento>` lê o JSON do hook na entrada padrão e responde; `cba --version` mostra a versão. Ex.: `cargo run -p cba-app -- --version`.

Os dados do app (índice, configuração, logs) ficam no diretório de dados do usuário, numa pasta `ai.deep-ion.ctower-business-agents` (`%LOCALAPPDATA%` no Windows, `~/Library/Application Support` no macOS, `~/.local/share` no Linux), separada da base de conhecimento. A variável `CBA_DATA_DIR` troca o lugar; `CBA_LOG` muda o nível do log (padrão `info`).

## Antes de enviar o PR

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace` passam.
- Mudanças de comportamento atualizam a spec ou o documento correspondente em `docs/`.
- Mudanças em roteiros, limites, descrições das ferramentas MCP, fixture ou gabarito passam pela [avaliação dos fluxos contra o gabarito](docs/avaliacao.md) antes e depois da mudança (`cargo run -p cba-avaliacao -- rodar`, com o Claude Code logado; usa a sua assinatura e não roda no CI). Ponha o resumo do relatório e o custo no PR.
- O título do PR segue os Conventional Commits.
- Exemplos e testes usam só o sistema fictício.

O CI roda nos runners do GitHub. Em PRs de fork, um mantenedor precisa aprovar a execução dos workflows.

## CLA

O projeto é licenciado sob a [Apache-2.0](LICENSE). Para manter aberta a possibilidade de uma edição comercial em repositório separado, toda contribuição externa exige o aceite do [CLA individual](docs/cla/CLA.md), baseado no Apache ICLA. Com ele, você mantém os direitos sobre a sua contribuição e concede ao mantenedor uma licença de direitos autorais e de patentes, inclusive para distribuí-la sob outros termos. A sua contribuição continua disponível aqui sob a Apache-2.0.

Como assinar:

1. Abra o PR. O bot do CLA comenta no PR e marca a verificação **CLA** como pendente se algum autor dos commits ainda não assinou.
2. Leia o [texto do CLA](docs/cla/CLA.md).
3. Comente no PR, exatamente:

   ```text
   Li o CLA e concordo com os seus termos
   ```

4. A verificação fica verde. A assinatura vale para todas as suas contribuições futuras neste repositório. Se a verificação não atualizar, comente `recheck`.

Todos os autores dos commits do PR precisam assinar, e o e-mail dos commits precisa estar ligado à sua conta do GitHub. As assinaturas ficam registradas na branch `cla-signatures` deste repositório.

> **Pendência antes de exigir o CLA:** a assinatura de código gratuita da SignPath Foundation (#41) exige licença aprovada pela OSI sem licenciamento duplo. Antes de ativar a verificação, o mantenedor precisa confirmar com a SignPath se este desenho (núcleo Apache-2.0 aqui, edição comercial em repositório separado, CLA que permite relicenciar) é aceito. Até lá, o texto do CLA pode mudar.
