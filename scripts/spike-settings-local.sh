#!/bin/sh
# Spike da #303: onde o Claude Code lê o `.claude/settings.local.json` e como ele
# convive com o escopo de usuário. Relatório: docs/specs/2026-11-23-spike-settings-local.md.
#
# Uso: scripts/spike-settings-local.sh [--com-modelo]
#
# Roda tudo numa pasta temporária, com `CLAUDE_CONFIG_DIR` temporário: não toca no
# `~/.claude` nem no `~/.claude.json` do analista. Sem login nessa configuração, o
# `claude -p` termina com "Not logged in", mas os hooks `SessionStart` e
# `UserPromptSubmit` disparam antes disso, o que basta para ver quais arquivos foram
# lidos. Com `--com-modelo`, o item 6 (proteção) faz uma chamada curta ao Haiku com a
# configuração logada do analista, numa pasta descartável.
set -eu

T=$(cd "$(mktemp -d)" && pwd -P)
CFG=$T/cfg
LOG=$T/log.txt
mkdir -p "$CFG" "$T/bin"
trap 'rm -rf "$T"' EXIT

cat > "$T/bin/h" <<EOF
#!/bin/sh
printf '%s %s\n' "\$1" "\$2" >> "$LOG"
cat >/dev/null
echo '{}'
EOF
chmod +x "$T/bin/h"

# grava um settings.local.json com um hook SessionStart rotulado
local_com() { # <pasta> <rótulo>
  mkdir -p "$1/.claude"
  printf '{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"%s/bin/h %s SessionStart","timeout":10}]}]}}\n' "$T" "$2" > "$1/.claude/settings.local.json"
}
usuario_com() { # <json do settings.json do usuário>
  printf '%s\n' "$1" > "$CFG/settings.json"
}
sessao() { # <pasta> <título>
  rm -f "$LOG"
  (cd "$1" && CLAUDE_CONFIG_DIR=$CFG claude -p ok </dev/null >/dev/null 2>&1) || true
  printf '%s: ' "$2"
  if [ -f "$LOG" ]; then cut -d' ' -f1 "$LOG" | sort | tr '\n' ' '; else printf '(nenhum hook)'; fi
  echo
}
git_init() { (cd "$1" && git init -q && echo x > a.txt && git add a.txt && git -c user.email=s@s -c user.name=s commit -qm i); }

echo "Claude Code $(claude --version)"

echo "== 1. git"
mkdir -p "$T/r"; git_init "$T/r"; local_com "$T/r" raiz
(cd "$T/r" && printf 'git status: [%s]  check-ignore: %s\n' "$(git status --short)" "$(git check-ignore -v .claude/settings.local.json || echo 'não ignorado')")

echo "== 2. pasta dentro de repositório (lê a pasta da sessão e a raiz do repositório)"
mkdir -p "$T/r/modulo/sub"; local_com "$T/r/modulo" modulo
sessao "$T/r/modulo" "sessão em r/modulo"
sessao "$T/r/modulo/sub" "sessão em r/modulo/sub"
mkdir -p "$T/fora/modulo/sub"; local_com "$T/fora" fora; local_com "$T/fora/modulo" fora-modulo
sessao "$T/fora/modulo" "sem git, sessão em fora/modulo"
sessao "$T/fora/modulo/sub" "sem git, sessão em fora/modulo/sub"

echo "== 3. worktree (lê a pasta da sessão e a raiz do repositório principal)"
(cd "$T/r" && git worktree add -q "$T/wt" -b spike-wt)
sessao "$T/wt" "worktree sem arquivo próprio"
local_com "$T/wt" wt; mkdir -p "$T/wt/modulo"; local_com "$T/wt/modulo" wt-modulo
sessao "$T/wt" "worktree com arquivo próprio"
sessao "$T/wt/modulo" "subpasta da worktree"

echo "== 7. convivência com o escopo de usuário"
usuario_com "{\"hooks\":{\"SessionStart\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"$T/bin/h usuario SessionStart\",\"timeout\":10}]}]}}"
sessao "$T/fora/modulo" "hook do analista no usuário + local"
usuario_com "{\"hooks\":{\"SessionStart\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"$T/bin/h fora-modulo SessionStart\",\"timeout\":10}]}]}}"
sessao "$T/fora/modulo" "mesmo comando no usuário e no local"
usuario_com '{"disableAllHooks":true}'
sessao "$T/fora/modulo" "disableAllHooks no usuário"
rm -f "$CFG/settings.json"

echo "== 5. confiança (interativo)"
if command -v expect >/dev/null; then
  cat > "$T/it.exp" <<'EOF'
set timeout 15
cd [lindex $argv 0]
spawn env CLAUDE_CONFIG_DIR=[lindex $argv 1] claude
sleep 8
send "\x03"; sleep 1; send "\x03"
expect eof
EOF
  python3 - "$CFG/.claude.json" <<'EOF'
import json, sys
p = sys.argv[1]
try: d = json.load(open(p))
except Exception: d = {}
d["hasCompletedOnboarding"] = True; d["theme"] = "dark"
json.dump(d, open(p, "w"))
EOF
  rm -f "$LOG"; expect "$T/it.exp" "$T/fora/modulo" "$CFG" >/dev/null 2>&1 || true
  printf 'sem confiança: %s\n' "$( [ -f "$LOG" ] && cat "$LOG" | tr '\n' ' ' || echo '(nenhum hook: o diálogo de confiança aparece antes)')"
  python3 - "$CFG/.claude.json" "$T/fora/modulo" "$T/r" <<'EOF'
import json, sys
p = sys.argv[1]; d = json.load(open(p))
for chave in sys.argv[2:]:
    d.setdefault("projects", {}).setdefault(chave, {})["hasTrustDialogAccepted"] = True
json.dump(d, open(p, "w"))
EOF
  rm -f "$LOG"; expect "$T/it.exp" "$T/fora/modulo" "$CFG" >/dev/null 2>&1 || true
  printf 'com hasTrustDialogAccepted na pasta: %s\n' "$(cut -d' ' -f1 "$LOG" 2>/dev/null | tr '\n' ' ')"
  rm -f "$LOG"; expect "$T/it.exp" "$T/wt/modulo" "$CFG" >/dev/null 2>&1 || true
  printf 'worktree, com hasTrustDialogAccepted na raiz principal: %s\n' "$(cut -d' ' -f1 "$LOG" 2>/dev/null | tr '\n' ' ')"
else
  echo "(sem expect: pulado)"
fi

if [ "${1:-}" = "--com-modelo" ]; then
  echo "== 6. proteção (configuração logada do analista, Haiku)"
  P=$T/prot; mkdir -p "$P/.claude"; echo x > "$P/a.txt"
  printf '{"permissions":{"deny":["Edit(/%s/**)"]}}\n' "$P" > "$P/.claude/settings.local.json"
  printf '{"permissions":{"allow":["Edit","Write"]}}\n' > "$T/allow.json"
  (cd "$P" && claude -p --model haiku --permission-mode acceptEdits --settings "$T/allow.json" \
    --allowedTools Edit Write --output-format json \
    "Use a ferramenta Edit para trocar o conteúdo de a.txt de x para y. Se for negado, tente Write. Responda só NEGADO ou FEITO." </dev/null) |
    python3 -c 'import json,sys; d=json.load(sys.stdin); print("resposta:", d.get("result"), "| negadas:", [x["tool_name"] for x in d.get("permission_denials") or []])'
  printf 'a.txt depois: %s\n' "$(cat "$P/a.txt")"
fi
