package exemplo.faturamento;

import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.SQLException;

import javax.sql.DataSource;

/**
 * Manutenção do cadastro de clientes.
 */
public class ClienteService {

    private static final String SQL_ATUALIZAR =
            "UPDATE CLIENTE SET NOME = ?, TIPO_CLIENTE = ?, ISENTO_ENCARGOS = ?, SITUACAO = ? "
            + "WHERE ID_CLIENTE = ?";

    private static final String MOTIVO_DESLIGAMENTO = "Cliente desligado do cadastro";

    private final DataSource dataSource;
    private final FaturaRepository faturas;
    private final CancelamentoService cancelamentoService;

    public ClienteService(DataSource dataSource, FaturaRepository faturas,
                          CancelamentoService cancelamentoService) {
        this.dataSource = dataSource;
        this.faturas = faturas;
        this.cancelamentoService = cancelamentoService;
    }

    public void editar(Cliente cliente, String usuario) {
        try (Connection con = dataSource.getConnection();
             PreparedStatement ps = con.prepareStatement(SQL_ATUALIZAR)) {
            ps.setString(1, cliente.nome());
            ps.setString(2, cliente.tipo().name());
            ps.setString(3, cliente.isentoEncargos() ? "S" : "N");
            ps.setString(4, cliente.situacao());
            ps.setLong(5, cliente.id());
            ps.executeUpdate();
        } catch (SQLException e) {
            throw new IllegalStateException("Falha ao gravar o cliente " + cliente.id(), e);
        }

        // Cliente desligado: as faturas em aberto saem da cobrança
        if ("DESLIGADO".equals(cliente.situacao())) {
            for (long idFatura : faturas.abertasDoCliente(cliente.id())) {
                cancelamentoService.cancelar(idFatura, MOTIVO_DESLIGAMENTO, usuario, false);
            }
        }
    }

    public record Cliente(long id, String nome, TipoCliente tipo, boolean isentoEncargos,
                          String situacao) {
    }
}
