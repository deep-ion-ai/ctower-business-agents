import { useCallback, useEffect, useState } from "react";
import { contar } from "./formatacao";
import { mensagemDe } from "./analise";
import {
  NOME_SITUACAO_CATALOGO,
  apiFuncionalidadesTauri,
  type ApiFuncionalidades,
  type Catalogo,
  type FuncionalidadeCatalogo,
} from "./funcionalidades";
import type { Tela } from "./Navegacao";

/**
 * Funcionalidades do projeto (#208): o catálogo por área, com a situação de cada
 * item (só mapeada ou documentada; nova ou sem ponto de entrada) e a ação
 * "Explicar", que abre o fluxo explicar com a funcionalidade preenchida. O
 * documento negocial aprovado fica ligado ao item.
 */
export function ListaFuncionalidades({
  projeto,
  navegar,
  api = apiFuncionalidadesTauri,
}: {
  projeto: string;
  navegar: (t: Tela) => void;
  api?: ApiFuncionalidades;
}) {
  const [catalogo, setCatalogo] = useState<Catalogo | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [explicando, setExplicando] = useState<string | null>(null);

  const carregar = useCallback(() => {
    // Promise.resolve: uma resposta vazia (sem o comando) não derruba a tela.
    Promise.resolve()
      .then(() => api.catalogo(projeto))
      .then((c) => {
        setCatalogo(c ?? null);
        setFalha(null);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto]);
  useEffect(carregar, [carregar]);

  const explicar = (f: FuncionalidadeCatalogo) => {
    setExplicando(f.id);
    setFalha(null);
    api
      .explicar(projeto, f.id)
      .then((criada) =>
        navegar({
          tipo: "analise",
          projeto,
          aberta: {
            id: criada.analise.id,
            abertura: { tipo: "nova", promptInicial: criada.promptInicial },
          },
        }),
      )
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setExplicando(null);
      });
  };

  const abrir = (documento: string) => navegar({ tipo: "base", documento });

  const [exportado, setExportado] = useState<string | null>(null);
  const exportar = (formato: "markdown" | "word") => {
    setFalha(null);
    setExportado(null);
    api
      .escolherDestino(formato)
      .then((destino) => (destino ? api.exportar(projeto, destino, formato) : null))
      .then((gravado) => gravado && setExportado(gravado))
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  return (
    <section aria-label="Funcionalidades" className="lista-funcionalidades">
      <h3>Funcionalidades</h3>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {catalogo && catalogo.total === 0 && (
        <p className="muted">
          Nenhuma funcionalidade mapeada ainda. Use “Mapear funcionalidades” no início: o agente
          monta o catálogo a partir do código.
        </p>
      )}
      {catalogo && catalogo.total > 0 && (
        <p className="muted">
          {contar(catalogo.total, "funcionalidade", "funcionalidades")} ·{" "}
          {contar(catalogo.documentadas, "documentada", "documentadas")}
          {catalogo.documento && (
            <>
              {" · "}
              <button type="button" className="link" onClick={() => abrir(catalogo.documento!)}>
                abrir o catálogo
              </button>
              {" · exportar em "}
              <button type="button" className="link" onClick={() => exportar("markdown")}>
                Markdown
              </button>
              {" ou "}
              <button type="button" className="link" onClick={() => exportar("word")}>
                Word
              </button>
            </>
          )}
        </p>
      )}
      {exportado && (
        <p role="status" className="muted">
          Catálogo exportado em {exportado}.
        </p>
      )}
      {catalogo?.areas.map((a) => (
        <div key={a.area} className="area-funcionalidades">
          <h4>{a.nome}</h4>
          <ul className="lista-regras">
            {a.funcionalidades.map((f) => (
              <li key={f.id} className="funcionalidade-catalogo" data-situacao={f.situacao}>
                <span className="painel-id">{f.id}</span> <strong>{f.nome}</strong>{" "}
                <span className="badge" data-tone={f.documento ? "done" : undefined}>
                  {f.documento ? "documentada" : "só mapeada"}
                </span>
                {f.situacao !== "mantida" && (
                  <span
                    className="badge"
                    data-tone={f.situacao === "sem-ponto-de-entrada" ? "attention" : undefined}
                  >
                    {NOME_SITUACAO_CATALOGO[f.situacao]}
                  </span>
                )}{" "}
                <span className="muted">{f.descricao}</span>{" "}
                {f.documento && (
                  <button type="button" className="link" onClick={() => abrir(f.documento!)}>
                    documento
                  </button>
                )}{" "}
                <button
                  type="button"
                  className="btn"
                  disabled={explicando !== null}
                  onClick={() => explicar(f)}
                  title={
                    f.pontos.length > 0 ? `Pontos de entrada: ${f.pontos.join(", ")}` : undefined
                  }
                >
                  {explicando === f.id ? "Abrindo…" : f.documento ? "Explicar de novo" : "Explicar"}
                </button>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </section>
  );
}
