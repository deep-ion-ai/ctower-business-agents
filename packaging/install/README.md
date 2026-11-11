# Instalação por script

Os dois scripts desta pasta são anexados a toda release e instalam o app e as dependências que faltarem, sem administrador. Como o `curl` e o `irm` não marcam o arquivo baixado, o app abre sem o aviso do Gatekeeper ou do SmartScreen que aparece com instaladores baixados pelo navegador.

| Sistema | Comando |
|---|---|
| macOS (e Linux, do código-fonte) | `curl -fsSL https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.sh \| sh` |
| Windows | `irm https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.ps1 \| iex` |

Prefira baixar, ler e só então rodar: o cabeçalho de cada script explica como e lista as opções.

## O que fazem

- Mostram o plano e pedem confirmação (`--yes` / `-Yes` / `CBA_YES=1` para não perguntar).
- **App:** baixam o instalador da release, conferem o SHA-256 contra o `SHA256SUMS` da mesma release (divergência aborta sem instalar nada) e instalam por usuário: `~/Applications` no macOS, `%LOCALAPPDATA%\Ctower Business Agents` no Windows, com o atalho do menu Iniciar criado pelo instalador. Não reinstalam a mesma versão.
- **git:** Command Line Tools da Apple (janela do sistema) ou `winget install --id Git.Git --scope user`.
- **WebView2** (Windows 10): instalador oficial da Microsoft, por usuário, com a assinatura conferida.
- **Claude Code e Claude Desktop:** só mostram o comando oficial da Anthropic e o link de download. O login (`/login`) é do analista. Executar o instalador oficial do Claude Code com confirmação depende da #43.
- **Log:** `~/Library/Logs/cba-install.log`, `${XDG_STATE_HOME:-~/.local/state}/cba/install.log` ou `%LOCALAPPDATA%\Ctower Business Agents\install.log`. Nunca pedem nem guardam credenciais.

## Do código-fonte (`--from-source` / `-FromSource`)

Instala, se faltarem, o rustup (`--no-modify-path`, em `~/.cargo`) e o Node 24 oficial de nodejs.org (conferido pelo `SHASUMS256.txt`, em `~/.local/share/cba/node` ou `%LOCALAPPDATA%\cba\node`). Usa o diretório atual se ele já for este repositório; senão, clona em `~/.local/share/cba/src` (ou `%LOCALAPPDATA%\cba\src`). Compila com `npm ci && npm run tauri build` em `ui/` e instala o resultado como o caminho binário. É o único caminho no Linux, que ainda não tem app publicado (as bibliotecas do sistema, como a `libwebkit2gtk-4.1-dev`, exigem administrador e ficam com o usuário). No Windows, as Visual Studio Build Tools exigem administrador: o script avisa no plano antes de instalá-las.

## Testes

`packaging/tests` roda os scripts contra uma release falsa em disco (`CBA_BASE_URL=file://...`) em modo de teste (`--dry-run` / `CBA_DRY_RUN=1`), que baixa e confere o SHA-256 sem instalar nada:

```sh
python3 -m unittest discover -s packaging/tests -p 'test_*.py'      # install.sh
pwsh -File packaging/tests/test-install.ps1 -Shell pwsh               # install.ps1, no Windows
```
