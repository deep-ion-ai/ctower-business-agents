import { useCallback, useState } from "react";
import { mensagemDe } from "./analise";
import type { ApiRepositorios, Prontidao, SituacaoPasso } from "./repositorios";

const TOM: Record<SituacaoPasso, "done" | "error" | "attention"> = {
  ok: "done",
  falhou: "error",
  pulado: "attention",
};

const ROTULO: Record<SituacaoPasso, string> = {
  ok: "ok",
  falhou: "falhou",
  pulado: "não conferido",
};

/**
 * Prontidão das integrações do projeto, passo a passo, com a orientação de cada falha.
 * Não bloqueia nada: o app funciona sem as integrações.
 */
export function ProntidaoRepositorios({
  api,
  projeto,
}: {
  api: Pick<ApiRepositorios, "prontidaoRepositorios">;
  projeto: string;
}) {
  const [estado, setEstado] = useState<Prontidao | null>(null);
  const [conferindo, setConferindo] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const conferir = useCallback(() => {
    setConferindo(true);
    setFalha(null);
    api
      .prontidaoRepositorios(projeto)
      .then(setEstado)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setConferindo(false));
  }, [api, projeto]);

  return (
    <div role="group" aria-label="Prontidão dos repositórios">
      <h5>Prontidão</h5>
      <button className="btn btn-sm" type="button" disabled={conferindo} onClick={conferir}>
        {estado ? "Conferir de novo" : "Conferir"}
      </button>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {estado && estado.integracoes.length === 0 && (
        <p className="muted">Nenhum repositório cadastrado: nada a conferir.</p>
      )}
      {estado?.integracoes.map((i) => (
        <section key={i.titulo} aria-label={i.titulo}>
          <h6>{i.titulo}</h6>
          <ul className="passos-prontidao">
            {i.passos.map((p) => (
              <li key={p.id}>
                <span className="badge" data-tone={TOM[p.situacao]}>
                  {ROTULO[p.situacao]}
                </span>{" "}
                {p.rotulo}
                {p.detalhe && <span className="muted"> · {p.detalhe}</span>}
                {p.orientacao && p.situacao === "falhou" && (
                  <p className="muted" role="note">
                    {p.orientacao}
                  </p>
                )}
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
