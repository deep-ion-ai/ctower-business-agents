import { useMemo, useState } from "react";
import { mensagemDe } from "./analise";
import { TerminalClaude } from "./TerminalClaude";
import type { ApiTerminal } from "./terminal";
import type { TelaTerminal } from "./telaXterm";
import type {
  ApiMapa,
  IssueDoRastreador,
  JustificativaDeIssue,
  MotivoDeIssue,
  ResultadoDoLote,
  VistaDeIssues,
} from "./funcionalidades";

/** Período padrão da leitura, em meses. */
const MESES_PADRAO = 6;

type FiltroEstado = "todas" | "aberta" | "fechada";
type FiltroVinculo = "todas" | "pendentes" | "confirmadas" | "sugeridas" | "sem";

/** O que o analista decidiu de uma issue: funcionalidades escolhidas ou descarte. */
interface Escolha {
  funcionalidades: string[];
  descartada: boolean;
}

const chaveDe = (i: IssueDoRastreador) => `${i.origem}:${i.chave}`;

/** Já confirmada: a demanda da base está ligada a alguma funcionalidade. */
const confirmada = (i: IssueDoRastreador) => i.ligadaA.length > 0;

/**
 * Escolha inicial: o que já foi confirmado; senão, só a melhor sugestão (a lista toda
 * continua à vista para trocar).
 */
function inicial(i: IssueDoRastreador): Escolha {
  if (confirmada(i)) return { funcionalidades: [...i.ligadaA], descartada: false };
  return { funcionalidades: i.sugestoes[0] ? [i.sugestoes[0].funcionalidade] : [], descartada: false };
}

/**
 * Vincular demandas (#270): as issues do GitHub ou do Jira criadas no período,
 * cada uma com a funcionalidade sugerida (sem IA) e a pontuação. O analista
 * confirma, troca ou descarta por item e em lote; confirmar cria (ou reaproveita)
 * a demanda da base e a liga à funcionalidade. Nada é escrito no rastreador.
 */
export function VincularIssues({
  projeto,
  api,
  aoVoltar,
  aoVincular,
  apiTerminal,
  criarTela,
}: {
  projeto: string;
  api: ApiMapa;
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
  aoVoltar: () => void;
  /** Depois de gravar, para a tela reler o mapa. */
  aoVincular: () => void;
}) {
  const [meses, setMeses] = useState(String(MESES_PADRAO));
  const [desde, setDesde] = useState("");
  const [vista, setVista] = useState<VistaDeIssues | null>(null);
  const [escolhas, setEscolhas] = useState<Record<string, Escolha>>({});
  const [lendo, setLendo] = useState(false);
  const [gravando, setGravando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [resultado, setResultado] = useState<ResultadoDoLote | null>(null);
  const [estado, setEstado] = useState<FiltroEstado>("todas");
  const [vinculo, setVinculo] = useState<FiltroVinculo>("todas");
  const [rotulo, setRotulo] = useState("");
  const [busca, setBusca] = useState("");
  /** Pedido da sessão do Claude que propõe o vínculo das issues sem sugestão. */
  const [sessaoIa, setSessaoIa] = useState<string | null>(null);

  const ler = () => {
    const n = Number(meses);
    if (!desde.trim() && (!Number.isInteger(n) || n < 1)) {
      setFalha("Informe quantos meses (um número inteiro) ou uma data.");
      return Promise.resolve();
    }
    setLendo(true);
    setFalha(null);
    return api
      .issues(projeto, desde.trim() ? null : n, desde.trim() || null)
      .then((v) => {
        setVista(v);
        setEscolhas(Object.fromEntries(v.issues.map((i) => [chaveDe(i), inicial(i)])));
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setLendo(false));
  };

  const escolha = (i: IssueDoRastreador): Escolha => escolhas[chaveDe(i)] ?? inicial(i);
  const mudar = (i: IssueDoRastreador, f: (e: Escolha) => Escolha) =>
    setEscolhas((atual) => ({ ...atual, [chaveDe(i)]: f(escolha(i)) }));

  const rotulos = useMemo(
    () => [...new Set(vista?.issues.flatMap((i) => i.rotulos) ?? [])].sort(),
    [vista],
  );
  const visiveis = (vista?.issues ?? []).filter((i) => {
    if (estado !== "todas" && i.estado !== estado) return false;
    if (rotulo && !i.rotulos.includes(rotulo)) return false;
    if (vinculo === "confirmadas" && !confirmada(i)) return false;
    if (vinculo === "pendentes" && confirmada(i)) return false;
    if (vinculo === "sugeridas" && i.sugestoes.length === 0) return false;
    if (vinculo === "sem" && i.sugestoes.length > 0) return false;
    const q = busca.trim().toLowerCase();
    return !q || `${i.chave} ${i.titulo}`.toLowerCase().includes(q);
  });

  /** Issues que a confirmação grava: com funcionalidade escolhida ainda não ligada. */
  const aGravar = (vista?.issues ?? []).filter((i) => {
    const e = escolha(i);
    return !e.descartada && e.funcionalidades.some((f) => !i.ligadaA.includes(f));
  });

  const confirmar = (lote: IssueDoRastreador[]) => {
    setGravando(true);
    setFalha(null);
    setResultado(null);
    api
      .vincular(
        projeto,
        lote.map((i) => {
          const { sugestoes: _s, demanda: _d, ligadaA: _l, justificativa: _j, ...issue } = i;
          return { issue, funcionalidades: escolha(i).funcionalidades };
        }),
      )
      .then((r) => {
        setResultado(r);
        aoVincular();
        // Sem reler o rastreador (uma chamada de rede lenta): a própria resposta diz
        // a demanda de cada issue, e a tela marca as confirmadas.
        const demanda = new Map(r.demandas.map((d) => [`${d.origem}:${d.chave}`, d.demanda]));
        const gravadas = new Map(lote.map((i) => [chaveDe(i), escolha(i).funcionalidades]));
        setVista((v) =>
          v && {
            ...v,
            issues: v.issues.map((i) => {
              const f = gravadas.get(chaveDe(i));
              const d = demanda.get(chaveDe(i));
              return f && d
                ? { ...i, demanda: d, ligadaA: [...new Set([...i.ligadaA, ...f])], justificativa: null }
                : i;
            }),
          },
        );
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setGravando(false));
  };

  /** Issues sem sugestão e sem justificativa, que o agente pode tentar vincular. */
  const semSugestao = (vista?.issues ?? []).filter(
    (i) => !confirmada(i) && i.sugestoes.length === 0 && !i.justificativa,
  );
  const sugerirComIa = () => {
    setFalha(null);
    api
      .pedidoVinculoIa(
        semSugestao.map((i) => {
          const { sugestoes: _s, demanda: _d, ligadaA: _l, justificativa: _j, ...issue } = i;
          return issue;
        }),
      )
      .then(setSessaoIa)
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  const justificar = (i: IssueDoRastreador, j: JustificativaDeIssue | null) => {
    setFalha(null);
    const { sugestoes: _s, demanda: _d, ligadaA: _l, justificativa: _j, ...issue } = i;
    return api
      .justificar(projeto, issue, j)
      .then(() =>
        setVista((v) =>
          v && { ...v, issues: v.issues.map((x) => (chaveDe(x) === chaveDe(i) ? { ...x, justificativa: j } : x)) },
        ),
      )
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  if (sessaoIa !== null) {
    return (
      <section className="vincular-issues" aria-label="Sugestão com IA">
        <header className="vincular-cabecalho">
          <button
            type="button"
            className="btn"
            onClick={() => {
              setSessaoIa(null);
              // As propostas do agente aparecem como sugestão ao reler.
              void ler();
            }}
          >
            Encerrar a sessão e ver as propostas
          </button>
          <h3>Sugerir vínculos com o Claude</h3>
        </header>
        <p className="muted">
          O agente consulta o mapa e propõe a funcionalidade das {semSugestao.length} issues sem
          sugestão, com a evidência. Ele não confirma nada: você confirma na lista.
        </p>
        <div className="terminal-mapeamento">
          <TerminalClaude
            projeto={projeto}
            pedido={{ promptInicial: sessaoIa }}
            api={apiTerminal}
            criarTela={criarTela}
          />
        </div>
      </section>
    );
  }

  return (
    <section className="vincular-issues" aria-label="Vincular demandas">
      <header className="vincular-cabecalho">
        <button type="button" className="btn" onClick={aoVoltar}>
          ← Voltar ao mapa
        </button>
        <h3>Vincular demandas</h3>
      </header>
      <p className="muted">
        Lê as issues do rastreador do projeto e sugere a funcionalidade de cada uma pelo texto.
        Nada é escrito no rastreador: confirmar cria a demanda na base e a liga à funcionalidade.
      </p>

      <form
        className="vincular-periodo"
        onSubmit={(e) => {
          e.preventDefault();
          void ler();
        }}
      >
        <label>
          Issues criadas nos últimos{" "}
          <input
            type="number"
            min={1}
            max={120}
            value={meses}
            disabled={desde.trim() !== ""}
            onChange={(e) => setMeses(e.target.value)}
            aria-label="Meses"
          />{" "}
          meses
        </label>
        <label>
          ou desde{" "}
          <input
            type="date"
            value={desde}
            onChange={(e) => setDesde(e.target.value)}
            aria-label="Data inicial"
          />
        </label>
        <button type="submit" className="btn btn-primary" disabled={lendo}>
          {lendo ? "Buscando…" : "Buscar issues"}
        </button>
      </form>

      {falha && <p className="erro">{falha}</p>}
      {resultado && (
        <p className="aviso" role="status">
          {resultado.vinculosNovos} vínculo(s) novo(s), {resultado.criadas.length} demanda(s)
          criada(s), {resultado.reaproveitadas.length} reaproveitada(s)
          {resultado.vinculosExistentes > 0 &&
            `, ${resultado.vinculosExistentes} já existia(m)`}
          .
        </p>
      )}

      {vista && (
        <>
          <p className="muted" aria-label="Resumo da leitura">
            {vista.issues.length} issue(s) criadas desde {vista.desde} ·{" "}
            {vista.issues.filter(confirmada).length} confirmada(s) ·{" "}
            {vista.issues.filter((i) => !confirmada(i) && i.sugestoes.length > 0).length} com
            sugestão ·{" "}
            {vista.issues.filter((i) => !confirmada(i) && i.sugestoes.length === 0).length} sem
            funcionalidade
          </p>
          {vista.excedeuTeto && (
            <p className="aviso">
              O período tem mais issues que o limite de leitura: só as mais recentes foram
              trazidas. Reduza o período para ver as demais.
            </p>
          )}
          {vista.funcionalidades.length === 0 && (
            <p className="aviso">O mapa ainda não tem funcionalidades abertas para receber demandas.</p>
          )}

          <div className="vincular-filtros" role="group" aria-label="Filtros">
            <label>
              Estado{" "}
              <select value={estado} onChange={(e) => setEstado(e.target.value as FiltroEstado)}>
                <option value="todas">Todas</option>
                <option value="aberta">Abertas</option>
                <option value="fechada">Fechadas</option>
              </select>
            </label>
            <label>
              Vínculo{" "}
              <select value={vinculo} onChange={(e) => setVinculo(e.target.value as FiltroVinculo)}>
                <option value="todas">Todas</option>
                <option value="pendentes">A confirmar</option>
                <option value="confirmadas">Confirmadas</option>
                <option value="sugeridas">Com sugestão</option>
                <option value="sem">Sem funcionalidade</option>
              </select>
            </label>
            <label>
              Rótulo{" "}
              <select value={rotulo} onChange={(e) => setRotulo(e.target.value)}>
                <option value="">Todos</option>
                {rotulos.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Buscar{" "}
              <input type="search" value={busca} onChange={(e) => setBusca(e.target.value)} />
            </label>
          </div>

          <div className="acoes vincular-lote">
            <button
              type="button"
              className="btn btn-primary"
              disabled={gravando || aGravar.length === 0}
              onClick={() => confirmar(aGravar)}
            >
              {gravando
                ? "Confirmando…"
                : `Confirmar ${aGravar.length} ${aGravar.length === 1 ? "issue" : "issues"}`}
            </button>
            <button
              type="button"
              className="btn"
              disabled={semSugestao.length === 0 || vista.funcionalidades.length === 0}
              onClick={sugerirComIa}
            >
              Sugerir com IA ({semSugestao.length})
            </button>
            <button
              type="button"
              className="btn"
              onClick={() =>
                setEscolhas(Object.fromEntries((vista?.issues ?? []).map((i) => [chaveDe(i), inicial(i)])))
              }
            >
              Restaurar sugestões
            </button>
          </div>

          <ul className="vincular-lista" aria-label="Issues">
            {visiveis.map((i) => (
              <LinhaDaIssue
                key={chaveDe(i)}
                issue={i}
                escolha={escolha(i)}
                funcionalidades={vista.funcionalidades}
                desabilitado={gravando}
                aoMudar={(f) => mudar(i, f)}
                aoConfirmar={() => confirmar([i])}
                aoJustificar={(j) => justificar(i, j)}
              />
            ))}
            {visiveis.length === 0 && <li className="empty">Nenhuma issue com esses filtros.</li>}
          </ul>
        </>
      )}
    </section>
  );
}

function LinhaDaIssue({
  issue: i,
  escolha: e,
  funcionalidades,
  desabilitado,
  aoMudar,
  aoConfirmar,
  aoJustificar,
}: {
  issue: IssueDoRastreador;
  escolha: Escolha;
  funcionalidades: { id: string; nome: string; areaNome: string }[];
  desabilitado: boolean;
  aoMudar: (f: (e: Escolha) => Escolha) => void;
  aoConfirmar: () => void;
  aoJustificar: (j: JustificativaDeIssue | null) => Promise<void>;
}) {
  const [justificando, setJustificando] = useState(false);
  const [motivo, setMotivo] = useState<MotivoDeIssue>("fora_do_escopo");
  const [texto, setTexto] = useState("");
  const nomeDe = (id: string) => funcionalidades.find((f) => f.id === id)?.nome ?? id;
  const sugeridas = new Set(i.sugestoes.map((s) => s.funcionalidade));
  // Sugeridas e as que o analista acrescentou.
  const exibidas = [...i.sugestoes.map((s) => s.funcionalidade), ...e.funcionalidades.filter((f) => !sugeridas.has(f))];
  const alternar = (id: string) =>
    aoMudar((atual) => ({
      ...atual,
      funcionalidades: atual.funcionalidades.includes(id)
        ? atual.funcionalidades.filter((f) => f !== id)
        : [...atual.funcionalidades, id],
    }));
  const ok = i.ligadaA.length > 0;
  const novas = e.funcionalidades.some((f) => !i.ligadaA.includes(f));
  const livres = funcionalidades.filter((f) => !exibidas.includes(f.id));
  return (
    <li
      className={
        e.descartada
          ? "vincular-issue descartada"
          : ok
            ? "vincular-issue confirmada"
            : "vincular-issue"
      }
    >
      <div className="vincular-titulo">
        <a href={i.url} target="_blank" rel="noreferrer">
          {i.chave}
        </a>{" "}
        <strong>{i.titulo}</strong>{" "}
        {ok && (
          <span className="vincular-selo" title={`Demanda ${i.demanda}`}>
            ✓ Confirmada
          </span>
        )}{" "}
        <span className="muted">
          {i.origem === "github" ? "GitHub" : "Jira"} · {i.estado === "aberta" ? "aberta" : "fechada"}
          {i.rotulos.length > 0 && ` · ${i.rotulos.join(", ")}`}
        </span>
      </div>
      {ok && (
        <p className="muted">
          Ligada a {i.ligadaA.map(nomeDe).join(", ")} · demanda {i.demanda}.
          {novas && " Há funcionalidades marcadas ainda não confirmadas."}
        </p>
      )}
      {e.descartada ? (
        <p className="muted">
          Descartada: fica sem funcionalidade.{" "}
          <button
            type="button"
            className="btn"
            onClick={() => aoMudar((a) => ({ ...a, descartada: false }))}
          >
            Desfazer
          </button>
        </p>
      ) : (
        <>
          {exibidas.length === 0 && <p className="muted">Sem funcionalidade sugerida.</p>}
          <ul className="vincular-sugestoes" aria-label={`Funcionalidades de ${i.chave}`}>
            {exibidas.map((id) => {
              const s = i.sugestoes.find((x) => x.funcionalidade === id);
              return (
                <li key={id}>
                  <label>
                    <input
                      type="checkbox"
                      checked={e.funcionalidades.includes(id)}
                      onChange={() => alternar(id)}
                    />{" "}
                    {id} · {nomeDe(id)}
                  </label>
                  {s && (
                    <span className="muted" title={s.evidencias.join("; ")}>
                      {" "}
                      — {s.ia && <span className="badge">IA</span>}{" "}
                      {s.pontuacao > 0 && `${s.pontuacao} pontos `}({s.evidencias.slice(0, 3).join("; ")})
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
          <div className="acoes">
            {livres.length > 0 && (
              <select
                aria-label={`Outra funcionalidade para ${i.chave}`}
                value=""
                onChange={(ev) => ev.target.value && alternar(ev.target.value)}
              >
                <option value="">Outra funcionalidade…</option>
                {livres.map((f) => (
                  <option key={f.id} value={f.id}>
                    {f.id} · {f.nome} ({f.areaNome})
                  </option>
                ))}
              </select>
            )}
            <button
              type="button"
              className="btn"
              disabled={desabilitado || !novas}
              onClick={aoConfirmar}
            >
              {ok ? "Confirmar as novas" : "Confirmar"}
            </button>
            <button
              type="button"
              className="btn"
              onClick={() => aoMudar((a) => ({ ...a, descartada: true }))}
            >
              Descartar
            </button>
          </div>
          {!ok && i.justificativa && (
            <p className="muted">
              Sem funcionalidade — {MOTIVOS[i.justificativa.motivo]}: {i.justificativa.texto}{" "}
              <button type="button" className="btn" onClick={() => void aoJustificar(null)}>
                Desfazer justificativa
              </button>
            </p>
          )}
          {!ok && !i.justificativa && !justificando && (
            <div className="acoes">
              <button type="button" className="btn" onClick={() => setJustificando(true)}>
                Justificar sem funcionalidade
              </button>
            </div>
          )}
          {!ok && !i.justificativa && justificando && (
            <div className="acoes vincular-justificar">
              <select
                aria-label={`Motivo de ${i.chave}`}
                value={motivo}
                onChange={(ev) => setMotivo(ev.target.value as MotivoDeIssue)}
              >
                {Object.entries(MOTIVOS).map(([v, nome]) => (
                  <option key={v} value={v}>
                    {nome}
                  </option>
                ))}
              </select>
              <input
                type="text"
                aria-label={`Justificativa de ${i.chave}`}
                placeholder="Por que fica sem funcionalidade"
                value={texto}
                onChange={(ev) => setTexto(ev.target.value)}
              />
              <button
                type="button"
                className="btn"
                disabled={texto.trim() === ""}
                onClick={() =>
                  void aoJustificar({ motivo, texto: texto.trim() }).then(() => {
                    setJustificando(false);
                    setTexto("");
                  })
                }
              >
                Salvar justificativa
              </button>
              <button type="button" className="btn" onClick={() => setJustificando(false)}>
                Cancelar
              </button>
            </div>
          )}
        </>
      )}
    </li>
  );
}

const MOTIVOS: Record<MotivoDeIssue, string> = {
  fora_do_escopo: "fora do escopo",
  infraestrutura: "infraestrutura",
  outro: "outro motivo",
};
