package exemplo.faturamento;

import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestHeader;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/**
 * Rotas da fatura usadas pelo portal de atendimento.
 */
@RestController
@RequestMapping("/api/faturas")
public class FaturaController {

    private final CancelamentoService cancelamentoService;

    public FaturaController(CancelamentoService cancelamentoService) {
        this.cancelamentoService = cancelamentoService;
    }

    @PostMapping("/{id}/cancelamento")
    public void cancelar(@PathVariable long id,
                         @RequestBody PedidoCancelamento pedido,
                         @RequestHeader("X-Usuario") String usuario) {
        cancelamentoService.cancelar(id, pedido.motivo(), usuario, true);
    }

    public record PedidoCancelamento(String motivo) {
    }
}
