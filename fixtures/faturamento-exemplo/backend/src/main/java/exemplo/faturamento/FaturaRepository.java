package exemplo.faturamento;

import java.math.BigDecimal;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.util.ArrayList;
import java.util.List;

import javax.sql.DataSource;

/**
 * Leitura e gravação da situação das faturas (tabela FATURA).
 */
public class FaturaRepository {

    private static final String SQL_SITUACAO =
            "SELECT STATUS, VL_PAGO FROM FATURA WHERE ID_FATURA = ?";

    private static final String SQL_ABERTAS_DO_CLIENTE =
            "SELECT ID_FATURA FROM FATURA WHERE ID_CLIENTE = ? AND STATUS = 'ABERTA'";

    private static final String SQL_CANCELAR =
            "UPDATE FATURA SET STATUS = 'CANCELADA', MOTIVO_CANCEL = ?, "
            + "DT_CANCELAMENTO = SYSDATE, USUARIO_CANCEL = ? WHERE ID_FATURA = ?";

    private final DataSource dataSource;

    public FaturaRepository(DataSource dataSource) {
        this.dataSource = dataSource;
    }

    public Situacao situacao(long idFatura) {
        try (Connection con = dataSource.getConnection();
             PreparedStatement ps = con.prepareStatement(SQL_SITUACAO)) {
            ps.setLong(1, idFatura);
            try (ResultSet rs = ps.executeQuery()) {
                if (!rs.next()) {
                    throw new IllegalStateException("Fatura não encontrada: " + idFatura);
                }
                return new Situacao(rs.getString(1), rs.getBigDecimal(2));
            }
        } catch (SQLException e) {
            throw new IllegalStateException("Falha ao ler a fatura " + idFatura, e);
        }
    }

    public List<Long> abertasDoCliente(long idCliente) {
        try (Connection con = dataSource.getConnection();
             PreparedStatement ps = con.prepareStatement(SQL_ABERTAS_DO_CLIENTE)) {
            ps.setLong(1, idCliente);
            List<Long> ids = new ArrayList<>();
            try (ResultSet rs = ps.executeQuery()) {
                while (rs.next()) {
                    ids.add(rs.getLong(1));
                }
            }
            return ids;
        } catch (SQLException e) {
            throw new IllegalStateException("Falha ao ler as faturas do cliente " + idCliente, e);
        }
    }

    public void gravarCancelamento(long idFatura, String motivo, String usuario) {
        try (Connection con = dataSource.getConnection();
             PreparedStatement ps = con.prepareStatement(SQL_CANCELAR)) {
            ps.setString(1, motivo);
            ps.setString(2, usuario);
            ps.setLong(3, idFatura);
            ps.executeUpdate();
        } catch (SQLException e) {
            throw new IllegalStateException("Falha ao gravar o cancelamento da fatura " + idFatura, e);
        }
    }

    public record Situacao(String status, BigDecimal valorPago) {
    }
}
