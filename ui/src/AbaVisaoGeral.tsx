import { ItemLista, ListaItens } from "./componentes";
import type { AbaProjeto, ItemSaude } from "./saudeProjeto";

const SIMBOLO = { ok: "✓", problema: "!", neutro: "·" } as const;

/**
 * Saúde do cadastro do projeto (#192; "Visão geral" até a #319): o checklist de saúde (código, demandas, o que o agente
 * lê, agente), cada item com a ação que leva à aba certa (ou a Acessos, quando o
 * conserto é o token global do host).
 */
export function AbaVisaoGeral({
  itens,
  aoIr,
  aoIrParaAcessos,
}: {
  itens: ItemSaude[];
  aoIr: (aba: AbaProjeto) => void;
  /** Leva a Configurações › Acessos (item cujo conserto é o token global do host). */
  aoIrParaAcessos: () => void;
}) {
  return (
    <ListaItens rotulo="Saúde do projeto">
      {itens.map((i) => (
        <ItemLista
          key={i.id}
          acoes={
            <button
              type="button"
              className={i.estado === "problema" ? "btn btn-sm btn-primary" : "btn btn-sm"}
              aria-label={`${i.acao}: ${i.titulo}`}
              onClick={() => (i.acessos ? aoIrParaAcessos() : aoIr(i.aba))}
            >
              {i.acao}
            </button>
          }
        >
          <div className="saude-item">
            <span className="saude-icone" data-estado={i.estado} aria-hidden="true">
              {SIMBOLO[i.estado]}
            </span>
            <div className="saude-texto">
              <strong>{i.titulo}</strong>
              <span className="muted">{i.texto}</span>
            </div>
          </div>
        </ItemLista>
      ))}
    </ListaItens>
  );
}
