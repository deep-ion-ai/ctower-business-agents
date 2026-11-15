# Projeto de referência para testes de uso

Para testar o fluxo de uso do app num código real, fora do **Faturamento Exemplo**, usamos o OpenCBS-Cloud como projeto de referência. Ele serve só às validações durante o desenvolvimento, não ao piloto, e não substitui o gabarito da [avaliação](avaliacao.md).

## O fork

| | |
|---|---|
| Origem | [OpenCBS/OpenCBS-Cloud](https://github.com/OpenCBS/OpenCBS-Cloud) |
| Fork usado | [deep-ion-ai/OpenCBS-Cloud](https://github.com/deep-ion-ai/OpenCBS-Cloud) |
| Commit base | `ab200dde82bd9b87905bf24399240b0bbe83d2a7` (27/01/2023) |
| Licença | GPL-3 |
| Tamanho | cerca de 68 mil linhas de Java e 9,7 mil de SQL |

É um sistema de microcrédito e core bancário em Java com SQL. **O código não é copiado para este repositório.** Quem testa cadastra o fork como projeto no app, apontando para o commit base.

Os caminhos abaixo valem no commit base e foram conferidos nele.

## Mapa de módulos úteis

Raiz: `server/`. Os pacotes Java ficam em `src/main/java/com/opencbs/<módulo>/`.

| Tema | Onde olhar |
|---|---|
| Empréstimos (visão geral) | `opencbs-loans/.../services/LoanService.java`, `controllers/LoanController.java` |
| Pedido de crédito e comitê | `opencbs-loans/.../services/LoanApplicationService.java`, `services/creditcommitteeservices/` (`CreditCommitteeService`, `CreditCommitteeAmountRangeService`, `CreditCommitteeVoteHistoryService`), `controllers/loanapplications/` |
| Apropriação de juros | `opencbs-loans/.../services/LoanInstallmentInterestAccrualService.java`, `LoanEventInterestAccrualService.java` |
| Multas (penalidades) | `opencbs-loans/.../services/LoanPenaltyAccountService.java`, `LoanPenaltyEventService.java`; no core, `services/PenaltyService.java` e `domain/Penalty.java` |
| Quitação antecipada | `opencbs-loans/.../services/earlyfee/` (`EarlyFeeRepaymentService`, `impl/EarlyFeeRepaymentServiceImpl`); `controllers/LoanRepaymentController.java` |
| Reescalonamento | `opencbs-loans/.../controllers/LoanRescheduleController.java` |
| Cronograma de parcelas | `opencbs-core/.../services/schedulegenerators/` (anuidade, diferencial, principal fixo, flat), `domain/schedule/` |
| Tarifa de abertura | `opencbs-core/.../services/EntryFeeService.java`, `services/entryfeecalculation/`, `domain/EntryFee.java` |
| Fechamento do dia | `opencbs-core/.../dayclosure/` (`DayClosureProcessWorker`, `DayClosureValidator`), `services/impl/DayClosureServiceImpl.java`, `services/operationdayservices/DayClosureProcessor.java` |
| Poupança | `opencbs-savings/.../services/SavingService.java`, `controllers/SavingController.java` |
| Migrations SQL | `opencbs-core/src/main/resources/db/migration/core` (328 scripts), `opencbs-loans/.../db/migration/loans` (22), `opencbs-savings/.../db/migration/savings` (12); também existem `opencbs-bonds`, `opencbs-borrowings` e `opencbs-term-deposits` |

Os módulos `opencbs-core`, `opencbs-loans` e `opencbs-savings` são os que interessam. Multas, cronograma, tarifa de abertura e fechamento do dia têm a parte genérica no core e a parte de empréstimo no loans.

## Especificações do Open Finance Brasil

Fonte de demandas de evolução negocial para a análise de impacto. Organização: [OpenBanking-Brasil](https://github.com/OpenBanking-Brasil).

| Repositório | Exemplo de demanda |
|---|---|
| [api-loans](https://github.com/OpenBanking-Brasil/api-loans) | Expor contratos de empréstimo, parcelas e encargos pela API de dados. Impacto em cronograma, juros e multas. |
| [api-credit-portability](https://github.com/OpenBanking-Brasil/api-credit-portability) | Aceitar a portabilidade de crédito: calcular o saldo devedor para a quitação na instituição de origem. Impacto em quitação antecipada e tarifas. |
| [api-consents](https://github.com/OpenBanking-Brasil/api-consents) | Exigir o consentimento do cliente antes de compartilhar os dados dele, com prazo, revogação e trilha. Impacto em pessoas, perfis e permissões. |

## Orientação para os agentes

Ao terminar uma issue que muda um fluxo ou uma tela, sugira no PR **um roteiro de teste de uso** no OpenCBS. Escolha um dos três tipos, o que mais se aproxima da mudança:

- **Pergunta** sobre o código. Exemplo: "como o sistema calcula a multa de uma parcela em atraso?".
- **Funcionalidade a explicar.** Exemplo: "explique a quitação antecipada de um empréstimo".
- **Demanda de evolução**, tirada das especificações acima. Exemplo: "o Open Finance exige expor as parcelas do contrato; qual o impacto?".

Escreva o roteiro em poucas linhas, com o tipo, o texto da pergunta ou da demanda e o módulo do mapa onde a resposta deve estar. Não invente resultado esperado: não há gabarito para esta referência.
