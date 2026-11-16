import type { AbaProjeto, ItemSaude } from "./saudeProjeto";

const SIMBOLO = { ok: "✓", problema: "!", neutro: "·" } as const;

/**
 * Visão geral do projeto (#192): o checklist de saúde (código, demandas, o que o agente
 * lê, agente), cada item com a ação que leva à aba certa.
 */
export function AbaVisaoGeral({
  itens,
  aoIr,
}: {
  itens: ItemSaude[];
  aoIr: (aba: AbaProjeto) => void;
}) {
  return (
    <ul className="saude-projeto" aria-label="Saúde do projeto">
      {itens.map((i) => (
        <li key={i.id} className="saude-item" data-estado={i.estado}>
          <span className="saude-icone" aria-hidden="true">
            {SIMBOLO[i.estado]}
          </span>
          <div className="saude-texto">
            <strong>{i.titulo}</strong>
            <span className="muted">{i.texto}</span>
          </div>
          <button
            type="button"
            className={i.estado === "problema" ? "btn btn-sm btn-primary" : "btn btn-sm"}
            aria-label={`${i.acao}: ${i.titulo}`}
            onClick={() => aoIr(i.aba)}
          >
            {i.acao}
          </button>
        </li>
      ))}
    </ul>
  );
}
