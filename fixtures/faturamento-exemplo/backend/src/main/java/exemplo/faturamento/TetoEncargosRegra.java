package exemplo.faturamento;

import java.math.BigDecimal;
import java.math.RoundingMode;

/**
 * Teto dos encargos por atraso (multa + juros), em percentual do valor
 * original da fatura, conforme o tipo de cliente.
 */
public final class TetoEncargosRegra {

    private TetoEncargosRegra() {
    }

    public static BigDecimal percentualTeto(TipoCliente tipo) {
        switch (tipo) {
            case PF:
                return new BigDecimal("10.00");
            case PJ:
                return new BigDecimal("20.00");
            case PUB:
                return new BigDecimal("5.00");
            default:
                throw new IllegalArgumentException("Tipo de cliente desconhecido: " + tipo);
        }
    }

    public static BigDecimal limitar(BigDecimal encargos, BigDecimal valorOriginal, TipoCliente tipo) {
        BigDecimal teto = valorOriginal
                .multiply(percentualTeto(tipo))
                .divide(new BigDecimal("100"), 2, RoundingMode.HALF_UP);
        return encargos.min(teto);
    }
}
