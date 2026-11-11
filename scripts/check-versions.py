#!/usr/bin/env python3
"""Confere se a versão do projeto é a mesma em todos os lugares onde ela vive.

A lista de arquivos de versão fica num lugar só: os `extra-files` do
release-please-config.json, os mesmos que o release-please atualiza. Arquivos da
lista que ainda não existem (ex.: ui/package.json antes do app) são ignorados.
Também confere o .release-please-manifest.json e, no Cargo.lock, a versão das
crates do workspace (o job lockfiles do release.yml as sincroniza).

Uso: python3 scripts/check-versions.py [raiz]
Sai com 1 se algo divergir.
"""
import json
import sys
import tomllib
from pathlib import Path


def ler(caminho):
    texto = caminho.read_text(encoding="utf-8")
    if caminho.suffix == ".toml" or caminho.name == "Cargo.lock":
        return tomllib.loads(texto)
    return json.loads(texto)


def pelo_jsonpath(dados, jsonpath):
    """Só o subconjunto usado na config: `$.a.b.c`."""
    if not jsonpath.startswith("$."):
        raise ValueError(f"jsonpath não suportado: {jsonpath}")
    for chave in jsonpath[2:].split("."):
        dados = dados[chave]
    return dados


def versoes(raiz):
    config = ler(raiz / "release-please-config.json")
    manifest = ler(raiz / ".release-please-manifest.json")
    encontradas = {".release-please-manifest.json": manifest["."]}
    for extra in config["packages"]["."].get("extra-files", []):
        caminho = raiz / extra["path"]
        if not caminho.exists():
            continue
        try:
            valor = pelo_jsonpath(ler(caminho), extra["jsonpath"])
        except KeyError:
            valor = None
        encontradas[f"{extra['path']} ({extra['jsonpath']})"] = valor

    # No Cargo.lock, os pacotes sem `source` são os nossos (path).
    lock = raiz / "Cargo.lock"
    if lock.exists():
        for pacote in ler(lock).get("package", []):
            if "source" not in pacote:
                encontradas[f"Cargo.lock ({pacote['name']})"] = pacote["version"]
    return encontradas


def main(argv):
    raiz = Path(argv[1]) if len(argv) > 1 else Path(__file__).resolve().parent.parent
    encontradas = versoes(raiz)
    esperada = encontradas[".release-please-manifest.json"]
    erradas = {k: v for k, v in encontradas.items() if v != esperada}
    if erradas:
        print(f"Versões divergentes (esperada {esperada}, do manifest):", file=sys.stderr)
        for k, v in erradas.items():
            print(f"  {k}: {v}", file=sys.stderr)
        print("Para o Cargo.lock: cargo update --workspace.", file=sys.stderr)
        return 1
    print(f"Versão {esperada} consistente em {len(encontradas)} lugares:")
    for k in encontradas:
        print(f"  {k}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
