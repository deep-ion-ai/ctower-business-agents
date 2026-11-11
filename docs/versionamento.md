# Versionamento

O projeto segue [SemVer](https://semver.org/lang/pt-BR/) desde o primeiro commit. Ninguém escolhe a versão à mão: ela sai dos commits, pelo [release-please](https://github.com/googleapis/release-please).

## Commits convencionais

Os commits e os títulos de PR seguem os [commits convencionais](https://www.conventionalcommits.org/pt-br/), em português:

```
feat(kb): grava o frontmatter das notas
fix(mcp): não perde a evidência quando o arquivo some
docs(spec): ...
```

| Tipo | Efeito na versão (antes da 1.0) |
|---|---|
| `feat` | menor: 0.1.0 → 0.2.0 |
| `fix`, `perf` | correção: 0.1.0 → 0.1.1 |
| `feat!` ou rodapé `BREAKING CHANGE:` | menor (`bump-minor-pre-major`); maior só depois da 1.0 |
| `docs`, `chore`, `refactor`, `test`, `build`, `ci`, `style` | nenhum |

O título do PR é conferido pelo workflow `pr-title`, porque é ele que vira a mensagem do commit no squash.

## Fluxo da release

1. O trabalho entra na `develop` e, quando é hora de publicar, a `develop` é mesclada na `main`.
2. A cada push na `main`, o workflow `release` abre ou atualiza o PR de release, com a versão nova e o `CHANGELOG.md`. O job `lockfiles` acerta o `Cargo.lock` nesse PR.
3. Mesclar o PR de release cria a tag `vX.Y.Z` e a GitHub Release em rascunho. Os jobs de build dos instaladores rodam no mesmo workflow, logo depois (tags criadas pelo `GITHUB_TOKEN` não disparam outros workflows).
4. O job `publicar` anexa à release os instaladores (`cba-windows-x64-setup.exe`, `cba-macos-universal.dmg`), os scripts de `packaging/install/` e o `SHA256SUMS`, acrescenta às notas as instruções de instalação (`packaging/notas-da-release.md`) e tira a release do rascunho. Assim, `releases/latest` só muda quando a release está completa.

Para testar os instaladores de uma branch sem publicar, dispare o workflow `release` manualmente (`gh workflow run release.yml --ref <branch>`): os instaladores ficam como artefatos do run.

## Primeira versão

A versão inicial é `0.0.0`, no manifest e no `Cargo.toml`. Com `bump-minor-pre-major`, o primeiro `feat` que chegar à `main` gera a `0.1.0`. Se a primeira release só tiver `fix`, ela seria `0.0.1`; para forçar a `0.1.0` nesse caso, use o rodapé `Release-As: 0.1.0` no commit.

## Onde a versão vive

- `.release-please-manifest.json`: a versão da última release, referência para o release-please.
- `Cargo.toml` (`[workspace.package] version`): herdada por todas as crates com `version.workspace = true`.
- `ui/package.json` e `ui/src-tauri/tauri.conf.json` (o `cba-app` herda a do workspace).
- `Cargo.lock` e `ui/package-lock.json`: lockfiles, sincronizados pelo job `lockfiles`.

A lista dos arquivos de versão fica num lugar só: os `extra-files` do `release-please-config.json`. Arquivo novo de versão entra ali. O `scripts/check-versions.py` lê essa lista, ignora os arquivos que ainda não existem e falha no CI se alguma versão divergir. Não edite as versões à mão.
