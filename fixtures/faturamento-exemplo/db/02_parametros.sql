-- =====================================================================
-- Faturamento Exemplo
-- Script 02: carga inicial de parametros e clientes de exemplo
-- =====================================================================

INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
VALUES ('PERC_MULTA_ATRASO', 2.0000, 'Percentual de multa por atraso sobre o valor original');

INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
VALUES ('PERC_JUROS_DIA', 0.0330, 'Percentual de juros de mora por dia de atraso');

INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
VALUES ('DIAS_CARENCIA', 3.0000, 'Dias corridos apos o vencimento sem cobranca de encargos');

INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
VALUES ('MAX_PARCELAS', 6.0000, 'Quantidade maxima de parcelas no parcelamento de fatura');

INSERT INTO PARAMETRO (CHAVE, VALOR, DESCRICAO)
VALUES ('VALOR_MIN_PARCELA', 50.0000, 'Valor minimo de cada parcela, em reais');

-- Clientes de exemplo (dados ficticios; documentos propositalmente invalidos)
INSERT INTO CLIENTE (ID_CLIENTE, NOME, DOCUMENTO, TIPO_CLIENTE, ISENTO_ENCARGOS)
VALUES (1, 'Cliente Exemplo Pessoa Fisica', '000.000.000-00', 'PF', 'N');

INSERT INTO CLIENTE (ID_CLIENTE, NOME, DOCUMENTO, TIPO_CLIENTE, ISENTO_ENCARGOS)
VALUES (2, 'Comercio Ficticio Exemplo Ltda.', '00.000.000/0000-00', 'PJ', 'N');

INSERT INTO CLIENTE (ID_CLIENTE, NOME, DOCUMENTO, TIPO_CLIENTE, ISENTO_ENCARGOS)
VALUES (3, 'Prefeitura da Cidade Imaginaria', '11.111.111/1111-11', 'PUB', 'N');

INSERT INTO CLIENTE (ID_CLIENTE, NOME, DOCUMENTO, TIPO_CLIENTE, ISENTO_ENCARGOS)
VALUES (4, 'Associacao Beneficente Exemplo', '22.222.222/2222-22', 'PJ', 'S');

COMMIT;
