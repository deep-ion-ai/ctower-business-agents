#!/usr/bin/env bash
# Spike da #352: workflows do Claude Code numa sessão como a da análise.
#
# Cria um projeto descartável com `.claude/workflows/teste-analise.js` (duas
# fases, um agente por arquivo, cada um chama uma ferramenta MCP), um servidor
# MCP mínimo que anota as chamadas num log e hooks que anotam o payload de cada
# evento. Roda o `claude -p` com Haiku, a configuração logada do analista e
# `--settings` / `--mcp-config` isolados (nada da configuração do analista é
# alterado). Custo típico: menos de US$ 0,02 por execução.
#
# Uso: [SO_INTERROMPER=1] scripts/spike-workflows.sh [--interromper]
#   --interromper  roda também um workflow com uma fase lenta, interrompe a
#                  sessão no meio e a retoma com `claude -p --resume`.
set -euo pipefail

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
P="$T/proj"
LOG="$T/log.txt"
mkdir -p "$P/.claude/workflows" "$T/bin"

cat >"$T/bin/mcp.py" <<'EOF'
import sys, json, os
LOG = os.environ["SPIKE_LOG"]
def out(m): sys.stdout.write(json.dumps(m) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    m = json.loads(line); i = m.get("id"); meth = m.get("method")
    if meth == "initialize":
        out({"jsonrpc": "2.0", "id": i, "result": {"protocolVersion": m["params"].get("protocolVersion", "2025-06-18"), "capabilities": {"tools": {}}, "serverInfo": {"name": "spk", "version": "0"}}})
    elif meth == "tools/list":
        out({"jsonrpc": "2.0", "id": i, "result": {"tools": [{"name": "registrar_achado", "description": "Registra um achado.", "inputSchema": {"type": "object", "properties": {"texto": {"type": "string"}}, "required": ["texto"]}}]}})
    elif meth == "tools/call":
        with open(LOG, "a") as f: f.write("MCP pid=%d %s\n" % (os.getpid(), json.dumps(m["params"]["arguments"], ensure_ascii=False)))
        out({"jsonrpc": "2.0", "id": i, "result": {"content": [{"type": "text", "text": "registrado"}]}})
    elif i is not None:
        out({"jsonrpc": "2.0", "id": i, "result": {}})
EOF

cat >"$T/bin/hook.sh" <<'EOF'
#!/bin/sh
{ printf 'HOOK %s ' "$1"; tr -d '\n'; echo; } >>"$SPIKE_LOG"; echo '{}'
EOF
chmod +x "$T/bin/hook.sh"

cat >"$P/.claude/workflows/teste-analise.js" <<'EOF'
export const meta = {
  name: 'teste-analise',
  description: 'Spike: duas fases, registra achados pelo MCP',
  phases: [{ title: 'Investigar' }, { title: 'Verificar' }],
}
const itens = (args && args.itens) || ['a.txt', 'b.txt']
phase('Investigar')
const achados = await parallel(itens.map(f => () => agent(
  `Leia o arquivo ${f} e chame a ferramenta MCP registrar_achado (servidor spk; carregue-a com ToolSearch) com texto = "achado de ${f}: " + a primeira linha do arquivo. Responda só com a primeira linha.`,
  { label: `investigar:${f}`, model: 'haiku', effort: 'low' })))
phase('Verificar')
const v = await agent(`Confira se estas linhas parecem frutas: ${JSON.stringify(achados)}. Responda sim ou não.`, { label: 'verificar', model: 'haiku', effort: 'low' })
return { achados, v }
EOF

cat >"$P/.claude/workflows/teste-lento.js" <<'EOF'
export const meta = {
  name: 'teste-lento',
  description: 'Spike: fase rápida e fase lenta, para interromper',
  phases: [{ title: 'Rapida' }, { title: 'Lenta' }],
}
phase('Rapida')
const r = await parallel(['a.txt', 'b.txt'].map(f => () => agent(
  `Chame a ferramenta MCP registrar_achado (servidor spk; carregue-a com ToolSearch) com texto = "rapida ${f}". Responda ok.`,
  { label: `rapida:${f}`, model: 'haiku', effort: 'low' })))
phase('Lenta')
const l = await agent('Rode no Bash `python3 -c \"import time; time.sleep(40)\"` em primeiro plano (sem run_in_background) e espere terminar. Depois chame a ferramenta MCP registrar_achado (servidor spk; carregue-a com ToolSearch) com texto = "lenta". Responda ok.', { label: 'lenta', model: 'haiku', effort: 'low' })
return { r, l }
EOF
echo "maçã" >"$P/a.txt"
echo "banana" >"$P/b.txt"
(cd "$P" && git init -q)

cat >"$T/mcp.json" <<EOF
{"mcpServers":{"spk":{"type":"stdio","command":"python3","args":["$T/bin/mcp.py"],"env":{"SPIKE_LOG":"$LOG"}}}}
EOF

configuracao() { # $1 = lista JSON de regras allow extras; grava $T/settings.json
  python3 - "$1" "$T" "$LOG" <<'EOF'
import json, sys
extras, t, log = json.loads(sys.argv[1]), sys.argv[2], sys.argv[3]
ev = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "SubagentStart", "SubagentStop", "Stop", "SessionEnd"]
h = {e: [{"hooks": [{"type": "command", "command": f"SPIKE_LOG={log} {t}/bin/hook.sh {e}", "timeout": 10}]}] for e in ev}
json.dump({"hooks": h, "permissions": {"allow": ["mcp__spk__registrar_achado", "Read", "Bash(python3 -c *)"] + extras}}, open(f"{t}/settings.json", "w"))
EOF
}

claude_p() { # $@ = opções extras; o pedido vem em $PEDIDO
  (cd "$P" && claude -p "$PEDIDO" --model haiku --settings "$T/settings.json" \
    --setting-sources project,local --mcp-config "$T/mcp.json" --strict-mcp-config \
    --max-budget-usd 1 "$@" </dev/null 2>&1)
}

contar() { echo "  chamadas MCP: $(grep -c '^MCP' "$LOG" 2>/dev/null || true)"; }

echo "Claude Code $(claude --version)"
LIVRE='Execute o workflow teste-analise com args {"itens":["a.txt","b.txt"]} e responda só com o resultado ou com o erro exato.'
NOMEADO='Chame a ferramenta Workflow com name teste-analise e args {"itens":["a.txt","b.txt"]} e responda só com o resultado ou com o erro exato.'

[[ -n "${SO_INTERROMPER:-}" ]] || for caso in \
  "LIVRE|[]" \
  "LIVRE|[\"Workflow(teste-analise)\"]" \
  "NOMEADO|[\"Workflow(teste-analise)\"]" \
  "LIVRE|[\"Workflow(teste-analise)\",\"Skill(teste-analise)\"]"; do
  pedido=${caso%%|*}; regras=${caso#*|}
  echo "== pedido $pedido, permissões extras: $regras"
  PEDIDO=${!pedido}
  configuracao "$regras"; rm -f "$LOG"
  claude_p | tail -2 | sed 's/^/  /'; contar
done

[[ -n "${SO_INTERROMPER:-}" ]] || echo "== eventos e ferramentas vistos pelos hooks (última execução)"
[[ -n "${SO_INTERROMPER:-}" ]] || grep '^HOOK' "$LOG" | sed 's/^HOOK [A-Za-z]* //' | python3 -c '
import sys, json
for l in sys.stdin:
    d = json.loads(l)
    extra = d.get("tool_name") or d.get("agent_type") or ""
    tr = d.get("tool_response")
    if isinstance(tr, dict) and tr.get("runId"): extra += " runId=" + tr["runId"]
    bt = d.get("background_tasks")
    if bt: extra += " background_tasks=" + json.dumps([(b["type"], b["status"]) for b in bt])
    print("  ", d["hook_event_name"], extra, ("agent_id=" + d["agent_id"]) if d.get("agent_id") else "")'

if [[ "${1:-}" == "--interromper" ]]; then
  configuracao '["Workflow(teste-lento)","Skill(teste-lento)"]'; rm -f "$LOG"
  SID=$(python3 -c 'import uuid; print(uuid.uuid4())')
  PEDIDO='Chame a ferramenta Workflow com name teste-lento e responda com o resultado.'
  echo "== interrupção no meio da fase lenta (sessão $SID)"
  claude_p --session-id "$SID" >/dev/null &
  for _ in $(seq 1 240); do grep -q '"tool_name":"Bash"' "$LOG" 2>/dev/null && break; sleep 0.5; done
  sleep 2; pkill -TERM -f "$SID" || true; sleep 3
  contar
  # O runId vem no PostToolUse da ferramenta Workflow: o receptor de hooks do app o vê.
  RUN=$(grep -o '"runId":"wf_[^"]*"' "$LOG" | head -1 | cut -d'"' -f4)
  echo "  runId visto pelo hook: $RUN"
  J=$(ls "$HOME"/.claude/projects/*/"$SID"/subagents/workflows/"$RUN"/journal.jsonl 2>/dev/null | head -1)
  [[ -n "$J" ]] && python3 -c 'import sys,json
for l in open(sys.argv[1]):
    d=json.loads(l); print("  journal:", d["type"], d.get("label",""), d.get("phase",""), str(d.get("result",""))[:80])' "$J"
  for modo in caminho nome; do
    rm -f "$LOG"
    if [[ $modo == caminho ]]; then
      PEDIDO='A sessão foi interrompida. Se o workflow teste-lento não estiver rodando, retome-o pela execução anterior (resumeFromRunId) e responda com o resultado.'
    else
      PEDIDO="A sessão foi interrompida. Chame a ferramenta Workflow com name teste-lento e resumeFromRunId $RUN, sem scriptPath, e responda com o resultado."
    fi
    echo "== retomada com --resume, pedido pelo $modo"
    claude_p --resume "$SID" | tail -12 | sed 's/^/  /'
    contar
    grep -o '"tool_name":"Workflow","tool_input":{[^}]*}' "$LOG" | head -1 | cut -c1-160 | sed 's/^/  /' || true
    grep -o '<usage>.*</usage>' "$LOG" | sed 's/^/  /' || true
  done
fi
