import { useCallback, useEffect, useState, type FormEvent } from "react";
import { mensagemDe, type AmbienteClaude, type AnaliseCriada } from "./analise";
import { ROTULO_FLUXO, type ApiConhecimento } from "./conhecimento";
import { apiFuncionalidadesTauri, type ApiFuncionalidades } from "./funcionalidades";
import { PrepararAmbiente } from "./PrepararAmbiente";
import type { ProjetoResumo } from "./terminal";

/**
 * Formulário do fluxo "Mapear funcionalidades" (#208): o projeto e, opcionais, a
 * pasta ou a área a mapear e uma orientação ao agente. Sem pasta nem área, o
 * projeto inteiro. Projetos grandes podem ser mapeados por partes, em várias
 * análises.
 */
export function NovoMapeamento({
  api,
  apiFuncionalidades = apiFuncionalidadesTauri,
  projetos,
  aoCriar,
  aoCancelar,
}: {
  api: ApiConhecimento;
  apiFuncionalidades?: ApiFuncionalidades;
  projetos: ProjetoResumo[];
  aoCriar: (projeto: string, criada: AnaliseCriada) => void;
  aoCancelar: () => void;
}) {
  const [projeto, setProjeto] = useState(projetos[0]?.id ?? "");
  const [pasta, setPasta] = useState("");
  const [area, setArea] = useState("");
  const [entrada, setEntrada] = useState("");
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [criando, setCriando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const verificar = useCallback(() => {
    if (!projeto) return;
    setAmbiente(null);
    api
      .verificarAmbiente(projeto)
      .then(setAmbiente)
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto]);
  useEffect(verificar, [verificar]);

  const criar = (ev: FormEvent) => {
    ev.preventDefault();
    if (!projeto) return;
    setCriando(true);
    setFalha(null);
    apiFuncionalidades
      .criarMapeamento(projeto, pasta.trim() || null, area.trim() || null, entrada.trim() || null)
      .then((criada) => aoCriar(projeto, criada))
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setCriando(false);
      });
  };

  const pronto = ambiente?.pronto === true;
  return (
    <form className="nova-pergunta" onSubmit={criar} aria-label={ROTULO_FLUXO.mapear}>
      <h3>{ROTULO_FLUXO.mapear}</h3>
      <p className="muted">
        O agente parte dos pontos de entrada do código (telas, endpoints, lotes, rotinas de banco) e
        os agrupa em funcionalidades de negócio, por área. Ao aprovar, elas entram no catálogo do
        projeto.
      </p>
      <label>
        Projeto{" "}
        <select value={projeto} onChange={(e) => setProjeto(e.target.value)}>
          {projetos.map((s) => (
            <option key={s.id} value={s.id}>
              {s.nome}
            </option>
          ))}
        </select>
      </label>
      {ambiente && !pronto && (
        <PrepararAmbiente api={api} projeto={projeto} ambiente={ambiente} aoConcluir={verificar} />
      )}
      <label>
        Pasta (opcional)
        <input
          value={pasta}
          placeholder="Ex.: db ou backend/src. Vazio: o projeto inteiro."
          onChange={(e) => setPasta(e.target.value)}
        />
      </label>
      <label>
        Área (opcional)
        <input
          value={area}
          placeholder="Ex.: Cobrança"
          onChange={(e) => setArea(e.target.value)}
        />
      </label>
      <label>
        Orientação ao agente (opcional)
        <textarea
          rows={3}
          value={entrada}
          placeholder="Ex.: as telas da retaguarda ficam em delphi/."
          onChange={(e) => setEntrada(e.target.value)}
        />
      </label>
      {falha && (
        <p className="erro" role="alert">
          {falha}
        </p>
      )}
      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={!projeto || !pronto || criando}>
          {criando ? "Criando…" : "Mapear"}
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}
