package exemplo.faturamento;

import java.math.BigDecimal;
import java.math.RoundingMode;
import java.time.LocalDate;
import java.util.ArrayList;
import java.util.List;

/**
 * Parcelamento de uma fatura em aberto.
 */
public class ParcelamentoService {

    private static final int MIN_PARCELAS = 2;
    private static final int DIAS_ENTRE_PARCELAS = 30;

    private final ParametroRepository parametros;
    private final EncargosService encargosService;

    public ParcelamentoService(ParametroRepository parametros, EncargosService encargosService) {
        this.parametros = parametros;
        this.encargosService = encargosService;
    }

    public List<Parcela> parcelar(Fatura fatura, int quantidade, LocalDate dataPedido) {
        if (!fatura.isAberta()) {
            throw new RegraNegocioException("Somente fatura ABERTA pode ser parcelada");
        }

        if (fatura.tipoCliente() == TipoCliente.PF) {
            throw new RegraNegocioException("Parcelamento disponível apenas para pessoa jurídica e órgão público");
        }

        int maxParcelas = parametros.inteiro(ParametroRepository.MAX_PARCELAS);
        if (quantidade < MIN_PARCELAS || quantidade > maxParcelas) {
            throw new RegraNegocioException(
                    "Quantidade de parcelas deve estar entre " + MIN_PARCELAS + " e " + maxParcelas);
        }

        // Valor parcelado = valor original + encargos na data do pedido
        EncargosService.Encargos encargos = encargosService.calcular(fatura, dataPedido);
        BigDecimal totalParcelado = fatura.valorOriginal().add(encargos.totalComTeto());

        BigDecimal valorParcela = totalParcelado
                .divide(BigDecimal.valueOf(quantidade), 2, RoundingMode.DOWN);

        // Órgão público não tem valor mínimo de parcela
        if (fatura.tipoCliente() != TipoCliente.PUB) {
            BigDecimal minimo = parametros.valor(ParametroRepository.VALOR_MIN_PARCELA);
            if (valorParcela.compareTo(minimo) < 0) {
                throw new RegraNegocioException("Valor da parcela abaixo do mínimo de " + minimo);
            }
        }

        // A diferença de centavos do arredondamento vai para a primeira parcela
        BigDecimal residuo = totalParcelado.subtract(valorParcela.multiply(BigDecimal.valueOf(quantidade)));

        List<Parcela> parcelas = new ArrayList<>();
        for (int i = 1; i <= quantidade; i++) {
            BigDecimal valor = (i == 1) ? valorParcela.add(residuo) : valorParcela;
            LocalDate vencimento = dataPedido.plusDays((long) DIAS_ENTRE_PARCELAS * i);
            parcelas.add(new Parcela(fatura.id(), i, vencimento, valor));
        }
        return parcelas;
    }

    public record Parcela(long idFatura, int numero, LocalDate vencimento, BigDecimal valor) {
    }

    public static class RegraNegocioException extends RuntimeException {
        public RegraNegocioException(String mensagem) {
            super(mensagem);
        }
    }
}
