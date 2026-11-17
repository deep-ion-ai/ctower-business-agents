-- =====================================================================
-- Faturamento Exemplo
-- Script 04: pacote de situacao da fatura (pagamento, parcelamento e
-- contestacao)
-- =====================================================================

-- Situacoes acrescentadas depois da carga inicial (script 01)
ALTER TABLE FATURA DROP CONSTRAINT CK_FATURA_STATUS;
ALTER TABLE FATURA ADD CONSTRAINT CK_FATURA_STATUS
    CHECK (STATUS IN ('ABERTA', 'PAGA', 'PARCELADA', 'CANCELADA', 'SUSPENSA', 'PROTESTADA'));

CREATE OR REPLACE PACKAGE PKG_SITUACAO AS

    PROCEDURE PRC_REGISTRAR_PAGAMENTO (P_ID_FATURA IN NUMBER,
                                       P_VALOR     IN NUMBER,
                                       P_USUARIO   IN VARCHAR2);

    PROCEDURE PRC_EFETIVAR_PARCELAMENTO (P_ID_FATURA IN NUMBER,
                                         P_USUARIO   IN VARCHAR2);

    PROCEDURE PRC_QUITAR_PARCELA (P_ID_FATURA  IN NUMBER,
                                  P_NR_PARCELA IN NUMBER,
                                  P_USUARIO    IN VARCHAR2);

    PROCEDURE PRC_SUSPENDER_COBRANCA (P_ID_FATURA IN NUMBER,
                                      P_MOTIVO    IN VARCHAR2,
                                      P_USUARIO   IN VARCHAR2);

END PKG_SITUACAO;
/

CREATE OR REPLACE PACKAGE BODY PKG_SITUACAO AS

    PROCEDURE PRC_LOG (P_ID_FATURA IN NUMBER,
                       P_EVENTO    IN VARCHAR2,
                       P_USUARIO   IN VARCHAR2,
                       P_DETALHE   IN VARCHAR2) IS
    BEGIN
        INSERT INTO LOG_FATURA (ID_LOG, ID_FATURA, EVENTO, USUARIO, DETALHE)
        VALUES (SQ_LOG_FATURA.NEXTVAL, P_ID_FATURA, P_EVENTO, P_USUARIO, P_DETALHE);
    END PRC_LOG;

    PROCEDURE PRC_REGISTRAR_PAGAMENTO (P_ID_FATURA IN NUMBER,
                                       P_VALOR     IN NUMBER,
                                       P_USUARIO   IN VARCHAR2) IS
        V_FATURA FATURA%ROWTYPE;
    BEGIN
        SELECT * INTO V_FATURA FROM FATURA WHERE ID_FATURA = P_ID_FATURA FOR UPDATE;

        IF V_FATURA.STATUS <> 'ABERTA' THEN
            RAISE_APPLICATION_ERROR(-20020, 'Pagamento so pode ser registrado em fatura ABERTA');
        END IF;

        UPDATE FATURA
           SET VL_PAGO = VL_PAGO + P_VALOR
         WHERE ID_FATURA = P_ID_FATURA;

        -- Pagamento parcial mantem a fatura aberta
        IF V_FATURA.VL_PAGO + P_VALOR >= V_FATURA.VL_ORIGINAL THEN
            UPDATE FATURA
               SET STATUS = 'PAGA'
             WHERE ID_FATURA = P_ID_FATURA;
        END IF;

        PRC_LOG(P_ID_FATURA, 'PAGAMENTO', P_USUARIO, TO_CHAR(P_VALOR));
    END PRC_REGISTRAR_PAGAMENTO;

    PROCEDURE PRC_EFETIVAR_PARCELAMENTO (P_ID_FATURA IN NUMBER,
                                         P_USUARIO   IN VARCHAR2) IS
        V_STATUS FATURA.STATUS%TYPE;
    BEGIN
        SELECT STATUS INTO V_STATUS FROM FATURA WHERE ID_FATURA = P_ID_FATURA FOR UPDATE;

        IF V_STATUS <> 'ABERTA' THEN
            RAISE_APPLICATION_ERROR(-20021, 'Somente fatura ABERTA pode ser parcelada');
        END IF;

        -- As parcelas ja foram gravadas pelo backend (ParcelamentoService)
        UPDATE FATURA
           SET STATUS = 'PARCELADA'
         WHERE ID_FATURA = P_ID_FATURA;

        PRC_LOG(P_ID_FATURA, 'PARCELAMENTO', P_USUARIO, NULL);
    END PRC_EFETIVAR_PARCELAMENTO;

    PROCEDURE PRC_QUITAR_PARCELA (P_ID_FATURA  IN NUMBER,
                                  P_NR_PARCELA IN NUMBER,
                                  P_USUARIO    IN VARCHAR2) IS
        V_ABERTAS NUMBER;
    BEGIN
        UPDATE PARCELA
           SET STATUS = 'PAGA'
         WHERE ID_FATURA = P_ID_FATURA
           AND NR_PARCELA = P_NR_PARCELA;

        SELECT COUNT(*)
          INTO V_ABERTAS
          FROM PARCELA
         WHERE ID_FATURA = P_ID_FATURA
           AND STATUS = 'ABERTA';

        -- Quitada a ultima parcela, a fatura parcelada fica paga
        IF V_ABERTAS = 0 THEN
            UPDATE FATURA
               SET STATUS = 'PAGA'
             WHERE ID_FATURA = P_ID_FATURA
               AND STATUS = 'PARCELADA';
        END IF;

        PRC_LOG(P_ID_FATURA, 'QUITACAO_PARCELA', P_USUARIO, TO_CHAR(P_NR_PARCELA));
    END PRC_QUITAR_PARCELA;

    PROCEDURE PRC_SUSPENDER_COBRANCA (P_ID_FATURA IN NUMBER,
                                      P_MOTIVO    IN VARCHAR2,
                                      P_USUARIO   IN VARCHAR2) IS
        V_STATUS FATURA.STATUS%TYPE;
    BEGIN
        SELECT STATUS INTO V_STATUS FROM FATURA WHERE ID_FATURA = P_ID_FATURA FOR UPDATE;

        -- Cliente contestou a fatura: a cobranca fica suspensa
        IF V_STATUS = 'ABERTA' THEN
            UPDATE FATURA
               SET STATUS = 'SUSPENSA'
             WHERE ID_FATURA = P_ID_FATURA;

            PRC_LOG(P_ID_FATURA, 'CONTESTACAO', P_USUARIO, P_MOTIVO);
        END IF;
    END PRC_SUSPENDER_COBRANCA;

END PKG_SITUACAO;
/
