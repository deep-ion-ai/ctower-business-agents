      ******************************************************************
      * FATURAMENTO EXEMPLO - EMPRESA EXEMPLO DE SERVICOS (FICTICIA)
      * PROGRAMA : CALCOBR
      * OBJETIVO : LOTE MENSAL DE COBRANCA. LE AS FATURAS VENCIDAS,
      *            CALCULA MULTA E JUROS E GERA O ARQUIVO DE COBRANCA.
      ******************************************************************
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALCOBR.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT ARQ-FATURAS  ASSIGN TO FATVENC
                  ORGANIZATION IS LINE SEQUENTIAL.
           SELECT ARQ-COBRANCA ASSIGN TO COBRANCA
                  ORGANIZATION IS LINE SEQUENTIAL.

       DATA DIVISION.
       FILE SECTION.
       FD  ARQ-FATURAS.
       01  REG-FATURA.
           05 FAT-ID               PIC 9(12).
           05 FAT-TIPO-CLIENTE     PIC X(03).
           05 FAT-ISENTO           PIC X(01).
           05 FAT-VL-ORIGINAL      PIC 9(10)V99.
           05 FAT-DIAS-ATRASO      PIC 9(05).

       FD  ARQ-COBRANCA.
       01  REG-COBRANCA.
           05 COB-ID               PIC 9(12).
           05 COB-VL-ORIGINAL      PIC 9(10)V99.
           05 COB-VL-MULTA         PIC 9(10)V99.
           05 COB-VL-JUROS         PIC 9(10)V99.
           05 COB-VL-TOTAL         PIC 9(10)V99.

       WORKING-STORAGE SECTION.
       77  WS-FIM-ARQUIVO          PIC X(01)      VALUE 'N'.
       77  WS-PERC-MULTA           PIC 9V9999     VALUE 2.0000.
       77  WS-PERC-JUROS-DIA       PIC 9V9999     VALUE 0.0330.
       77  WS-DIAS-CARENCIA        PIC 9(03)      VALUE 3.
       77  WS-VALOR-MIN-COBRANCA   PIC 9(05)V99   VALUE 10.00.
       77  WS-MULTA                PIC 9(10)V99   VALUE ZEROS.
       77  WS-JUROS                PIC 9(10)V99   VALUE ZEROS.
       77  WS-TOTAL                PIC 9(10)V99   VALUE ZEROS.
       77  WS-QTD-LIDAS            PIC 9(07)      VALUE ZEROS.
       77  WS-QTD-GERADAS          PIC 9(07)      VALUE ZEROS.
       77  WS-QTD-IGNORADAS        PIC 9(07)      VALUE ZEROS.

       PROCEDURE DIVISION.
       0000-PRINCIPAL.
           OPEN INPUT  ARQ-FATURAS
                OUTPUT ARQ-COBRANCA
           PERFORM 1000-LER-FATURA
           PERFORM 2000-PROCESSAR-FATURA UNTIL WS-FIM-ARQUIVO = 'S'
           CLOSE ARQ-FATURAS ARQ-COBRANCA
           DISPLAY 'CALCOBR - LIDAS....: ' WS-QTD-LIDAS
           DISPLAY 'CALCOBR - GERADAS..: ' WS-QTD-GERADAS
           DISPLAY 'CALCOBR - IGNORADAS: ' WS-QTD-IGNORADAS
           STOP RUN.

       1000-LER-FATURA.
           READ ARQ-FATURAS
               AT END MOVE 'S' TO WS-FIM-ARQUIVO
               NOT AT END ADD 1 TO WS-QTD-LIDAS
           END-READ.

       2000-PROCESSAR-FATURA.
           MOVE ZEROS TO WS-MULTA WS-JUROS
           IF FAT-ISENTO NOT = 'S'
              AND FAT-DIAS-ATRASO > WS-DIAS-CARENCIA
               COMPUTE WS-MULTA ROUNDED =
                   FAT-VL-ORIGINAL * WS-PERC-MULTA / 100
               COMPUTE WS-JUROS ROUNDED =
                   FAT-VL-ORIGINAL * WS-PERC-JUROS-DIA / 100
                   * FAT-DIAS-ATRASO
           END-IF
           COMPUTE WS-TOTAL = FAT-VL-ORIGINAL + WS-MULTA + WS-JUROS
           IF WS-TOTAL < WS-VALOR-MIN-COBRANCA
               ADD 1 TO WS-QTD-IGNORADAS
           ELSE
               PERFORM 3000-GRAVAR-COBRANCA
           END-IF
           PERFORM 1000-LER-FATURA.

       3000-GRAVAR-COBRANCA.
           MOVE FAT-ID          TO COB-ID
           MOVE FAT-VL-ORIGINAL TO COB-VL-ORIGINAL
           MOVE WS-MULTA        TO COB-VL-MULTA
           MOVE WS-JUROS        TO COB-VL-JUROS
           MOVE WS-TOTAL        TO COB-VL-TOTAL
           WRITE REG-COBRANCA
           ADD 1 TO WS-QTD-GERADAS.
