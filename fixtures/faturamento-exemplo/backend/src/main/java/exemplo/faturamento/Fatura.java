package exemplo.faturamento;

import java.math.BigDecimal;
import java.time.LocalDate;

/**
 * Visão da fatura usada pelos serviços de cobrança.
 */
public record Fatura(
        long id,
        long idCliente,
        TipoCliente tipoCliente,
        LocalDate dataEmissao,
        LocalDate dataVencimento,
        BigDecimal valorOriginal,
        BigDecimal valorPago,
        String status) {

    public boolean isAberta() {
        return "ABERTA".equals(status);
    }
}
