# A-0212 · Cenários de teste do cancelamento de fatura

Cenários de teste · Projeto `faturamento-exemplo` · Status: rascunho · Criados em 2026-11-14 · Analista: Ana Exemplo · Origem: A-0193 · Código: `main` @ `1a2b3c4d`

## Pedido

Cenários de teste das regras da análise A-0193 (Cancelamento de fatura).

## Tabelas de decisão

### TD-1 · Cancelamento de fatura emitida há mais de 30 dias

**Situação:** aceito

_Origem:_

- RN-FAT-014: Cancelamento após 30 dias da emissão exige supervisor

|  | R1 | R2 | R3 |
|---|---|---|---|
| **C1** fatura emitida há mais de 30 dias | S | S | N |
| **C2** usuário com perfil SUPERVISOR | S | N | — |
| **A1** segue para o cancelamento | X |  | X |
| **A2** recusa: "Solicite a um supervisor" |  | X |  |
| **Cenário** | CT-0021 | CT-0022 | CT-0023 |

**Onde o código testa cada condição:**

- **C1** fatura emitida há mais de 30 dias[^1]

- **C2** usuário com perfil SUPERVISOR[^2]

## Valores-limite e partições

| Cenário | Técnica | Grandeza | Limite no código | Valor testado | Resultado esperado | Evidência do limite |
|---|---|---|---|---|---|---|
| CT-0022 | valor-limite | dias desde a emissão da fatura | **30** | 31 dias | o cancelamento é recusado: "Solicite a um supervisor" | `delphi/UFrmCancelamento.pas:41` |
| CT-0023 | valor-limite | dias desde a emissão da fatura | **30** | 30 dias | a fatura é cancelada | `delphi/UFrmCancelamento.pas:41` |
| CT-0024 | valor-limite | tamanho do motivo, sem espaços nas pontas | **10** | 9 caracteres | o banco recusa com o erro -20012 | `db/03_pkg_cobranca.sql:119` |

## Cenários de teste

### CT-0021 · Supervisor cancela fatura de 31 dias

**Técnica:** tabela de decisão · **Situação:** aceito · **Coluna:** TD-1/R1

**Valor testado:** 31 dias, perfil SUPERVISOR

- **Dado** uma fatura ABERTA, sem pagamento, emitida há 31 dias
- **Quando** um usuário com perfil SUPERVISOR pede o cancelamento
- **Então** a fatura é cancelada

_Origem:_

- RN-FAT-014: Cancelamento após 30 dias da emissão exige supervisor[^3]

### CT-0022 · 31 dias sem supervisor é recusado

**Técnica:** valor-limite · **Situação:** sem decisão · **Coluna:** TD-1/R2

**Limite:** **30** (dias desde a emissão da fatura, lido do código)

**Valor testado:** 31 dias

- **Dado** uma fatura ABERTA, sem pagamento, emitida há 31 dias
- **Quando** um atendente sem perfil SUPERVISOR pede o cancelamento
- **Então** o cancelamento é recusado: "Solicite a um supervisor"

_Origem:_

- RN-FAT-014: Cancelamento após 30 dias da emissão exige supervisor[^4][^3]

### CT-0023 · 30 dias sem supervisor cancela

**Técnica:** valor-limite · **Situação:** sem decisão · **Coluna:** TD-1/R3

**Limite:** **30** (dias desde a emissão da fatura, lido do código)

**Valor testado:** 30 dias

- **Dado** uma fatura ABERTA, sem pagamento, emitida há 30 dias
- **Quando** um atendente sem perfil SUPERVISOR pede o cancelamento
- **Então** a fatura é cancelada

_Origem:_

- RN-FAT-014: Cancelamento após 30 dias da emissão exige supervisor[^4][^3]

### CT-0024 · Motivo com 9 caracteres é recusado

**Técnica:** valor-limite · **Situação:** sem decisão

_A confirmar: depende da pergunta A-0193/1._

**Limite:** **10** (tamanho do motivo, sem espaços nas pontas, lido do código)

**Valor testado:** 9 caracteres

- **Dado** uma fatura ABERTA, sem pagamento
- **Quando** o atendente cancela com o motivo "duplicada" (9 caracteres)
- **Então** o banco recusa com o erro -20012

_Origem:_

- RN-FAT-013: Motivo do cancelamento com ao menos 10 caracteres[^5][^6]

## Rastreabilidade

| Item | Técnica | Regras e afirmações | Evidências | História | Observação |
|---|---|---|---|---|---|
| TD-1 | tabela de decisão | RN-FAT-014 | — | — | |
| CT-0021 | tabela de decisão | RN-FAT-014 | `delphi/UFrmCancelamento.pas:69-75` | — |  |
| CT-0022 | valor-limite | RN-FAT-014 | `delphi/UFrmCancelamento.pas:41`, `delphi/UFrmCancelamento.pas:69-75` | — |  |
| CT-0023 | valor-limite | RN-FAT-014 | `delphi/UFrmCancelamento.pas:41`, `delphi/UFrmCancelamento.pas:69-75` | — |  |
| CT-0024 | valor-limite | RN-FAT-013 | `db/03_pkg_cobranca.sql:119`, `db/03_pkg_cobranca.sql:119-121` | — | a confirmar (A-0193/1) |

## Perguntas em aberto

_Nenhuma pergunta em aberto._

## Evidências

[^1]: `delphi/UFrmCancelamento.pas:70` · hash `5f2a9c1e`
[^2]: `delphi/UFrmCancelamento.pas:71` · hash `5f2a9c1e`
[^3]: `delphi/UFrmCancelamento.pas:69-75` · hash `7c3d0b2a`
[^4]: `delphi/UFrmCancelamento.pas:41` · hash `5f2a9c1e`
[^5]: `db/03_pkg_cobranca.sql:119` · hash `5f2a9c1e`
[^6]: `db/03_pkg_cobranca.sql:119-121` · hash `7c3d0b2a`

## Apêndice: trechos citados

### [1] `delphi/UFrmCancelamento.pas:70`

```pascal
70 |   if (DiasDesdeEmissao(AIdFatura) > DIAS_LIMITE_SEM_SUPERVISOR) and
```

### [2] `delphi/UFrmCancelamento.pas:71`

```pascal
71 |      (FPerfil <> PERFIL_SUPERVISOR) then
```

### [3] `delphi/UFrmCancelamento.pas:69-75`

```pascal
69 |   { Fatura emitida ha mais de 30 dias so pode ser cancelada por supervisor }
70 |   if (DiasDesdeEmissao(AIdFatura) > DIAS_LIMITE_SEM_SUPERVISOR) and
71 |      (FPerfil <> PERFIL_SUPERVISOR) then
72 |   begin
73 |     ShowMessage('Fatura emitida ha mais de 30 dias. Solicite a um supervisor.');
74 |     Exit;
75 |   end;
```

### [4] `delphi/UFrmCancelamento.pas:41`

```pascal
41 |   DIAS_LIMITE_SEM_SUPERVISOR = 30;
```

### [5] `db/03_pkg_cobranca.sql:119`

```sql
119 |         IF P_MOTIVO IS NULL OR LENGTH(TRIM(P_MOTIVO)) < 10 THEN
```

### [6] `db/03_pkg_cobranca.sql:119-121`

```sql
119 |         IF P_MOTIVO IS NULL OR LENGTH(TRIM(P_MOTIVO)) < 10 THEN
120 |             RAISE_APPLICATION_ERROR(-20012, 'Motivo do cancelamento deve ter ao menos 10 caracteres');
121 |         END IF;
```
