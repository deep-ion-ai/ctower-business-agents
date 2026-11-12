#!/usr/bin/env python3
"""Testes de check-deps.py e check-versions.py.

Uso: python3 -m unittest discover -s scripts -p 'test_*.py'
"""
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

AQUI = Path(__file__).resolve().parent


def carregar(arquivo):
    spec = importlib.util.spec_from_file_location(arquivo.replace("-", "_")[:-3], AQUI / arquivo)
    modulo = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(modulo)
    return modulo


check_deps = carregar("check-deps.py")
check_versions = carregar("check-versions.py")


def metadata(grafo, membros):
    """Monta uma saída mínima de `cargo metadata` a partir de {pacote: [deps]}."""
    nomes = set(grafo) | {d for ds in grafo.values() for d in ds}
    return {
        "packages": [{"id": f"{n}-id", "name": n} for n in sorted(nomes)],
        "workspace_members": [f"{m}-id" for m in membros],
        "resolve": {
            "nodes": [
                {"id": f"{n}-id", "deps": [{"pkg": f"{d}-id"} for d in grafo.get(n, [])]}
                for n in sorted(nomes)
            ]
        },
    }


class CheckDeps(unittest.TestCase):
    def test_workspace_sem_tauri_passa(self):
        m = metadata({"cba-core": ["serde"], "cba-kb": ["cba-core"]}, ["cba-core", "cba-kb"])
        self.assertEqual(check_deps.violacoes(m), {})

    def test_cba_app_pode_depender_do_tauri(self):
        m = metadata({"cba-app": ["tauri", "cba-core"], "cba-core": []}, ["cba-app", "cba-core"])
        self.assertEqual(check_deps.violacoes(m), {})

    def test_dependencia_direta_falha(self):
        m = metadata({"cba-core": ["tauri"]}, ["cba-core"])
        self.assertEqual(check_deps.violacoes(m), {"cba-core": ["cba-core", "tauri"]})

    def test_dependencia_transitiva_falha_com_caminho(self):
        m = metadata(
            {"cba-kb": ["cba-core"], "cba-core": ["algo"], "algo": ["tauri-utils"]},
            ["cba-kb", "cba-core"],
        )
        self.assertEqual(
            check_deps.violacoes(m),
            {
                "cba-kb": ["cba-kb", "cba-core", "algo", "tauri-utils"],
                "cba-core": ["cba-core", "algo", "tauri-utils"],
            },
        )

    def test_depender_do_app_falha(self):
        m = metadata({"cba-core": ["cba-app"], "cba-app": ["tauri"]}, ["cba-core", "cba-app"])
        self.assertIn("cba-core", check_deps.violacoes(m))

    def test_main_le_arquivo_e_sai_com_1(self):
        with tempfile.TemporaryDirectory() as d:
            arq = Path(d) / "m.json"
            arq.write_text(json.dumps(metadata({"cba-core": ["tauri"]}, ["cba-core"])))
            self.assertEqual(check_deps.main(["x", str(arq)]), 1)


class CheckVersions(unittest.TestCase):
    def raiz(self, d, versao_cargo="0.1.0", versao_lock="0.1.0", com_ui=False, versao_ui="0.1.0"):
        r = Path(d)
        (r / "release-please-config.json").write_text(
            (AQUI.parent / "release-please-config.json").read_text(encoding="utf-8")
        )
        (r / ".release-please-manifest.json").write_text('{".": "0.1.0"}')
        (r / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{versao_cargo}"\n')
        (r / "Cargo.lock").write_text(
            "version = 4\n\n"
            f'[[package]]\nname = "cba-core"\nversion = "{versao_lock}"\n\n'
            '[[package]]\nname = "serde"\nversion = "1.0.0"\n'
            'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
        )
        if com_ui:
            (r / "ui/src-tauri").mkdir(parents=True)
            (r / "ui/package.json").write_text(json.dumps({"version": versao_ui}))
            (r / "ui/src-tauri/tauri.conf.json").write_text(json.dumps({"version": "0.1.0"}))
        return r

    def test_consistente_passa(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(check_versions.main(["x", str(self.raiz(d))]), 0)

    def test_arquivos_ausentes_sao_ignorados(self):
        with tempfile.TemporaryDirectory() as d:
            v = check_versions.versoes(self.raiz(d))
            self.assertNotIn("ui/package.json ($.version)", v)

    def test_arquivos_da_ui_entram_quando_existem(self):
        with tempfile.TemporaryDirectory() as d:
            r = self.raiz(d, com_ui=True)
            self.assertEqual(check_versions.main(["x", str(r)]), 0)
            self.assertEqual(len(check_versions.versoes(r)), 5)

    def test_cargo_toml_divergente_falha(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(check_versions.main(["x", str(self.raiz(d, versao_cargo="0.2.0"))]), 1)

    def test_cargo_lock_desatualizado_falha(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(check_versions.main(["x", str(self.raiz(d, versao_lock="0.0.0"))]), 1)

    def test_ui_divergente_falha(self):
        with tempfile.TemporaryDirectory() as d:
            r = self.raiz(d, com_ui=True, versao_ui="9.9.9")
            self.assertEqual(check_versions.main(["x", str(r)]), 1)


if __name__ == "__main__":
    unittest.main()
