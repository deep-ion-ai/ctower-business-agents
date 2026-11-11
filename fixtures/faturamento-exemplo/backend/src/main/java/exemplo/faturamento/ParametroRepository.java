package exemplo.faturamento;

import java.math.BigDecimal;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;

import javax.sql.DataSource;

/**
 * Lê os parâmetros de negócio da tabela PARAMETRO.
 */
public class ParametroRepository {

    public static final String MAX_PARCELAS = "MAX_PARCELAS";
    public static final String VALOR_MIN_PARCELA = "VALOR_MIN_PARCELA";

    private static final String SQL = "SELECT VALOR FROM PARAMETRO WHERE CHAVE = ?";

    private final DataSource dataSource;

    public ParametroRepository(DataSource dataSource) {
        this.dataSource = dataSource;
    }

    public BigDecimal valor(String chave) {
        try (Connection con = dataSource.getConnection();
             PreparedStatement ps = con.prepareStatement(SQL)) {
            ps.setString(1, chave);
            try (ResultSet rs = ps.executeQuery()) {
                if (!rs.next()) {
                    throw new IllegalStateException("Parâmetro não cadastrado: " + chave);
                }
                return rs.getBigDecimal(1);
            }
        } catch (SQLException e) {
            throw new IllegalStateException("Falha ao ler parâmetro " + chave, e);
        }
    }

    public int inteiro(String chave) {
        return valor(chave).intValueExact();
    }
}
