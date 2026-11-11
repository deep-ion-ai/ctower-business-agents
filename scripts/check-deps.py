#!/usr/bin/env python3
"""Confere que só o app (cba-app) depende do Tauri.

A spec (seção 5) diz que o núcleo não conhece a UI: nenhuma crate do workspace além
de cba-app pode depender, direta ou transitivamente, de um pacote `tauri*`. Todas
as dependências contam (normais, de build e de desenvolvimento) e para todas as
plataformas, como o `cargo metadata` as resolve.

Uso: python3 scripts/check-deps.py [arquivo.json]
Sem argumento, roda `cargo metadata --locked`; com um arquivo, lê a saída do
`cargo metadata` dele (usado nos testes). Sai com 1 se encontrar violação.
"""
import json
import subprocess
import sys

# Crates que podem depender do Tauri.
PERMITIDAS = {"cba-app"}
PROIBIDO = "tauri"


def violacoes(metadata):
    """Devolve {crate do workspace: [caminho até o pacote tauri*]} das que violam."""
    pacotes = {p["id"]: p["name"] for p in metadata["packages"]}
    deps = {n["id"]: [d["pkg"] for d in n["deps"]] for n in metadata["resolve"]["nodes"]}
    resultado = {}
    for membro in metadata["workspace_members"]:
        nome = pacotes[membro]
        if nome in PERMITIDAS:
            continue
        # Busca em largura guardando o caminho, para a mensagem dizer por onde o tauri entra.
        vistos = {membro}
        fila = [(membro, [nome])]
        while fila:
            atual, caminho = fila.pop(0)
            for dep in deps.get(atual, []):
                if dep in vistos:
                    continue
                vistos.add(dep)
                novo = caminho + [pacotes[dep]]
                if pacotes[dep].startswith(PROIBIDO):
                    resultado[nome] = novo
                    fila = []
                    break
                fila.append((dep, novo))
    return resultado


def main(argv):
    if len(argv) > 1:
        with open(argv[1], encoding="utf-8") as f:
            metadata = json.load(f)
    else:
        saida = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        metadata = json.loads(saida)
    erradas = violacoes(metadata)
    if erradas:
        print("Crates que não podem depender do Tauri e dependem:", file=sys.stderr)
        for nome, caminho in sorted(erradas.items()):
            print(f"  {' -> '.join(caminho)}", file=sys.stderr)
        return 1
    print(f"Nenhuma crate fora de {sorted(PERMITIDAS)} depende de {PROIBIDO}*.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
