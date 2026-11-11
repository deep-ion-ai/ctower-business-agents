# ctower-business-agents

Assistente desktop para analistas de requisitos e de negócio entenderem sistemas legados **a partir do código**: responde perguntas, explica funcionalidades e analisa o impacto de demandas, com evidência citada para cada afirmação.

- Standalone: instala, aponta para a pasta do sistema e usa. Sem servidor e sem conta nossa.
- Usa a assinatura de IA que o analista já tem. Recomendado: Claude Code no terminal embutido, com conversa, documento e evidências numa janela só. Para quem não quer terminal: Claude Desktop.
- Analisa, não desenvolve: o agente é orientado a não alterar código nem publicar nada.
- Evidência para cada afirmação, validada pelo app, e base de conhecimento local do sistema.

Status: em desenvolvimento (M1, esqueleto). O app abre uma tela vazia; o servidor MCP (`cba mcp`) e o receptor de hooks (`cba hook <evento>`) respondem, ainda sem ferramentas. Veja a spec em [`docs/specs/2026-11-11-mvp-standalone.md`](docs/specs/2026-11-11-mvp-standalone.md) e o plano em [`docs/plans/2026-11-11-plano-mvp.md`](docs/plans/2026-11-11-plano-mvp.md).

## Desenvolvimento

Pré-requisitos: Rust (a versão do `rust-toolchain.toml` é instalada sozinha pelo `rustup`), Node 22 e as [dependências do Tauri 2](https://v2.tauri.app/start/prerequisites/) do seu sistema.

```sh
cd ui
npm ci
npm run tauri dev        # app com recarga da UI
npm run build            # constrói ui/dist, exigido para compilar o cba-app
cd ..
cargo test --workspace   # testes do Rust (depois do npm run build)
cargo run -p cba-app -- --version
```

O executável `cba` tem os modos: sem argumentos abre o app; `cba mcp` sobe o servidor MCP em stdio; `cba hook <evento>` lê o JSON do hook na entrada padrão. Dados do app (índice, configuração, logs) ficam no diretório de dados do usuário, em `ai.deep-ion.ctower-business-agents` (`CBA_DATA_DIR` troca o lugar).

Os instaladores de Windows e macOS saem a cada release; veja [`docs/versionamento.md`](docs/versionamento.md).

Versionamento: SemVer com commits convencionais e release-please, veja [`docs/versionamento.md`](docs/versionamento.md).

Nasce da experiência com o [ctower-agents](https://github.com/deep-ion-ai/ctower-agents).

Licença: [Apache-2.0](LICENSE).
