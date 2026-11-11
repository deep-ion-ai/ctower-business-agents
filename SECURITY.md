# Política de segurança

## Versões suportadas

O projeto está antes da `v1.0.0`. Só a última versão publicada nas [Releases](https://github.com/deep-ion-ai/ctower-business-agents/releases) recebe correções de segurança; a correção sai numa nova versão, não em versões anteriores.

| Versão | Suportada |
| --- | --- |
| Última release | Sim |
| Anteriores | Não |

## Como reportar uma vulnerabilidade

**Não abra uma issue pública, PR ou discussão** sobre uma vulnerabilidade. Use o relato privado do GitHub: aba **Security → Report a vulnerability** deste repositório (GitHub Security Advisories). O relato fica visível só para os mantenedores.

Inclua, se puder:

- a versão do app e o sistema operacional;
- o componente afetado (app, servidor MCP, hooks, instalador, base de conhecimento);
- os passos para reproduzir, de preferência com o sistema fictício `fixtures/faturamento-exemplo`, e o impacto esperado.

Não envie código, dados ou segredos de um sistema real no relato.

## Prazos

- **Confirmação do recebimento:** até 5 dias úteis.
- **Avaliação inicial** (se é vulnerabilidade, gravidade e plano): até 10 dias úteis.
- **Correção:** depende da gravidade; buscamos publicar falhas críticas e altas em até 30 dias. Mantemos você informado pelo próprio advisory.

Coordenamos a divulgação com você: o advisory é publicado depois da correção, com crédito a quem reportou, se você quiser.

## Pontos sensíveis

O app lê o código de sistemas legados e roda localmente, sem servidor nosso. Tratamos como prioridade relatos sobre:

- leitura ou envio de arquivos fora da pasta do sistema configurada;
- o agente conseguir alterar código, executar comandos ou publicar algo apesar das salvaguardas de "analisa, não desenvolve";
- vazamento de credenciais ou de conteúdo do código analisado para fora da máquina, além do provedor de IA escolhido pelo analista;
- comprometimento dos instaladores ou do script de instalação (integridade, assinatura, SHA-256).
