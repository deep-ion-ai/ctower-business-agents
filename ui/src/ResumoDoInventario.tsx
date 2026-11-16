import { tamanhoLegivel, type ResumoInventario } from "./primeiroUso";

/** O que será analisado (por linguagem) e o que fica fora (por motivo e item a item). */
export function ResumoDoInventario({
  inventario,
  aoReincluir,
  ocupado = false,
}: {
  inventario: ResumoInventario;
  aoReincluir?: (caminho: string) => void;
  ocupado?: boolean;
}) {
  const omitidos = inventario.ignorados - inventario.itensIgnorados.length;
  return (
    <div className="resumo-inventario">
      <div role="group" aria-label="O que será analisado">
        <h3>
          Será analisado: {inventario.analisados} arquivos (
          {tamanhoLegivel(inventario.bytesAnalisados)})
        </h3>
        <table>
          <tbody>
            {inventario.porLinguagem.map((c) => (
              <tr key={c.chave}>
                <td>{c.rotulo}</td>
                <td className="numero">{c.quantidade}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div role="group" aria-label="O que fica fora">
        <h3>Fica fora: {inventario.ignorados} itens</h3>
        {inventario.porMotivo.length > 0 && (
          <table>
            <tbody>
              {inventario.porMotivo.map((c) => (
                <tr key={c.chave}>
                  <td>{c.rotulo}</td>
                  <td className="numero">{c.quantidade}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {inventario.itensIgnorados.length > 0 && (
          <details>
            <summary>Ver os itens que ficam fora</summary>
            <ul className="lista-ignorados">
              {inventario.itensIgnorados.map((i) => (
                <li key={i.caminho}>
                  <code>
                    {i.caminho}
                    {i.pasta ? "/" : ""}
                  </code>{" "}
                  <span className="muted">{i.rotulo}</span>
                  {aoReincluir && i.ajustavel && (
                    <>
                      {" "}
                      <button
                        className="btn"
                        type="button"
                        disabled={ocupado}
                        aria-label={`Incluir ${i.caminho} na análise`}
                        onClick={() => aoReincluir(i.caminho)}
                      >
                        Incluir
                      </button>
                    </>
                  )}
                </li>
              ))}
            </ul>
            {omitidos > 0 && <p className="muted">E mais {omitidos} itens.</p>}
          </details>
        )}
      </div>
    </div>
  );
}
