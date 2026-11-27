package exemplo.faturamento;

import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestHeader;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/**
 * Rotas do cadastro de clientes usadas pela retaguarda.
 */
@RestController
@RequestMapping("/api/clientes")
public class ClienteController {

    private final ClienteService clienteService;

    public ClienteController(ClienteService clienteService) {
        this.clienteService = clienteService;
    }

    @PutMapping("/{id}")
    public void editar(@PathVariable long id,
                       @RequestBody ClienteService.Cliente cliente,
                       @RequestHeader("X-Usuario") String usuario) {
        if (cliente.id() != id) {
            throw new IllegalArgumentException("Id do cliente diferente da rota");
        }
        clienteService.editar(cliente, usuario);
    }
}
