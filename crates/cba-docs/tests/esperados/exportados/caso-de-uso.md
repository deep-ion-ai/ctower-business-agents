# Caso de uso: Cancelamento de fatura

## Identificação

- **Área:** cancelamento
- **Tema:** cancelamento-de-fatura
- **Status:** rascunho
- **Origem:** não informada

## Objetivo

Cancelar uma fatura emitida, registrando motivo e responsável.

## Gatilho

O atendente pede o cancelamento de uma fatura emitida.

## Pré-condições

- **[confirmado]** Cancelamento só de fatura aberta e sem pagamento (RN-FAT-012): Só fatura com status ABERTA e sem nenhum valor pago pode ser cancelada. Fatura com pagamento parcial não pode ser cancelada.[^1][^2]

## Fluxo principal

1. O atendente informa a fatura e o motivo.
2. O sistema confere as regras.
3. A fatura é cancelada com o responsável registrado.

## Fluxos alternativos

- **[confirmado]** Cancelamento após 30 dias da emissão exige supervisor (RN-FAT-014): Fatura emitida há mais de 30 dias só pode ser cancelada por usuário com perfil SUPERVISOR. A regra existe apenas na tela de retaguarda; o banco não a verifica.[^3][^4]

## Exceções

- **[confirmado]** Motivo do cancelamento com ao menos 10 caracteres (RN-FAT-013): O cancelamento exige motivo com ao menos 10 caracteres (desprezando espaços nas pontas). O motivo, a data e o usuário ficam gravados na fatura e no log.[^5][^6][^7]

## Regras de negócio

- **[confirmado]** Cancelamento só de fatura aberta e sem pagamento (RN-FAT-012): Só fatura com status ABERTA e sem nenhum valor pago pode ser cancelada. Fatura com pagamento parcial não pode ser cancelada.[^1][^2]
- **[confirmado]** Motivo do cancelamento com ao menos 10 caracteres (RN-FAT-013): O cancelamento exige motivo com ao menos 10 caracteres (desprezando espaços nas pontas). O motivo, a data e o usuário ficam gravados na fatura e no log.[^5][^6][^7]
- **[confirmado]** Cancelamento após 30 dias da emissão exige supervisor (RN-FAT-014): Fatura emitida há mais de 30 dias só pode ser cancelada por usuário com perfil SUPERVISOR. A regra existe apenas na tela de retaguarda; o banco não a verifica.[^3][^4]

## Evidências

[^1]: `db/03_pkg_cobranca.sql:111-113`
[^2]: `db/03_pkg_cobranca.sql:115-117`
[^3]: `delphi/UFrmCancelamento.pas:69-75`
[^4]: `delphi/UFrmCancelamento.pas:41-42`
[^5]: `db/03_pkg_cobranca.sql:119-121`
[^6]: `db/03_pkg_cobranca.sql:123-131`
[^7]: `delphi/UFrmCancelamento.pas:63-67`

## Apêndice: trechos citados

### [1] `db/03_pkg_cobranca.sql:111-113`

```sql
111 |         IF V_STATUS <> 'ABERTA' THEN
112 |             RAISE_APPLICATION_ERROR(-20010, 'Somente fatura ABERTA pode ser cancelada');
113 |         END IF;
```

### [2] `db/03_pkg_cobranca.sql:115-117`

```sql
115 |         IF V_VL_PAGO > 0 THEN
116 |             RAISE_APPLICATION_ERROR(-20011, 'Fatura com pagamento parcial nao pode ser cancelada');
117 |         END IF;
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

### [4] `delphi/UFrmCancelamento.pas:41-42`

```pascal
41 |   DIAS_LIMITE_SEM_SUPERVISOR = 30;
42 |   PERFIL_SUPERVISOR = 'SUPERVISOR';
```

### [5] `db/03_pkg_cobranca.sql:119-121`

```sql
119 |         IF P_MOTIVO IS NULL OR LENGTH(TRIM(P_MOTIVO)) < 10 THEN
120 |             RAISE_APPLICATION_ERROR(-20012, 'Motivo do cancelamento deve ter ao menos 10 caracteres');
121 |         END IF;
```

### [6] `db/03_pkg_cobranca.sql:123-131`

```sql
123 |         UPDATE FATURA
124 |            SET STATUS          = 'CANCELADA',
125 |                MOTIVO_CANCEL   = TRIM(P_MOTIVO),
126 |                DT_CANCELAMENTO = SYSDATE,
127 |                USUARIO_CANCEL  = P_USUARIO
128 |          WHERE ID_FATURA = P_ID_FATURA;
129 |
130 |         INSERT INTO LOG_FATURA (ID_LOG, ID_FATURA, EVENTO, USUARIO, DETALHE)
131 |         VALUES (SQ_LOG_FATURA.NEXTVAL, P_ID_FATURA, 'CANCELAMENTO', P_USUARIO, TRIM(P_MOTIVO));
```

### [7] `delphi/UFrmCancelamento.pas:63-67`

```pascal
63 |   if Length(Trim(memMotivo.Text)) < TAM_MINIMO_MOTIVO then
64 |   begin
65 |     ShowMessage('Informe o motivo do cancelamento (minimo de 10 caracteres).');
66 |     Exit;
67 |   end;
```
