import { useCallback, useEffect, useState, type FormEvent } from "react";
import { mensagemDe } from "./analise";
import type { ApiConhecimento } from "./conhecimento";
import { ROTULO_STATUS_DEMANDA, type DemandaVista } from "./demandas";
import type { Tela } from "./Navegacao";

/**
 * Demandas do projeto (#138), no início da tela do projeto: as abertas primeiro,
 * com criar (título, descrição e chave externa opcional), encerrar e reabrir. Cada
 * demanda abre o painel dela.
 */
export function ListaDemandas({
  api,
  projeto,
  navegar,
}: {
  api: ApiConhecimento;
  projeto: string;
  navegar: (t: Tela) => void;
}) {
  const [demandas, setDemandas] = useState<DemandaVista[] | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [criando, setCriando] = useState(false);
  const [mudando, setMudando] = useState<string | null>(null);

  const carregar = useCallback(() => {
    api
      .listarDemandas(projeto)
      .then((l) => {
        setDemandas(l);
        setFalha(null);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto]);

  useEffect(carregar, [carregar]);

  const mudarStatus = (d: DemandaVista) => {
    setMudando(d.id);
    setFalha(null);
    (d.status === "aberta"
      ? api.encerrarDemanda(projeto, d.id)
      : api.reabrirDemanda(projeto, d.id)
    )
      .then(carregar)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setMudando(null));
  };

  return (
    <section aria-label="Demandas">
      <h3>Demandas</h3>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {demandas === null && !falha && <p className="muted">Carregando…</p>}
      {demandas?.length === 0 && (
        <p className="muted">
          Nenhuma demanda ainda. Crie uma para juntar as atas, as análises e as sessões do pedido do
          cliente.
        </p>
      )}
      {demandas && demandas.length > 0 && (
        <ul className="lista-demandas">
          {demandas.map((d) => (
            <li key={d.id} data-status={d.status}>
              <button
                type="button"
                className="link"
                onClick={() => navegar({ tipo: "demanda", projeto, demanda: d.id })}
              >
                {d.id}: {d.titulo}
              </button>{" "}
              <span className="badge" data-tone={d.status === "aberta" ? "running" : "done"}>
                {ROTULO_STATUS_DEMANDA[d.status]}
              </span>
              {d.chave && <span className="muted">{d.chave}</span>}
              <span className="muted">
                {d.status === "encerrada" && d.encerrado
                  ? `encerrada em ${d.encerrado}`
                  : `criada em ${d.criado}`}
              </span>
              <button
                type="button"
                className="btn"
                disabled={mudando === d.id}
                onClick={() => mudarStatus(d)}
              >
                {d.status === "aberta" ? `Encerrar ${d.id}` : `Reabrir ${d.id}`}
              </button>
            </li>
          ))}
        </ul>
      )}
      {criando ? (
        <FormDemanda
          api={api}
          projeto={projeto}
          aoCancelar={() => setCriando(false)}
          aoCriar={() => {
            setCriando(false);
            carregar();
          }}
        />
      ) : (
        <button type="button" className="btn" onClick={() => setCriando(true)}>
          Nova demanda
        </button>
      )}
    </section>
  );
}

function FormDemanda({
  api,
  projeto,
  aoCriar,
  aoCancelar,
}: {
  api: ApiConhecimento;
  projeto: string;
  aoCriar: (d: DemandaVista) => void;
  aoCancelar: () => void;
}) {
  const [titulo, setTitulo] = useState("");
  const [descricao, setDescricao] = useState("");
  const [chave, setChave] = useState("");
  const [gravando, setGravando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const criar = (ev: FormEvent) => {
    ev.preventDefault();
    if (!titulo.trim()) return;
    setGravando(true);
    setFalha(null);
    api
      .criarDemanda(projeto, {
        titulo: titulo.trim(),
        descricao: descricao.trim() || undefined,
        chave: chave.trim() || undefined,
      })
      .then(aoCriar)
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setGravando(false);
      });
  };

  return (
    <form className="nova-pergunta" onSubmit={criar} aria-label="Nova demanda">
      <label>
        Título
        <input
          value={titulo}
          onChange={(e) => setTitulo(e.target.value)}
          placeholder="Ex.: Parcelamento da fatura para pessoa jurídica"
        />
      </label>
      <label>
        Descrição (opcional)
        <textarea
          value={descricao}
          onChange={(e) => setDescricao(e.target.value)}
          rows={3}
          placeholder="O pedido do cliente, com as palavras dele."
        />
      </label>
      <label>
        Chave externa (opcional)
        <input
          value={chave}
          onChange={(e) => setChave(e.target.value)}
          placeholder="Ex.: FAT-123"
          spellCheck={false}
        />
      </label>
      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={gravando || !titulo.trim()}>
          Criar demanda
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </form>
  );
}
