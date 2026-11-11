-- =====================================================================
-- Faturamento Exemplo
-- Script 03: pacote de cobranca (multa, juros e cancelamento)
-- =====================================================================

CREATE OR REPLACE PACKAGE PKG_COBRANCA AS

    FUNCTION FN_PARAMETRO (P_CHAVE IN VARCHAR2) RETURN NUMBER;

    FUNCTION FN_DIAS_ATRASO (P_DT_VENCIMENTO IN DATE,
                             P_DT_REFERENCIA IN DATE) RETURN NUMBER;

    FUNCTION FN_CALCULAR_MULTA (P_ID_FATURA     IN NUMBER,
                                P_DT_REFERENCIA IN DATE) RETURN NUMBER;

    FUNCTION FN_CALCULAR_JUROS (P_ID_FATURA     IN NUMBER,
                                P_DT_REFERENCIA IN DATE) RETURN NUMBER;

    PROCEDURE PRC_CANCELAR_FATURA (P_ID_FATURA IN NUMBER,
                                   P_MOTIVO    IN VARCHAR2,
                                   P_USUARIO   IN VARCHAR2);

END PKG_COBRANCA;
/

CREATE OR REPLACE PACKAGE BODY PKG_COBRANCA AS

    FUNCTION FN_PARAMETRO (P_CHAVE IN VARCHAR2) RETURN NUMBER IS
        V_VALOR PARAMETRO.VALOR%TYPE;
    BEGIN
        SELECT VALOR
          INTO V_VALOR
          FROM PARAMETRO
         WHERE CHAVE = P_CHAVE;
        RETURN V_VALOR;
    EXCEPTION
        WHEN NO_DATA_FOUND THEN
            RAISE_APPLICATION_ERROR(-20001, 'Parametro nao cadastrado: ' || P_CHAVE);
    END FN_PARAMETRO;

    FUNCTION FN_DIAS_ATRASO (P_DT_VENCIMENTO IN DATE,
                             P_DT_REFERENCIA IN DATE) RETURN NUMBER IS
    BEGIN
        IF TRUNC(P_DT_REFERENCIA) <= TRUNC(P_DT_VENCIMENTO) THEN
            RETURN 0;
        END IF;
        RETURN TRUNC(P_DT_REFERENCIA) - TRUNC(P_DT_VENCIMENTO);
    END FN_DIAS_ATRASO;

    FUNCTION FN_CALCULAR_MULTA (P_ID_FATURA     IN NUMBER,
                                P_DT_REFERENCIA IN DATE) RETURN NUMBER IS
        V_FATURA  FATURA%ROWTYPE;
        V_CLIENTE CLIENTE%ROWTYPE;
        V_DIAS    NUMBER;
    BEGIN
        SELECT * INTO V_FATURA FROM FATURA WHERE ID_FATURA = P_ID_FATURA;
        SELECT * INTO V_CLIENTE FROM CLIENTE WHERE ID_CLIENTE = V_FATURA.ID_CLIENTE;

        -- Cliente com isencao cadastrada nao paga multa nem juros
        IF V_CLIENTE.ISENTO_ENCARGOS = 'S' THEN
            RETURN 0;
        END IF;

        V_DIAS := FN_DIAS_ATRASO(V_FATURA.DT_VENCIMENTO, P_DT_REFERENCIA);

        -- Dentro da carencia nao ha multa
        IF V_DIAS <= FN_PARAMETRO('DIAS_CARENCIA') THEN
            RETURN 0;
        END IF;

        -- Multa unica, aplicada sobre o valor original da fatura
        RETURN ROUND(V_FATURA.VL_ORIGINAL * FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2);
    END FN_CALCULAR_MULTA;

    FUNCTION FN_CALCULAR_JUROS (P_ID_FATURA     IN NUMBER,
                                P_DT_REFERENCIA IN DATE) RETURN NUMBER IS
        V_FATURA  FATURA%ROWTYPE;
        V_CLIENTE CLIENTE%ROWTYPE;
        V_DIAS    NUMBER;
    BEGIN
        SELECT * INTO V_FATURA FROM FATURA WHERE ID_FATURA = P_ID_FATURA;
        SELECT * INTO V_CLIENTE FROM CLIENTE WHERE ID_CLIENTE = V_FATURA.ID_CLIENTE;

        IF V_CLIENTE.ISENTO_ENCARGOS = 'S' THEN
            RETURN 0;
        END IF;

        V_DIAS := FN_DIAS_ATRASO(V_FATURA.DT_VENCIMENTO, P_DT_REFERENCIA);

        IF V_DIAS <= FN_PARAMETRO('DIAS_CARENCIA') THEN
            RETURN 0;
        END IF;

        -- Passada a carencia, os juros contam desde o primeiro dia de atraso
        -- (juros simples sobre o valor original)
        RETURN ROUND(V_FATURA.VL_ORIGINAL * FN_PARAMETRO('PERC_JUROS_DIA') / 100 * V_DIAS, 2);
    END FN_CALCULAR_JUROS;

    PROCEDURE PRC_CANCELAR_FATURA (P_ID_FATURA IN NUMBER,
                                   P_MOTIVO    IN VARCHAR2,
                                   P_USUARIO   IN VARCHAR2) IS
        V_STATUS  FATURA.STATUS%TYPE;
        V_VL_PAGO FATURA.VL_PAGO%TYPE;
    BEGIN
        SELECT STATUS, VL_PAGO
          INTO V_STATUS, V_VL_PAGO
          FROM FATURA
         WHERE ID_FATURA = P_ID_FATURA
           FOR UPDATE;

        IF V_STATUS <> 'ABERTA' THEN
            RAISE_APPLICATION_ERROR(-20010, 'Somente fatura ABERTA pode ser cancelada');
        END IF;

        IF V_VL_PAGO > 0 THEN
            RAISE_APPLICATION_ERROR(-20011, 'Fatura com pagamento parcial nao pode ser cancelada');
        END IF;

        IF P_MOTIVO IS NULL OR LENGTH(TRIM(P_MOTIVO)) < 10 THEN
            RAISE_APPLICATION_ERROR(-20012, 'Motivo do cancelamento deve ter ao menos 10 caracteres');
        END IF;

        UPDATE FATURA
           SET STATUS          = 'CANCELADA',
               MOTIVO_CANCEL   = TRIM(P_MOTIVO),
               DT_CANCELAMENTO = SYSDATE,
               USUARIO_CANCEL  = P_USUARIO
         WHERE ID_FATURA = P_ID_FATURA;

        INSERT INTO LOG_FATURA (ID_LOG, ID_FATURA, EVENTO, USUARIO, DETALHE)
        VALUES (SQ_LOG_FATURA.NEXTVAL, P_ID_FATURA, 'CANCELAMENTO', P_USUARIO, TRIM(P_MOTIVO));
    END PRC_CANCELAR_FATURA;

END PKG_COBRANCA;
/
