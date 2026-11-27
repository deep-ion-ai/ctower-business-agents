-- =====================================================================
-- Faturamento Exemplo
-- Script 06: situacao do cliente no cadastro
-- =====================================================================

-- ATIVO = cliente em cobranca normal; DESLIGADO = saiu da carteira
ALTER TABLE CLIENTE ADD SITUACAO VARCHAR2(10) DEFAULT 'ATIVO' NOT NULL;
ALTER TABLE CLIENTE ADD CONSTRAINT CK_CLIENTE_SITUACAO
    CHECK (SITUACAO IN ('ATIVO', 'DESLIGADO'));
