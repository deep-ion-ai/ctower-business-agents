#!/bin/sh
# Instalador do Ctower Business Agents para macOS (e Linux, a partir do código-fonte).
#
# Recomendado: baixe, leia e só então rode.
#   curl -fsSLO https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.sh
#   less install.sh
#   sh install.sh
# Atalho:          curl -fsSL https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.sh | sh
# Sem perguntas:   ... | sh -s -- --yes
#
# O que faz, sempre mostrando o plano antes de executar:
#   - macOS: baixa o app da release, confere o SHA-256 publicado na mesma release
#     e instala em ~/Applications; instala o git (Command Line Tools) se faltar.
#   - Linux: não há binário publicado; --from-source compila e instala em ~/.local/bin.
#   - Claude Code e Claude Desktop: mostra como instalar pelos fluxos oficiais da Anthropic.
# Não usa sudo, não pede nem guarda credenciais e não reinstala o que já existe.
#
# Opções (ou variáveis de ambiente):
#   --yes            CBA_YES=1          não pergunta nada
#   --version X.Y.Z  CBA_VERSION        versão específica (padrão: a mais recente)
#   --from-source    CBA_FROM_SOURCE=1  compila do código-fonte (rustup e Node na pasta do usuário)
#   --dry-run        CBA_DRY_RUN=1      mostra o plano, baixa e confere o app, e não muda nada
#   --base-url URL   CBA_BASE_URL       origem das releases (espelhos e testes)
# Só para testes: CBA_PLATFORM=macos|linux força o sistema; CBA_APPS_DIR troca ~/Applications.
# Log: ~/Library/Logs/cba-install.log (macOS) ou ${XDG_STATE_HOME:-~/.local/state}/cba/install.log.
set -eu

REPO=deep-ion-ai/ctower-business-agents
APP_NAME="Ctower Business Agents"
DMG=cba-macos-universal.dmg
NODE_MAJOR=24

YES=${CBA_YES:-0}
VERSION=${CBA_VERSION:-}
FROM_SOURCE=${CBA_FROM_SOURCE:-0}
DRY_RUN=${CBA_DRY_RUN:-0}
BASE_URL=${CBA_BASE_URL:-https://github.com/$REPO/releases}
LOG=
TMP=
MOUNTED=

say() {
    printf '%s\n' "$*"
    if [ -n "$LOG" ]; then printf '%s %s\n' "$(date '+%Y-%m-%dT%H:%M:%S')" "$*" >>"$LOG"; fi
}
die() { say "ERRO: $*" >&2; exit 1; }

# Roda um comando mostrando a saída e copiando-a para o log; aborta se falhar.
run() {
    say "+ $*"
    { "$@" 2>&1; echo $? >"$TMP/rc"; } | tee -a "$LOG"
    [ "$(cat "$TMP/rc")" = 0 ] || die "falhou: $*"
}

fetch() { curl -fL --retry 3 -sS --proto '=https,file' -o "$2" "$1" || die "não consegui baixar $1"; }

cleanup() {
    if [ -n "$MOUNTED" ]; then hdiutil detach -quiet "$MOUNTED" || true; fi
    if [ -n "$TMP" ]; then rm -rf "$TMP"; fi
}

ask() {
    [ "$YES" = 1 ] && return 0
    (: </dev/tty) 2>/dev/null || die "sem terminal para confirmar. Rode de novo com --yes (curl ... | sh -s -- --yes)."
    printf '%s [s/N] ' "$1" >/dev/tty
    read -r resposta </dev/tty || resposta=
    case $resposta in s | S | sim | Sim | y | Y) return 0 ;; esac
    return 1
}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi |
        cut -d' ' -f1 | tr 'A-F' 'a-f'
}

# verify <arquivo> <nome no SHA256SUMS> <arquivo SHA256SUMS>
verify() {
    esperado=$(awk -v n="$2" '$2 == n || $2 == "*" n { print tolower($1); exit }' "$3")
    [ -n "$esperado" ] || die "o arquivo de hashes não lista $2. Instalação abortada; nada foi instalado."
    obtido=$(sha256 "$1")
    if [ "$esperado" != "$obtido" ]; then
        die "SHA-256 não confere para $2 (esperado $esperado, obtido $obtido). O arquivo pode estar corrompido ou adulterado. Instalação abortada; nada foi instalado."
    fi
    say "SHA-256 conferido: $2"
}

# A página /releases/latest redireciona para /releases/tag/vX.Y.Z.
latest_version() {
    url=$(curl -fsSIL -o /dev/null -w '%{url_effective}' "$BASE_URL/latest" 2>/dev/null) || return 0
    case $url in */tag/v*) printf '%s\n' "${url##*/tag/v}" ;; esac
}

installed_version() {
    if [ "$OS" = macos ]; then
        plist="$APPS_DIR/$APP_NAME.app/Contents/Info.plist"
        [ -f "$plist" ] || return 0
        sed -n '/CFBundleShortVersionString/{n;s/.*<string>\(.*\)<\/string>.*/\1/p;}' "$plist"
    elif [ -f "$STATE_DIR/installed-version" ] && [ -x "$HOME/.local/bin/cba" ]; then
        cat "$STATE_DIR/installed-version"
    fi
}

have_git() {
    if [ "$OS" = macos ]; then xcode-select -p >/dev/null 2>&1; else command -v git >/dev/null 2>&1; fi
}
have_claude_code() { command -v claude >/dev/null 2>&1 || [ -x "$HOME/.local/bin/claude" ]; }
have_claude_desktop() { [ -d /Applications/Claude.app ] || [ -d "$APPS_DIR/Claude.app" ]; }
have_cargo() { command -v cargo >/dev/null 2>&1 || [ -x "${CARGO_HOME:-$HOME/.cargo}/bin/cargo" ]; }
node_ok() { command -v node >/dev/null 2>&1 && [ "$(node -p 'process.versions.node.split(".")[0]')" -ge 20 ]; }
have_node() { node_ok 2>/dev/null || [ -x "$TOOLS_DIR/node/bin/node" ]; }

# --- Passos ---------------------------------------------------------------

install_git() {
    if [ "$OS" = macos ]; then
        say "Abrindo a janela da Apple para instalar as Command Line Tools (inclui o git)."
        xcode-select --install >>"$LOG" 2>&1 || true
        say "Conclua a instalação na janela do sistema."
        if [ "$FROM_SOURCE" = 1 ]; then die "a compilação precisa das Command Line Tools. Conclua a instalação e rode o script de novo."; fi
    else
        say "git não encontrado. Instale pelo gerenciador de pacotes (exige administrador), ex.: sudo apt install git"
        if [ "$FROM_SOURCE" = 1 ]; then die "a compilação precisa do git."; fi
    fi
}

download_app() {
    say "Baixando $DMG de $DL"
    fetch "$DL/SHA256SUMS" "$TMP/SHA256SUMS"
    fetch "$DL/$DMG" "$TMP/$DMG"
    verify "$TMP/$DMG" "$DMG" "$TMP/SHA256SUMS"
}

# copy_app <.app de origem>: substitui a cópia em ~/Applications.
copy_app() {
    mkdir -p "$APPS_DIR"
    INSTALLED_APP=$APPS_DIR/$(basename "$1")
    rm -rf "$INSTALLED_APP"
    run ditto "$1" "$INSTALLED_APP"
    say "App instalado em $INSTALLED_APP"
}

install_app_macos() {
    MOUNTED=$TMP/volume
    mkdir -p "$MOUNTED"
    run hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$MOUNTED" "$TMP/$DMG"
    app=$(find "$MOUNTED" -maxdepth 1 -name '*.app' -type d | head -n 1)
    [ -n "$app" ] || die "o DMG não contém um .app."
    copy_app "$app"
    run hdiutil detach -quiet "$MOUNTED"
    MOUNTED=
}

ensure_rust() {
    PATH=${CARGO_HOME:-$HOME/.cargo}/bin:$PATH
    command -v cargo >/dev/null 2>&1 && return 0
    say "Instalando o rustup na pasta do usuário (sem alterar o perfil do shell)."
    fetch https://sh.rustup.rs "$TMP/rustup-init.sh"
    run sh "$TMP/rustup-init.sh" -y --no-modify-path --profile minimal --default-toolchain none
}

# Node oficial (tarball de nodejs.org) em ~/.local/share/cba/tools/node, conferido pelo SHASUMS256.
ensure_node() {
    PATH=$TOOLS_DIR/node/bin:$PATH
    node_ok 2>/dev/null && return 0
    case $(uname -m) in arm64 | aarch64) arch=arm64 ;; x86_64 | amd64) arch=x64 ;; *) die "arquitetura sem Node oficial: $(uname -m)" ;; esac
    case $OS in macos) plat=darwin ;; *) plat=linux ;; esac
    dist=https://nodejs.org/dist/latest-v$NODE_MAJOR.x
    fetch "$dist/SHASUMS256.txt" "$TMP/node-sums"
    nome=$(awk -v s="-$plat-$arch.tar.gz" 'substr($2, length($2) - length(s) + 1) == s { print $2; exit }' "$TMP/node-sums")
    [ -n "$nome" ] || die "não achei o Node $NODE_MAJOR para $plat-$arch."
    say "Instalando $nome em $TOOLS_DIR/node."
    fetch "$dist/$nome" "$TMP/$nome"
    verify "$TMP/$nome" "$nome" "$TMP/node-sums"
    mkdir -p "$TMP/node" "$TOOLS_DIR"
    tar -xzf "$TMP/$nome" -C "$TMP/node"
    rm -rf "$TOOLS_DIR/node"
    mv "$TMP/node/${nome%.tar.gz}" "$TOOLS_DIR/node"
}

# Usa o diretório atual se já for o repositório; senão, clona em ~/.local/share/cba/src.
prepare_source() {
    if [ -f ui/package.json ] && git remote get-url origin 2>/dev/null | grep -q ctower-business-agents; then
        SRC=$(pwd)
        say "Usando o repositório atual: $SRC"
        return 0
    fi
    SRC=$TOOLS_DIR/src
    [ -d "$SRC/.git" ] || run git clone -q --depth 1 "https://github.com/$REPO.git" "$SRC"
    if [ -n "$VERSION" ]; then
        run git -C "$SRC" fetch -q --depth 1 origin tag "v$VERSION"
        run git -C "$SRC" checkout -q "v$VERSION"
    fi
}

# Supõe o app Tauri em ui/ com o script "tauri" no package.json e o executável "cba" (#3).
build_source() {
    if [ "$OS" = linux ] && ! pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
        die "faltam bibliotecas do sistema (exigem administrador). No Debian/Ubuntu: sudo apt install build-essential libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev"
    fi
    ensure_rust
    ensure_node
    (cd "$SRC" && run rustup toolchain install)
    if [ "$OS" = macos ]; then bundle="--bundles app"; else bundle="--no-bundle"; fi
    # shellcheck disable=SC2086 # $bundle tem duas palavras de propósito.
    (cd "$SRC/ui" && run npm ci && run npm run tauri build -- $bundle)
    if [ "$OS" = macos ]; then
        app=$(find "$SRC/target/release/bundle/macos" "$SRC/ui/src-tauri/target/release/bundle/macos" \
            -maxdepth 1 -name '*.app' -type d 2>/dev/null | head -n 1)
        [ -n "$app" ] || die "a compilação não gerou o .app."
        copy_app "$app"
    else
        bin=$(find "$SRC/target/release/cba" "$SRC/ui/src-tauri/target/release/cba" -type f 2>/dev/null | head -n 1)
        [ -n "$bin" ] || die "a compilação não gerou o executável cba."
        mkdir -p "$HOME/.local/bin"
        run cp "$bin" "$HOME/.local/bin/cba"
        printf '%s\n' "${VERSION:-desconhecida}" >"$STATE_DIR/installed-version"
        say "Instalado em ~/.local/bin/cba"
    fi
}

# Claude Code e Claude Desktop: só os fluxos oficiais da Anthropic.
# Enquanto a #43 (confirmação com a Anthropic) não for respondida, o script só MOSTRA o
# comando oficial. Se a resposta permitir, este é o lugar de perguntar (ask) e executar o
# instalador oficial sem modificação; até lá, nenhuma opção o executa.
show_claude() {
    if have_claude_code; then
        say "Claude Code: já instalado. Se ainda não fez login, rode 'claude' e use /login."
    else
        say "Claude Code: não encontrado. Para instalar, rode num terminal o instalador oficial da Anthropic:"
        say "    curl -fsSL https://claude.ai/install.sh | bash"
        say "  Depois rode 'claude' e faça login com /login. O login é seu: este script não toca em credenciais."
    fi
    if [ "$OS" = macos ] && ! have_claude_desktop; then
        say "Claude Desktop (opcional, para quem prefere não usar terminal): https://claude.ai/download"
    fi
}

# --- Principal ------------------------------------------------------------

while [ $# -gt 0 ]; do
    case $1 in
        -y | --yes) YES=1 ;;
        --version) [ $# -ge 2 ] || die "--version precisa de um valor"; VERSION=$2; shift ;;
        --version=*) VERSION=${1#*=} ;;
        --from-source) FROM_SOURCE=1 ;;
        --dry-run) DRY_RUN=1 ;;
        --base-url) [ $# -ge 2 ] || die "--base-url precisa de um valor"; BASE_URL=$2; shift ;;
        --base-url=*) BASE_URL=${1#*=} ;;
        -h | --help)
            echo "Uso: sh install.sh [--yes] [--version X.Y.Z] [--from-source] [--dry-run] [--base-url URL]"
            echo "Detalhes no cabeçalho do script."
            exit 0 ;;
        *) die "opção desconhecida: $1 (veja --help)" ;;
    esac
    shift
done
VERSION=${VERSION#v}

case ${CBA_PLATFORM:-$(uname -s)} in
    Darwin | macos) OS=macos ;;
    Linux | linux) OS=linux ;;
    *) die "sistema não suportado por este script: $(uname -s). No Windows, use o install.ps1." ;;
esac
command -v curl >/dev/null 2>&1 || die "o curl é necessário."

APPS_DIR=${CBA_APPS_DIR:-$HOME/Applications}
TOOLS_DIR=$HOME/.local/share/cba
if [ "$OS" = macos ]; then
    STATE_DIR=$HOME/Library/Logs
    LOG=$STATE_DIR/cba-install.log
else
    STATE_DIR=${XDG_STATE_HOME:-$HOME/.local/state}/cba
    LOG=$STATE_DIR/install.log
fi
mkdir -p "$STATE_DIR"
TMP=$(mktemp -d)
trap cleanup EXIT
trap 'exit 130' INT TERM
say "== Ctower Business Agents: instalação ($OS) =="

if [ -z "$VERSION" ]; then VERSION=$(latest_version); fi
if [ -n "$VERSION" ]; then DL=$BASE_URL/download/v$VERSION; else DL=$BASE_URL/latest/download; fi

if [ "$OS" = linux ] && [ "$FROM_SOURCE" != 1 ]; then
    say "Não há app publicado para Linux nesta fase. Dá para compilar do código-fonte com --from-source"
    say "(instala rustup e Node na pasta do usuário; as bibliotecas do sistema exigem administrador)."
    if [ "$DRY_RUN" != 1 ] && [ "$YES" != 1 ] && ask "Compilar do código-fonte agora?"; then FROM_SOURCE=1; fi
fi

ATUAL=$(installed_version)
NEED_APP=1
if [ -n "$ATUAL" ] && [ "$ATUAL" = "$VERSION" ]; then NEED_APP=0; fi
if [ "$OS" = linux ] && [ "$FROM_SOURCE" != 1 ]; then NEED_APP=0; fi
NEED_GIT=0
have_git || NEED_GIT=1

say ""
say "Plano:"
if [ "$OS" = linux ] && [ "$FROM_SOURCE" != 1 ]; then
    say "  - app: sem binário para Linux; nada a fazer (use --from-source)"
elif [ "$NEED_APP" = 0 ] && [ -n "$ATUAL" ]; then
    say "  - app: versão $ATUAL já instalada; nada a fazer"
elif [ "$NEED_APP" = 1 ] && [ "$FROM_SOURCE" = 1 ]; then
    have_cargo || say "  - instalar o rustup em ~/.cargo (sem alterar o perfil do shell)"
    have_node || say "  - instalar o Node $NODE_MAJOR em $TOOLS_DIR/node"
    say "  - compilar a versão ${VERSION:-mais recente} (npm ci && npm run tauri build) e instalar"
elif [ "$NEED_APP" = 1 ]; then
    say "  - app: instalar a versão ${VERSION:-mais recente (não consegui descobrir o número; reinstala)}${ATUAL:+ (instalada: $ATUAL)} em $APPS_DIR, conferindo o SHA-256"
fi
if [ "$NEED_GIT" = 1 ]; then say "  - git: instalar (no macOS, pelas Command Line Tools, numa janela do sistema)"; else say "  - git: já instalado"; fi
if have_claude_code; then say "  - Claude Code: já instalado"; else say "  - Claude Code: mostrar o comando oficial de instalação (não executa)"; fi
say "  Log: $LOG"
say ""

if [ "$DRY_RUN" = 1 ]; then
    if [ "$NEED_APP" = 1 ] && [ "$FROM_SOURCE" != 1 ]; then download_app; fi
    say "Modo de teste (--dry-run): nada foi instalado."
    exit 0
fi

ask "Continuar?" || die "cancelado; nada foi feito."

if [ "$NEED_GIT" = 1 ]; then install_git; fi
INSTALLED_APP=
if [ "$NEED_APP" = 1 ] && [ "$FROM_SOURCE" = 1 ]; then
    prepare_source
    build_source
elif [ "$NEED_APP" = 1 ]; then
    download_app
    install_app_macos
fi
say ""
show_claude
if [ "$OS" = macos ]; then
    open "${INSTALLED_APP:-$APPS_DIR/$APP_NAME.app}" || say "Abra o app em $APPS_DIR."
fi
say "Pronto."
