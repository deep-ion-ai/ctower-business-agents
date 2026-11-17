import { useCallback, useEffect, useState, type ReactNode } from "react";
import { mensagemDe } from "./analise";
import type { Tela } from "./Navegacao";
import {
  apiRastreabilidadeTauri,
  rotuloEvidencia,
  ROTULO_ALERTA,
  TITULO_LACUNA,
  type Alerta,
  type ApiRastreabilidade,
  type CenarioRastro,
  type EvidenciaRastro,
  type FormatoMatriz,
  type Matriz,
  type RegraRastro,
} from "./rastreabilidade";
import "./rastreabilidade.css";

type Linhas = "historia" | "regra";

/**
 * Matriz de rastreabilidade da demanda (#121): demanda → análises → histórias →
 * regras → evidências → cenários, derivada do índice da base a cada mudança (nada
 * aqui é editado à mão). Linhas por história ou por regra, filtro "só lacunas e
 * alertas", exportação para Excel e Word, e o painel das lacunas: evidência que
 * mudou sob história aprovada, regra afetada sem história, história sem cenário e
 * história sem regra.
 */
export function MatrizRastreabilidade({
  projeto,
  demanda,
  navegar,
  api = apiRastreabilidadeTauri,
  atualizacao,
}: {
  projeto: string;
  demanda: string;
  navegar: (t: Tela) => void;
  api?: ApiRastreabilidade;
  /** Muda quando o painel relê a demanda (a base mudou): relê a matriz junto. */
  atualizacao?: unknown;
}) {
  const [matriz, setMatriz] = useState<Matriz | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [linhas, setLinhas] = useState<Linhas>("historia");
  const [soLacunas, setSoLacunas] = useState(false);
  const [exportando, setExportando] = useState(false);
  const [aviso, setAviso] = useState<string | null>(null);

  const carregar = useCallback(() => {
    let ativo = true;
    api
      .matriz(projeto, demanda)
      .then((m) => {
        if (!ativo || !m) return;
        setMatriz(m);
        setFalha(null);
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto, demanda]);

  useEffect(carregar, [carregar, atualizacao]);

  const abrirAnalise = (id: string) =>
    navegar({ tipo: "analise", projeto, aberta: { id, abertura: { tipo: "reabrir" } } });
  const abrirDocumento = (caminho: string) => navegar({ tipo: "base", documento: caminho });

  const exportar = (formato: FormatoMatriz) => {
    setExportando(true);
    setAviso(null);
    setFalha(null);
    api
      .escolherDestino(`matriz-${demanda}`, formato)
      .then((destino) =>
        destino
          ? api
              .exportar(projeto, demanda, formato, destino)
              .then((gravado) => setAviso(`Matriz exportada em ${gravado}.`))
          : undefined,
      )
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setExportando(false));
  };

  const Regra = ({ r }: { r: RegraRastro }) =>
    r.documento ? (
      <button
        type="button"
        className="link mono"
        title={r.titulo}
        onClick={() => abrirDocumento(r.documento!)}
      >
        {r.id}
      </button>
    ) : (
      <span className="mono" title="regra fora da base">
        {r.id}
      </span>
    );
  const Analise = ({ id }: { id: string }) => (
    <button type="button" className="link mono" onClick={() => abrirAnalise(id)}>
      {id}
    </button>
  );
  const Cenario = ({ c }: { c: CenarioRastro }) => (
    <button
      type="button"
      className="link mono"
      title={c.titulo}
      onClick={() => abrirAnalise(c.analise)}
      data-alerta={c.aRevalidar ? "" : undefined}
    >
      {c.id}
    </button>
  );

  const total = matriz ? (linhas === "historia" ? matriz.historias : matriz.regras).length : 0;
  const historias = matriz?.historias.filter((h) => !soLacunas || h.alertas.length > 0) ?? [];
  const regras = matriz?.regras.filter((r) => !soLacunas || r.alertas.length > 0) ?? [];
  const mostradas = linhas === "historia" ? historias.length : regras.length;

  return (
    <section className="rastreabilidade" aria-label="Rastreabilidade">
      <header className="rastreabilidade-cabecalho">
        <h3>Matriz de rastreabilidade</h3>
        <span className="muted">derivada do índice; reconstruída a cada mudança</span>
        <div className="rastreabilidade-controles">
          <label>
            Linhas por{" "}
            <select value={linhas} onChange={(e) => setLinhas(e.target.value as Linhas)}>
              <option value="historia">História</option>
              <option value="regra">Regra</option>
            </select>
          </label>
          <label>
            <input
              type="checkbox"
              checked={soLacunas}
              onChange={(e) => setSoLacunas(e.target.checked)}
            />{" "}
            só lacunas e alertas
          </label>
          <button
            type="button"
            className="btn"
            disabled={!matriz || exportando}
            onClick={() => exportar("xlsx")}
          >
            Exportar matriz (.xlsx)
          </button>
          <button
            type="button"
            className="btn"
            disabled={!matriz || exportando}
            onClick={() => exportar("word")}
          >
            Exportar Word
          </button>
        </div>
      </header>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {aviso && <p role="status">{aviso}</p>}
      {!matriz ? (
        !falha && <p className="muted">Carregando a matriz…</p>
      ) : matriz.analises.length === 0 ? (
        <p className="muted">
          Nenhuma análise ligada à demanda ainda. A matriz se monta sozinha a partir das
          análises, histórias e cenários ligados a ela.
        </p>
      ) : (
        <div className="rastreabilidade-corpo">
          <div className="rastreabilidade-tabela">
            {linhas === "historia" ? (
              <table aria-label="Matriz por história">
                <thead>
                  <tr>
                    <th>História</th>
                    <th>Análise</th>
                    <th>Regras</th>
                    <th>Evidências</th>
                    <th>Cenários</th>
                    <th>Situação</th>
                  </tr>
                </thead>
                <tbody>
                  {historias.map((h) => (
                    <tr key={h.id} data-alerta={h.alertas.length > 0 ? "" : undefined}>
                      <td>
                        <button
                          type="button"
                          className="link mono"
                          onClick={() => abrirAnalise(h.analise)}
                        >
                          {h.id}
                        </button>{" "}
                        {h.titulo}{" "}
                        {h.aprovada && (
                          <span className="badge" data-tone="done">
                            aprovada
                          </span>
                        )}{" "}
                        {h.aConfirmar && (
                          <span className="badge" data-tone="attention">
                            a confirmar
                          </span>
                        )}
                      </td>
                      <td>
                        <Lista itens={h.analises.map((a) => <Analise key={a} id={a} />)} />
                      </td>
                      <td>
                        <Lista
                          itens={h.regras.map((r) => <Regra key={r.id} r={r} />)}
                          vazio={h.aConfirmar ? "sem regra (pergunta em aberto)" : "sem regra"}
                        />
                      </td>
                      <td>
                        <Evidencias lista={h.evidencias} />
                      </td>
                      <td>
                        <Lista
                          itens={h.cenarios.map((c) => <Cenario key={c.id} c={c} />)}
                          vazio="nenhum cenário"
                        />
                      </td>
                      <td>
                        <Situacao alertas={h.alertas} />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <table aria-label="Matriz por regra">
                <thead>
                  <tr>
                    <th>Regra</th>
                    <th>Análises</th>
                    <th>Histórias</th>
                    <th>Evidências</th>
                    <th>Cenários</th>
                    <th>Situação</th>
                  </tr>
                </thead>
                <tbody>
                  {regras.map((r) => (
                    <tr key={r.regra.id} data-alerta={r.alertas.length > 0 ? "" : undefined}>
                      <td>
                        <Regra r={r.regra} /> {r.regra.titulo}{" "}
                        {r.regra.status === "a-revalidar" && (
                          <span className="badge" data-tone="attention">
                            a revalidar
                          </span>
                        )}
                      </td>
                      <td>
                        <Lista itens={r.analises.map((a) => <Analise key={a} id={a} />)} />
                      </td>
                      <td>
                        <Lista
                          itens={r.historias.map((h) => (
                            <span key={h} className="mono">
                              {h}
                            </span>
                          ))}
                          vazio="nenhuma história"
                        />
                      </td>
                      <td>
                        <Evidencias lista={r.evidencias} />
                      </td>
                      <td>
                        <Lista itens={r.cenarios.map((c) => <Cenario key={c.id} c={c} />)} />
                      </td>
                      <td>
                        <Situacao alertas={r.alertas} />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
            <p className="muted">
              Mostrando {mostradas} de {total} linha(s)
              {soLacunas && " (filtro: só lacunas e alertas)"}. Cada célula abre o documento ou
              a análise; nada aqui é editado à mão.
            </p>
          </div>
          <aside className="rastreabilidade-lacunas" aria-label="Lacunas e alertas">
            <h4>Lacunas e alertas</h4>
            {matriz.lacunas.length === 0 ? (
              <p className="muted">Nenhuma lacuna.</p>
            ) : (
              <ul>
                {matriz.lacunas.map((l) => (
                  <li key={`${l.alerta}:${l.alvo}`} data-alerta={l.alerta}>
                    <strong>{TITULO_LACUNA[l.alerta]}</strong>
                    <p>{l.descricao}</p>
                  </li>
                ))}
              </ul>
            )}
          </aside>
        </div>
      )}
    </section>
  );
}

function Lista({ itens, vazio = "—" }: { itens: ReactNode[]; vazio?: string }) {
  if (itens.length === 0) return <span className="muted">{vazio}</span>;
  return (
    <ul className="rastreabilidade-lista">
      {itens.map((i, n) => (
        <li key={n}>{i}</li>
      ))}
    </ul>
  );
}

function Evidencias({ lista }: { lista: EvidenciaRastro[] }) {
  if (lista.length === 0) return <span className="muted">—</span>;
  return (
    <ul className="rastreabilidade-lista">
      {lista.map((e) => (
        <li key={`${e.arquivo}:${e.linhaInicial}-${e.linhaFinal}`} title={`de ${e.de}`}>
          <span className="mono">{rotuloEvidencia(e)}</span>
          {e.aRevalidar && <span className="rastreabilidade-revalidar"> a revalidar</span>}
        </li>
      ))}
    </ul>
  );
}

function Situacao({ alertas }: { alertas: Alerta[] }) {
  if (alertas.length === 0) {
    return (
      <span className="badge" data-tone="done">
        ok
      </span>
    );
  }
  return (
    <>
      {alertas.map((a) => (
        <span key={a} className="badge" data-tone="attention">
          {ROTULO_ALERTA[a]}
        </span>
      ))}
    </>
  );
}
