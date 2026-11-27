package exemplo.faturamento;

/**
 * Cancelamento de fatura.
 */
public class CancelamentoService {

    private static final int MIN_CARACTERES_MOTIVO = 10;

    private final FaturaRepository faturas;

    public CancelamentoService(FaturaRepository faturas) {
        this.faturas = faturas;
    }

    public void cancelar(long idFatura, String motivo, String usuario, boolean validar) {
        if (validar) {
            FaturaRepository.Situacao situacao = faturas.situacao(idFatura);
            if (!"ABERTA".equals(situacao.status())) {
                throw new IllegalStateException("Somente fatura ABERTA pode ser cancelada");
            }
            if (motivo == null || motivo.trim().length() < MIN_CARACTERES_MOTIVO) {
                throw new IllegalStateException("Motivo do cancelamento obrigatorio (minimo 10 caracteres)");
            }
        }
        faturas.gravarCancelamento(idFatura, motivo, usuario);
    }
}
