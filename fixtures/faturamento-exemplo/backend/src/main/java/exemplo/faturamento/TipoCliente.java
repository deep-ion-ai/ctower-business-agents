package exemplo.faturamento;

/**
 * Tipo de cliente, espelhando a coluna CLIENTE.TIPO_CLIENTE.
 */
public enum TipoCliente {
    /** Pessoa física. */
    PF,
    /** Pessoa jurídica. */
    PJ,
    /** Órgão público. */
    PUB;

    public static TipoCliente doCodigo(String codigo) {
        return TipoCliente.valueOf(codigo.trim().toUpperCase());
    }
}
