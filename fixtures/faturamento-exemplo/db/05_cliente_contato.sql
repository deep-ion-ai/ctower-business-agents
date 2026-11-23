-- =====================================================================
-- Faturamento Exemplo
-- Script 05: contato dos clientes, usado pelo atendimento
-- =====================================================================

CREATE TABLE CLIENTE_CONTATO (
    ID_CLIENTE        NUMBER(10)     NOT NULL,
    EMAIL             VARCHAR2(120),
    TELEFONE          VARCHAR2(20),
    LOGRADOURO        VARCHAR2(150),
    CEP               VARCHAR2(9),
    DT_NASCIMENTO     DATE,
    -- texto livre preenchido pelo atendimento
    OBS_ATENDIMENTO   VARCHAR2(400),
    -- S = cliente aceita receber avisos por e-mail
    ACEITA_AVISO      CHAR(1)        DEFAULT 'N' NOT NULL,
    DT_ATUALIZACAO    DATE           DEFAULT SYSDATE NOT NULL,
    CONSTRAINT PK_CLIENTE_CONTATO PRIMARY KEY (ID_CLIENTE),
    CONSTRAINT FK_CONTATO_CLIENTE FOREIGN KEY (ID_CLIENTE)
        REFERENCES CLIENTE (ID_CLIENTE),
    CONSTRAINT CK_CONTATO_ACEITA CHECK (ACEITA_AVISO IN ('S', 'N'))
);

-- Contato de exemplo (dados ficticios; dominio reservado exemplo.com)
INSERT INTO CLIENTE_CONTATO (ID_CLIENTE, EMAIL, TELEFONE, LOGRADOURO, CEP,
                             DT_NASCIMENTO, ACEITA_AVISO)
VALUES (1, 'cliente.pf@exemplo.com', '(00) 0000-0000', 'Rua Ficticia, 100',
        '00000-000', DATE '1980-01-01', 'N');

COMMIT;
