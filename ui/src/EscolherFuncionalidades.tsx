import { useMemo, useState } from "react";
import { DialogoConfirmacao } from "./DialogoConfirmacao";

export interface FuncionalidadeEscolhivel {
  id: string;
  nome: string;
  areaNome: string;
}

/**
 * Modal para escolher uma ou mais funcionalidades do mapa: busca por ID, nome ou
 * área e árvore por área (recolhível). As já escolhidas aparecem marcadas e
 * travadas; só as novas voltam em `aoAdicionar`.
 */
export function EscolherFuncionalidades({
  titulo,
  demanda,
  funcionalidades,
  jaEscolhidas,
  aoAdicionar,
  aoFechar,
}: {
  titulo: string;
  /** Título da demanda (issue) que recebe as funcionalidades. */
  demanda?: string;
  funcionalidades: FuncionalidadeEscolhivel[];
  jaEscolhidas: string[];
  aoAdicionar: (ids: string[]) => void;
  aoFechar: () => void;
}) {
  const [busca, setBusca] = useState("");
  const [marcadas, setMarcadas] = useState<string[]>([]);
  const q = busca.trim().toLowerCase();
  const areas = useMemo(() => {
    const por = new Map<string, FuncionalidadeEscolhivel[]>();
    for (const f of funcionalidades) {
      if (q && !`${f.id} ${f.nome} ${f.areaNome}`.toLowerCase().includes(q)) continue;
      por.set(f.areaNome, [...(por.get(f.areaNome) ?? []), f]);
    }
    return [...por.entries()];
  }, [funcionalidades, q]);
  const alternar = (id: string) =>
    setMarcadas((m) => (m.includes(id) ? m.filter((x) => x !== id) : [...m, id]));
  return (
    <DialogoConfirmacao
      titulo={titulo}
      aoCancelar={aoFechar}
      acoes={
        <>
          <button type="button" className="btn" onClick={aoFechar}>
            Cancelar
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={marcadas.length === 0}
            onClick={() => aoAdicionar(marcadas)}
          >
            {marcadas.length > 0 ? `Adicionar ${marcadas.length}` : "Adicionar"}
          </button>
        </>
      }
    >
      {demanda && (
        <p className="escolher-demanda">
          <strong>{demanda}</strong>
        </p>
      )}
      <input
        type="search"
        autoFocus
        aria-label="Buscar funcionalidade"
        placeholder="Buscar por ID, nome ou área"
        value={busca}
        onChange={(e) => setBusca(e.target.value)}
      />
      <div className="escolher-funcionalidades" role="tree" aria-label="Funcionalidades do mapa">
        {areas.length === 0 && <p className="muted">Nenhuma funcionalidade encontrada.</p>}
        {areas.map(([area, itens]) => (
          <details key={area} open={q !== "" || undefined} className="escolher-area">
            <summary>
              {area} <span className="muted">({itens.length})</span>
            </summary>
            <ul>
              {itens.map((f) => {
                const ja = jaEscolhidas.includes(f.id);
                return (
                  <li key={f.id}>
                    <label>
                      <input
                        type="checkbox"
                        checked={ja || marcadas.includes(f.id)}
                        disabled={ja}
                        onChange={() => alternar(f.id)}
                      />{" "}
                      {f.id} · {f.nome}
                      {ja && <span className="muted"> (já escolhida)</span>}
                    </label>
                  </li>
                );
              })}
            </ul>
          </details>
        ))}
      </div>
    </DialogoConfirmacao>
  );
}
