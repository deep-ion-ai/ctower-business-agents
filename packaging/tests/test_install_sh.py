#!/usr/bin/env python3
"""Testes do packaging/install/install.sh sem release real.

Monta uma release falsa em disco, aponta o script para ela com CBA_BASE_URL=file://...
e roda em --dry-run, que baixa e confere o SHA-256 mas não instala nada.

Uso: python3 -m unittest discover -s packaging/tests -p 'test_*.py'
Shell: CBA_TEST_SH (padrão: sh).
"""
import hashlib
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "install" / "install.sh"
SHELL = os.environ.get("CBA_TEST_SH", "sh")
DMG = "cba-macos-universal.dmg"

PLIST = """<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>Ctower Business Agents</string>
  <key>CFBundleShortVersionString</key>
  <string>{versao}</string>
</dict>
</plist>
"""


class InstallShTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.raiz = Path(self._tmp.name)
        self.home = self.raiz / "home"
        self.home.mkdir()
        self.release = self.raiz / "releases"

    def tearDown(self):
        self._tmp.cleanup()

    def publicar(self, versao="0.1.0", conteudo=b"dmg falso", hash_publicado=None, listar=True):
        pasta = self.release / "download" / f"v{versao}"
        pasta.mkdir(parents=True)
        (pasta / DMG).write_bytes(conteudo)
        h = hash_publicado or hashlib.sha256(conteudo).hexdigest()
        linhas = [f"{'0' * 64}  cba-windows-x64-setup.exe"]
        if listar:
            linhas.append(f"{h}  {DMG}")
        (pasta / "SHA256SUMS").write_text("\n".join(linhas) + "\n")

    def instalar_app(self, versao):
        contents = self.home / "Applications" / "Ctower Business Agents.app" / "Contents"
        contents.mkdir(parents=True)
        (contents / "Info.plist").write_text(PLIST.format(versao=versao))

    def rodar(self, *args, plataforma="macos"):
        env = {
            "PATH": os.environ["PATH"],
            "HOME": str(self.home),
            "CBA_PLATFORM": plataforma,
            "CBA_BASE_URL": f"file://{self.release}",
        }
        # Sessão nova: sem /dev/tty, como num CI ou num `curl | sh` sem terminal.
        return subprocess.run(
            [SHELL, str(SCRIPT), *args],
            env=env,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
            start_new_session=True,
            timeout=60,
        )

    def test_hash_certo_passa(self):
        self.publicar()
        r = self.rodar("--dry-run", "--version", "0.1.0")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("SHA-256 conferido: " + DMG, r.stdout)
        self.assertIn("nada foi instalado", r.stdout)
        self.assertFalse((self.home / "Applications").exists())

    def test_hash_divergente_aborta(self):
        self.publicar(hash_publicado="ab" * 32)
        r = self.rodar("--dry-run", "--version", "0.1.0")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("SHA-256 não confere", r.stderr)
        self.assertIn("Instalação abortada", r.stderr)

    def test_asset_fora_do_sha256sums_aborta(self):
        self.publicar(listar=False)
        r = self.rodar("--dry-run", "--version", "v0.1.0")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("não lista " + DMG, r.stderr)

    def test_mesma_versao_nao_reinstala(self):
        # Sem release publicada: se tentasse baixar, falharia.
        self.instalar_app("0.1.0")
        r = self.rodar("--dry-run", "--version", "0.1.0")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("versão 0.1.0 já instalada", r.stdout)
        self.assertNotIn("Baixando", r.stdout)

    def test_versao_diferente_atualiza(self):
        self.instalar_app("0.0.9")
        self.publicar()
        r = self.rodar("--dry-run", "--version", "0.1.0")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("(instalada: 0.0.9)", r.stdout)
        self.assertIn("SHA-256 conferido", r.stdout)

    def test_log_em_arquivo(self):
        self.publicar()
        self.rodar("--dry-run", "--version", "0.1.0")
        log = self.home / "Library" / "Logs" / "cba-install.log"
        self.assertIn("SHA-256 conferido", log.read_text())

    def test_linux_sem_binario_oferece_from_source(self):
        r = self.rodar("--dry-run", plataforma="linux")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("--from-source", r.stdout)
        self.assertTrue((self.home / ".local" / "state" / "cba" / "install.log").exists())

    def test_sem_terminal_e_sem_yes_nao_faz_nada(self):
        r = self.rodar(plataforma="linux")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("--yes", r.stderr)

    def test_opcao_desconhecida(self):
        r = self.rodar("--nao-existe")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("opção desconhecida", r.stderr)


if __name__ == "__main__":
    unittest.main()
