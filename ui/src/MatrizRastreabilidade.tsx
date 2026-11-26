import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { mensagemDe } from "./analise";
import { Alerta as AlertaLacuna, EstadoVazio, MenuAcoes, Secao, Selo } from "./componentes";
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
  type Lacuna,
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
 *
 * Sem histórias (#407), a tabela não aparece: ficam as lacunas, em alerta, e uma linha
 * com o próximo passo ("Gerar histórias e critérios").
 */
export function MatrizRastreabilidade({
  projeto,
  demanda,
  navegar,
  api = apiRastreabilidadeTauri,
  atualizacao,
  aoGerarHistorias,
  aoCarregar,
}: {
  projeto: string;
  demanda: string;
  navegar: (t: Tela) => void;
  api?: ApiRastreabilidade;
  /** Muda quando o painel relê a demanda (a base mudou): relê a matriz junto. */
  atualizacao?: unknown;
  /** Abre a análise de histórias e critérios ligada à demanda; sem ela, não há o link. */
  aoGerarHistorias?: () => void;
  /** Avisa a matriz lida (o painel da demanda conta as lacunas no resumo). */
  aoCarregar?: (m: Matriz) => void;
}) {
  const [matriz, setMatriz] = useState<Matriz | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [linhas, setLinhas] = useState<Linhas>("historia");
  const [soLacunas, setSoLacunas] = useState(false);
  const [exportando, setExportando] = useState(false);
  const [aviso, setAviso] = useState<string | null>(null);
  // Na ref, para não reler a matriz quando o painel troca a função a cada render.
  const avisarCarga = useRef(aoCarregar);
  avisarCarga.current = aoCarregar;

  const carregar = useCallback(() => {
    let ativo = true;
    api
      .matriz(projeto, demanda)
      .then((m) => {
        if (!ativo || !m) return;
        setMatriz(m);
        setFalha(null);
        avisarCarga.current?.(m);
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

  // A tabela só aparece com histórias: sem elas, as linhas por regra repetem as lacunas.
  const comLinhas = !!matriz && matriz.historias.length > 0;
  const total = matriz ? (linhas === "historia" ? matriz.historias : matriz.regras).length : 0;
  const historias = matriz?.historias.filter((h) => !soLacunas || h.alertas.length > 0) ?? [];
  const regras = matriz?.regras.filter((r) => !soLacunas || r.alertas.length > 0) ?? [];
  const mostradas = linhas === "historia" ? historias.length : regras.length;

  const controles = comLinhas && (
    <>
      <label className="rastreabilidade-controle">
        Linhas por{" "}
        <select value={linhas} onChange={(e) => setLinhas(e.target.value as Linhas)}>
          <option value="historia">História</option>
          <option value="regra">Regra</option>
        </select>
      </label>
      <label className="rastreabilidade-controle">
        <input
          type="checkbox"
          checked={soLacunas}
          onChange={(e) => setSoLacunas(e.target.checked)}
        />{" "}
        só lacunas e alertas
      </label>
      <MenuAcoes
        rotulo="Exportar"
        desabilitado={exportando}
        itens={[
          { texto: "Excel (.xlsx)", aoEscolher: () => exportar("xlsx") },
          { texto: "Word (.docx)", aoEscolher: () => exportar("word") },
        ]}
      />
    </>
  );

  return (
    <Secao titulo="Matriz de rastreabilidade" acoes={controles} className="rastreabilidade">
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {aviso && <p role="status">{aviso}</p>}
      {!matriz ? (
        !falha && <p className="muted">Carregando a matriz…</p>
      ) : matriz.analises.length === 0 ? (
        <EstadoVazio>
          Nenhuma análise ligada à demanda ainda. A matriz se monta sozinha a partir das
          análises, histórias e cenários ligados a ela.
        </EstadoVazio>
      ) : !comLinhas ? (
        <>
          <Lacunas lacunas={matriz.lacunas} />
          <EstadoVazio
            acao={
              aoGerarHistorias
                ? { texto: "Gerar histórias e critérios", aoEscolher: aoGerarHistorias }
                : undefined
            }
          >
            Ainda não há histórias. A matriz aparece quando a primeira história for criada.
          </EstadoVazio>
        </>
      ) : (
        <div
          className="rastreabilidade-corpo"
          data-sem-lacunas={matriz.lacunas.length === 0 ? "" : undefined}
        >
          <div className="rastreabilidade-tabela">
            {mostradas === 0 ? (
              <EstadoVazio>Nenhuma linha com lacuna ou alerta.</EstadoVazio>
            ) : linhas === "historia" ? (
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
                        {h.titulo} {h.aprovada && <Selo tom="concluido">aprovada</Selo>}{" "}
                        {h.aConfirmar && <Selo tom="atencao">a confirmar</Selo>}
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
                          <Selo tom="atencao">a revalidar</Selo>
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
            {soLacunas && mostradas > 0 && (
              <p className="muted rastreabilidade-rodape">
                Mostrando {mostradas} de {total} linha(s) (filtro: só lacunas e alertas).
              </p>
            )}
          </div>
          {matriz.lacunas.length > 0 && (
            <aside className="rastreabilidade-lacunas" aria-label="Lacunas e alertas">
              <Lacunas lacunas={matriz.lacunas} />
            </aside>
          )}
        </div>
      )}
    </Secao>
  );
}

/** As lacunas da matriz, cada uma num alerta de atenção (fixo: não é anunciado). */
function Lacunas({ lacunas }: { lacunas: Lacuna[] }) {
  if (lacunas.length === 0) return null;
  return (
    <ul className="rastreabilidade-alertas" aria-label="Lacunas da rastreabilidade">
      {lacunas.map((l) => (
        <li key={`${l.alerta}:${l.alvo}`} data-alerta={l.alerta}>
          <AlertaLacuna tom="atencao" titulo={TITULO_LACUNA[l.alerta]} role="note">
            {l.descricao}
          </AlertaLacuna>
        </li>
      ))}
    </ul>
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
  if (alertas.length === 0) return <Selo tom="concluido">ok</Selo>;
  return (
    <>
      {alertas.map((a) => (
        <Selo key={a} tom="atencao">
          {ROTULO_ALERTA[a]}
        </Selo>
      ))}
    </>
  );
}
