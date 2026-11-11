# ctower-business-agents

Assistente desktop para analistas de requisitos e de negócio entenderem sistemas legados **a partir do código**: responde perguntas, explica funcionalidades e analisa o impacto de demandas, com evidência citada para cada afirmação.

- Standalone: instala, aponta para a pasta do sistema e usa. Sem servidor e sem conta nossa.
- Usa a assinatura de IA que o analista já tem. Recomendado: Claude Code no terminal embutido, com conversa, documento e evidências numa janela só. Para quem não quer terminal: Claude Desktop.
- Analisa, não desenvolve: o agente é orientado a não alterar código nem publicar nada.
- Evidência para cada afirmação, validada pelo app, e base de conhecimento local do sistema.

## Instalação

macOS:

```sh
curl -fsSL https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://github.com/deep-ion-ai/ctower-business-agents/releases/latest/download/install.ps1 | iex
```

Instalam o app e o que faltar (git, WebView2), sem administrador, e mostram como instalar o Claude Code. Detalhes, opções e Linux (do código-fonte) em [`packaging/install`](packaging/install/README.md).

Status: em especificação. Veja [`docs/specs/2026-11-11-mvp-standalone.md`](docs/specs/2026-11-11-mvp-standalone.md).

Versionamento: SemVer com commits convencionais e release-please, veja [`docs/versionamento.md`](docs/versionamento.md).

Nasce da experiência com o [ctower-agents](https://github.com/deep-ion-ai/ctower-agents).

## Compilar localmente

Pré-requisitos por sistema e comandos em [Compilar localmente](CONTRIBUTING.md#compilar-localmente), no CONTRIBUTING.

## Contribuir

[CONTRIBUTING](CONTRIBUTING.md) · [Código de conduta](CODE_OF_CONDUCT.md) · [Segurança](SECURITY.md) · [CLA](docs/cla/CLA.md)

Licença: [Apache-2.0](LICENSE).
