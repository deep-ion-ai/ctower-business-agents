# ctower-business-agents

Assistente desktop para analistas de requisitos e de negócio entenderem sistemas legados **a partir do código**: responde perguntas, explica funcionalidades e analisa o impacto de demandas, com evidência citada para cada afirmação.

- Standalone: instala, aponta para a pasta do sistema e usa. Sem servidor e sem conta nossa.
- Usa a assinatura de IA que o analista já tem. Recomendado: Claude Code no terminal embutido, com conversa, documento e evidências numa janela só. Para quem não quer terminal: Claude Desktop.
- Analisa, não desenvolve: o agente é orientado a não alterar código nem publicar nada.
- Evidência para cada afirmação, validada pelo app, e base de conhecimento local do sistema.

Status: em desenvolvimento (M1, esqueleto). O app abre uma tela vazia; o servidor MCP (`cba mcp`) e o receptor de hooks (`cba hook <evento>`) respondem, ainda sem ferramentas. Veja a spec em [`docs/specs/2026-11-11-mvp-standalone.md`](docs/specs/2026-11-11-mvp-standalone.md) e o plano em [`docs/plans/2026-11-11-plano-mvp.md`](docs/plans/2026-11-11-plano-mvp.md).

Instalação: cada [release](https://github.com/deep-ion-ai/ctower-business-agents/releases) traz o instalador do Windows x64 e o DMG universal do macOS, ainda sem assinatura; as notas da release explicam como instalar.

Versionamento: SemVer com commits convencionais e release-please, veja [`docs/versionamento.md`](docs/versionamento.md).

Nasce da experiência com o [ctower-agents](https://github.com/deep-ion-ai/ctower-agents).

## Compilar localmente

Pré-requisitos por sistema e comandos em [Compilar localmente](CONTRIBUTING.md#compilar-localmente), no CONTRIBUTING. Em resumo: `(cd ui && npm ci && npm run build)`, depois `cargo test --workspace`; `npm run tauri dev` em `ui/` abre o app em modo de desenvolvimento.

## Contribuir

[CONTRIBUTING](CONTRIBUTING.md) · [Código de conduta](CODE_OF_CONDUCT.md) · [Segurança](SECURITY.md) · [CLA](docs/cla/CLA.md)

Licença: [Apache-2.0](LICENSE).
