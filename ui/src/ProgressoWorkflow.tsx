import type { Analise } from "./analise";
import { agentesDaFase, ROTULO_SITUACAO } from "./conducao";

interface Props {
  analise: Analise;
  /** Pedido para continuar pela condução simples; sem ele, o botão não aparece. */
  pedidoSemWorkflow?: string | null;
  aoContinuarSemWorkflow?: (pedido: string) => void;
}

function milhares(n: number): string {
  return n >= 1_000_000
    ? `${(n / 1_000_000).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} mi`
    : `${Math.round(n / 1000).toLocaleString("pt-BR")} mil`;
}

/**
 * Condução da análise pelo workflow no painel (#371): as fases com "n de m agentes",
 * a situação e o uso no fim, o aviso "Conduzida sem workflow" e, quando o workflow não
 * rodou ou parou, "Continuar sem workflow".
 */
export function ProgressoWorkflow({ analise, pedidoSemWorkflow, aoContinuarSemWorkflow }: Props) {
  const w = analise.workflow ?? null;
  const aviso = analise.aviso_conducao ?? null;
  if (!w && !aviso && analise.conducao !== "workflow") return null;
  const parou = w !== null && (w.situacao === "falhou" || w.situacao === "interrompido");
  const semIniciar = analise.conducao === "workflow" && w === null && aviso !== null;
  const oferecer = !!pedidoSemWorkflow && !!aoContinuarSemWorkflow && (parou || semIniciar);
  return (
    <section className="progresso-workflow" aria-label="Workflow da análise">
      {aviso && (
        <p className="aviso-conducao" role="status">
          Conduzida sem workflow: {aviso}
        </p>
      )}
      {w && (
        <>
          <p className="progresso-workflow-titulo">
            <strong>Análise aprofundada</strong>{" "}
            <span className="muted">
              (workflow {w.nome}
              {w.retomada ? ", retomado" : ""})
            </span>{" "}
            <span
              className="badge"
              data-tone={
                w.situacao === "concluido"
                  ? "done"
                  : w.situacao === "rodando"
                    ? "running"
                    : "attention"
              }
            >
              {ROTULO_SITUACAO[w.situacao]}
            </span>
          </p>
          {w.fases.length > 0 ? (
            <ol className="fases-workflow" aria-label="Fases do workflow">
              {w.fases.map((f) => (
                <li
                  key={f.titulo}
                  data-feita={f.iniciados > 0 && f.concluidos >= f.iniciados ? "" : undefined}
                >
                  <span>{f.titulo}</span>{" "}
                  <span className="muted">
                    {f.iniciados === 0 ? "aguardando" : agentesDaFase(f.iniciados, f.concluidos)}
                  </span>
                </li>
              ))}
            </ol>
          ) : (
            <p className="muted">{agentesDaFase(w.iniciados, w.concluidos)}</p>
          )}
          {w.uso && (
            <p className="muted">
              {w.uso.agentes} agentes, {milhares(w.uso.tokens)} tokens dos agentes, {Math.round(w.uso.duracao_ms / 60000)} min
              {w.uso.agentes_com_erro > 0 && `; ${w.uso.agentes_com_erro} com erro`}.
            </p>
          )}
        </>
      )}
      {analise.conducao === "workflow" && !w && !aviso && (
        <p className="muted">Análise aprofundada: o workflow começa quando a conversa abrir.</p>
      )}
      {oferecer && (
        <p>
          <button
            type="button"
            className="btn btn-sm"
            onClick={() => pedidoSemWorkflow && aoContinuarSemWorkflow?.(pedidoSemWorkflow)}
          >
            Continuar sem workflow
          </button>{" "}
          <span className="muted">A conversa segue pelo roteiro completo; o que já foi registrado fica.</span>
        </p>
      )}
    </section>
  );
}
