#!/usr/bin/env python3
"""Confere que o termo antigo "sistema" não voltou como nome da entidade cadastrada.

A entidade que o analista cadastra (pastas, base, regras, análises) se chama
**projeto** (#95). "Sistema" continua permitido no texto quando fala do software
("sistema legado", "sistema analisado", "pergunta ao sistema") ou do ambiente
("sistema operacional"). Por isso a verificação olha só o que é nome no código:

- identificador composto com o termo, em qualquer arquivo: `cadastrar_sistema`,
  `TelaSistema`, `CBA_SISTEMA`, `regras-do-sistema`;
- nos arquivos de código e de dados (.rs, .ts, .tsx, .json, .yaml, .toml), o
  termo isolado usado como nome: `let sistema`, `.sistema`, `sistema:`,
  `sistema(`, `sistema,` numa linha que não é comentário.

Uso: python3 scripts/check-termos.py [raiz]
Sai com 1 se encontrar o termo antigo, com arquivo, linha e trecho.
"""
import re
import subprocess
import sys
from pathlib import Path

# Pastas e arquivos verificados (relativos à raiz).
INCLUIR = re.compile(r"^(crates/|ui/src/|ui/src-tauri/src/|ui/src-tauri/tests/|fixtures/gabaritos/|scripts/)")
# Fora da verificação: este script e os testes dele citam o termo de propósito; a
# fixture é código de um sistema fictício.
EXCLUIR = re.compile(r"^(scripts/check-termos\.py|scripts/test_checks\.py|fixtures/faturamento-exemplo/)")
CODIGO = {".rs", ".ts", ".tsx", ".json", ".yaml", ".yml", ".toml"}

# Usos permitidos, retirados da linha antes de procurar.
PERMITIDOS = re.compile(
    r"subsistemas?|ecossistemas?",
    re.IGNORECASE,
)
# Emenda por `_`, `-` ou camelCase, antes ou depois do termo.
COMPOSTO = re.compile(
    r"[A-Za-z0-9][_-](?:sistema|Sistema|SISTEMA)|(?<!\\)[a-z0-9](?:Sistema|SISTEMA)"
    r"|(?:sistema|Sistema|SISTEMA)s?(?:[_-][A-Za-z0-9]|[A-Z])"
)
ISOLADO = re.compile(
    r"(?:\blet\s+(?:mut\s+)?|\bconst\s+|\bfn\s+|\.)sistemas?\b"
    r"|(?<![\"'\w] )(?<!\bao )(?<!\bdo )(?<!\bde )\bsistemas?\s*[:(,]"
)
COMENTARIO = re.compile(r"^\s*(//|#|\*|/\*)")


def ocorrencias(caminho, texto):
    codigo = Path(caminho).suffix in CODIGO
    for n, linha in enumerate(texto.splitlines(), 1):
        limpa = PERMITIDOS.sub("", linha)
        if COMPOSTO.search(limpa):
            yield n, linha.strip()
        elif codigo and not COMENTARIO.match(limpa) and ISOLADO.search(limpa):
            yield n, linha.strip()


def arquivos(raiz):
    saida = subprocess.run(["git", "ls-files"], cwd=raiz, capture_output=True, text=True, check=True)
    for nome in saida.stdout.splitlines():
        if INCLUIR.search(nome) and not EXCLUIR.search(nome):
            yield nome


def verificar(raiz):
    achados = []
    for nome in arquivos(raiz):
        try:
            texto = (Path(raiz) / nome).read_text(encoding="utf-8")
        except (UnicodeDecodeError, FileNotFoundError):
            continue
        for n, trecho in ocorrencias(nome, texto):
            achados.append(f"{nome}:{n}: {trecho[:160]}")
    return achados


def main(argv):
    raiz = argv[1] if len(argv) > 1 else "."
    achados = verificar(raiz)
    if achados:
        print('Termo antigo "sistema" como nome da entidade cadastrada; use "projeto" (#95):')
        for a in achados:
            print(f"  {a}")
        return 1
    print('Nenhum uso do termo antigo "sistema" como nome.')
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
