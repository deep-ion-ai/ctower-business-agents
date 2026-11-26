import { useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, BarraAcoes, Secao } from "./componentes";
import type { ApiRepositorios } from "./repositorios";

type Api = Pick<ApiRepositorios, "pastaClones" | "definirPastaClones">;

/**
 * Clones (#179): a pasta onde o app clona os repositórios de código. Os tokens (PAT)
 * ficam na seção Acessos (`SecaoAcessos`, #193).
 */
export function SecaoClones({ api }: { api: Api }) {
  return (
    <Secao titulo="Clones">
      <PastaClones api={api} />
    </Secao>
  );
}

function PastaClones({ api }: { api: Api }) {
  const [pasta, setPasta] = useState("");
  const [gravada, setGravada] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [salvo, setSalvo] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    Promise.resolve()
      .then(() => api.pastaClones())
      .then((p) => {
        if (!ativo) return;
        setGravada(p ?? null);
        setPasta(p ?? "");
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api]);

  const salvar = () => {
    setOcupado(true);
    setFalha(null);
    setSalvo(false);
    api
      .definirPastaClones(pasta.trim())
      .then((p) => {
        setGravada(p);
        setPasta(p);
        setSalvo(true);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  return (
    <div role="group" aria-label="Pasta dos clones" className="secao-clones">
      <div className="field">
        <label className="campo-largo">
          Pasta dos clones
          <input
            type="text"
            className="caminho"
            value={pasta}
            placeholder="/Users/voce/Documents/ctower-business-repos"
            spellCheck={false}
            onChange={(e) => {
              setPasta(e.target.value);
              setSalvo(false);
            }}
          />
        </label>
        <p className="muted">
          Onde o app clona os repositórios de código, um por subpasta. Caminho completo.
        </p>
      </div>
      <BarraAcoes
        rotulo="Ações da pasta dos clones"
        secundarias={[
          {
            texto: "Salvar a pasta",
            aoEscolher: salvar,
            desabilitado: ocupado || !pasta.trim() || pasta.trim() === gravada,
          },
        ]}
      >
        <span role="status" aria-live="polite">
          {salvo && <span className="ok-text"> Salvo.</span>}
        </span>
      </BarraAcoes>
      {falha && <Alerta tom="erro" titulo={falha} />}
    </div>
  );
}
