package exemplo.faturamento;

import java.math.BigDecimal;
import java.sql.CallableStatement;
import java.sql.Connection;
import java.sql.Date;
import java.sql.SQLException;
import java.sql.Types;
import java.time.LocalDate;

import javax.sql.DataSource;

/**
 * Calcula os encargos por atraso de uma fatura numa data de referência.
 *
 * <p>O cálculo base de multa e juros fica no banco (PKG_COBRANCA). Aqui
 * entram as exceções por tipo de cliente e o teto.</p>
 */
public class EncargosService {

    private final DataSource dataSource;

    public EncargosService(DataSource dataSource) {
        this.dataSource = dataSource;
    }

    public Encargos calcular(Fatura fatura, LocalDate dataReferencia) {
        BigDecimal multa = chamarFuncao("PKG_COBRANCA.FN_CALCULAR_MULTA", fatura.id(), dataReferencia);
        BigDecimal juros = chamarFuncao("PKG_COBRANCA.FN_CALCULAR_JUROS", fatura.id(), dataReferencia);

        // Órgão público não paga multa por atraso, apenas juros
        if (fatura.tipoCliente() == TipoCliente.PUB) {
            multa = BigDecimal.ZERO;
        }

        BigDecimal total = TetoEncargosRegra.limitar(
                multa.add(juros), fatura.valorOriginal(), fatura.tipoCliente());

        return new Encargos(multa, juros, total);
    }

    private BigDecimal chamarFuncao(String funcao, long idFatura, LocalDate dataReferencia) {
        String sql = "{ ? = call " + funcao + "(?, ?) }";
        try (Connection con = dataSource.getConnection();
             CallableStatement cs = con.prepareCall(sql)) {
            cs.registerOutParameter(1, Types.NUMERIC);
            cs.setLong(2, idFatura);
            cs.setDate(3, Date.valueOf(dataReferencia));
            cs.execute();
            return cs.getBigDecimal(1);
        } catch (SQLException e) {
            throw new IllegalStateException("Falha ao chamar " + funcao, e);
        }
    }

    /** Multa e juros calculados e o total já limitado pelo teto. */
    public record Encargos(BigDecimal multa, BigDecimal juros, BigDecimal totalComTeto) {
    }
}
