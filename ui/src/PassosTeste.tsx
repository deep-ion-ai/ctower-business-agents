import type { IntegracaoProntidao, SituacaoPasso } from "./repositorios";

const SIMBOLO: Record<SituacaoPasso, string> = { ok: "✓", falhou: "✗", pulado: "–" };
const ROTULO: Record<SituacaoPasso, string> = { ok: "ok", falhou: "falhou", pulado: "pulado" };

/** Os passos de um teste de prontidão, cada um com o detalhe e, se falhou, a orientação. */
export function PassosTeste({ resultado }: { resultado: IntegracaoProntidao }) {
  return (
    <ul className="passos-teste" aria-label={resultado.titulo || "Resultado do teste"}>
      {resultado.passos.map((p) => (
        <li key={p.id} data-situacao={p.situacao}>
          <span className="simbolo" aria-hidden="true">
            {SIMBOLO[p.situacao]}
          </span>
          <span>
            {p.rotulo}
            <span className="sr-only"> ({ROTULO[p.situacao]})</span>
            {p.detalhe && <span className="muted">: {p.detalhe}</span>}
            {p.orientacao && <span className="dica orientacao">{p.orientacao}</span>}
          </span>
        </li>
      ))}
    </ul>
  );
}
