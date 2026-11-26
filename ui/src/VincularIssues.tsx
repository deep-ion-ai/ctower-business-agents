import { useEffect, useMemo, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, EstadoVazio, Selo } from "./componentes";
import { EscolherFuncionalidades } from "./EscolherFuncionalidades";
import {
  BotaoIcone,
  IconeBuscar,
  IconeConfirmar,
  IconeDescartar,
  IconeDesfazer,
  IconeIa,
  IconeInfo,
  IconeLigada,
  IconeLinkExterno,
  IconeMais,
  IconeRestaurar,
  IconeSemFuncionalidade,
  IconeVoltar,
} from "./icones";
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

/** O trabalho em andamento numa tela de vínculo: a última busca e as escolhas. */
interface Trabalho {
  meses: string;
  desde: string;
  vista: VistaDeIssues;
  escolhas: Record<string, Escolha>;
  estado: FiltroEstado;
  vinculo: FiltroVinculo;
  rotulo: string;
  busca: string;
  /** Quando a busca foi feita (ms). */
  buscadaEm: number;
}

/**
 * Último trabalho por projeto: em memória (sair e voltar à tela) e no
 * armazenamento local (reabrir o app). Sem armazenamento, fica só a memória.
 */
const trabalhos = new Map<string, Trabalho>();
const chaveDoTrabalho = (projeto: string) => `cba.vincular.${projeto}`;

function trabalhoSalvo(projeto: string): Trabalho | null {
  const emMemoria = trabalhos.get(projeto);
  if (emMemoria) return emMemoria;
  try {
    const texto = localStorage.getItem(chaveDoTrabalho(projeto));
    const t = texto ? (JSON.parse(texto) as Trabalho) : null;
    if (t?.vista?.issues) {
      trabalhos.set(projeto, t);
      return t;
    }
  } catch {
    // Sem armazenamento ou conteúdo inválido: começa do zero.
  }
  return null;
}

function salvarTrabalho(projeto: string, t: Trabalho) {
  trabalhos.set(projeto, t);
  try {
    localStorage.setItem(chaveDoTrabalho(projeto), JSON.stringify(t));
  } catch {
    // Sem armazenamento: fica só em memória.
  }
}

/** Para os testes: esquece o trabalho guardado. */
export function esquecerTrabalhos() {
  for (const projeto of trabalhos.keys()) {
    try {
      localStorage.removeItem(chaveDoTrabalho(projeto));
    } catch {
      // ignora
    }
  }
  trabalhos.clear();
}

const quando = (ms: number) =>
  new Date(ms).toLocaleString("pt-BR", { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });

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
  // Volta onde parou: a última busca do projeto, com as escolhas e os filtros.
  const [salvo] = useState(() => trabalhoSalvo(projeto));
  const [meses, setMeses] = useState(salvo?.meses ?? String(MESES_PADRAO));
  const [desde, setDesde] = useState(salvo?.desde ?? "");
  const [vista, setVista] = useState<VistaDeIssues | null>(salvo?.vista ?? null);
  const [escolhas, setEscolhas] = useState<Record<string, Escolha>>(salvo?.escolhas ?? {});
  const [buscadaEm, setBuscadaEm] = useState<number | null>(salvo?.buscadaEm ?? null);
  const [lendo, setLendo] = useState(false);
  const [gravando, setGravando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [resultado, setResultado] = useState<ResultadoDoLote | null>(null);
  const [estado, setEstado] = useState<FiltroEstado>(salvo?.estado ?? "todas");
  const [vinculo, setVinculo] = useState<FiltroVinculo>(salvo?.vinculo ?? "todas");
  const [rotulo, setRotulo] = useState(salvo?.rotulo ?? "");
  const [busca, setBusca] = useState(salvo?.busca ?? "");

  useEffect(() => {
    if (vista) {
      salvarTrabalho(projeto, {
        meses,
        desde,
        vista,
        escolhas,
        estado,
        vinculo,
        rotulo,
        busca,
        buscadaEm: buscadaEm ?? Date.now(),
      });
    }
  }, [projeto, meses, desde, vista, escolhas, estado, vinculo, rotulo, busca, buscadaEm]);
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
        setBuscadaEm(Date.now());
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
  /** Issues que passam no estado, no rótulo e na busca (os chips contam sobre elas). */
  const filtradas = (vista?.issues ?? []).filter((i) => {
    if (estado !== "todas" && i.estado !== estado) return false;
    if (rotulo && !i.rotulos.includes(rotulo)) return false;
    const q = busca.trim().toLowerCase();
    return !q || `${i.chave} ${i.titulo}`.toLowerCase().includes(q);
  });
  const naSituacao = (i: IssueDoRastreador, v: FiltroVinculo) => {
    switch (v) {
      case "todas":
        return true;
      case "pendentes":
        return !confirmada(i);
      case "confirmadas":
        return confirmada(i);
      case "sugeridas":
        return !confirmada(i) && i.sugestoes.length > 0;
      case "sem":
        return !confirmada(i) && i.sugestoes.length === 0;
    }
  };
  const visiveis = filtradas.filter((i) => naSituacao(i, vinculo));

  /** Issues que a confirmação grava: com funcionalidade escolhida ainda não ligada. */
  // Só as que estão na tela: o lote não confirma o que os filtros escondem.
  const aGravar = visiveis.filter((i) => {
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
        projeto,
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
          <h3>Sugerir vínculos com o Claude</h3>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => {
              setSessaoIa(null);
              // As propostas do agente aparecem como sugestão ao reler.
              void ler();
            }}
          >
            <IconeConfirmar /> Encerrar a sessão e ver as propostas
          </button>
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

  const total = vista?.issues.length ?? 0;
  const contar = (v: FiltroVinculo) => filtradas.filter((i) => naSituacao(i, v)).length;
  const contagem: Record<FiltroVinculo, number> = {
    todas: contar("todas"),
    pendentes: contar("pendentes"),
    confirmadas: contar("confirmadas"),
    sugeridas: contar("sugeridas"),
    sem: contar("sem"),
  };
  const vinculadas = (vista?.issues ?? []).filter(confirmada).length;
  const progresso = total ? Math.round((vinculadas / total) * 100) : 0;

  return (
    <section className="vincular-issues" aria-label="Vincular demandas">
      <header className="vincular-cabecalho">
        <BotaoIcone rotulo="Voltar ao mapa" onClick={aoVoltar}>
          <IconeVoltar />
        </BotaoIcone>
        <h3>Vincular demandas</h3>
        <span
          className="vincular-info"
          tabIndex={0}
          title="Lê as issues do rastreador do projeto e sugere a funcionalidade de cada uma pelo texto. Nada é escrito no rastreador: confirmar cria a demanda na base e a liga à funcionalidade."
          aria-label="Sobre esta tela"
        >
          <IconeInfo />
        </span>
        <form
          className="vincular-periodo"
          onSubmit={(e) => {
            e.preventDefault();
            void ler();
          }}
        >
          <label>
            Últimos
            <input
              type="number"
              min={1}
              max={120}
              value={meses}
              disabled={desde.trim() !== ""}
              onChange={(e) => setMeses(e.target.value)}
              aria-label="Meses"
            />
            meses
          </label>
          <span className="muted">ou desde</span>
          <input
            type="date"
            value={desde}
            onChange={(e) => setDesde(e.target.value)}
            aria-label="Data inicial"
          />
          <button
            type="submit"
            className="btn btn-primary vincular-buscar"
            disabled={lendo}
            aria-label="Buscar issues"
            title="Buscar as issues do período no rastreador"
          >
            <IconeBuscar />
            {lendo ? "Buscando…" : "Buscar"}
          </button>
        </form>
      </header>

      {falha && <Alerta tom="erro" titulo={falha} />}
      {resultado && (
        <p className="vincular-resultado" role="status">
          <IconeConfirmar />
          {resultado.vinculosNovos} vínculo(s) novo(s), {resultado.criadas.length} demanda(s)
          criada(s), {resultado.reaproveitadas.length} reaproveitada(s)
          {resultado.vinculosExistentes > 0 &&
            `, ${resultado.vinculosExistentes} já existia(m)`}
          .
          <button
            type="button"
            className="vincular-fechar"
            aria-label="Fechar aviso"
            title="Fechar aviso"
            onClick={() => setResultado(null)}
          >
            <IconeDescartar />
          </button>
        </p>
      )}

      {!vista && !lendo && (
        <EstadoVazio>
          Escolha o período e busque as issues do rastreador para vincular às funcionalidades.
        </EstadoVazio>
      )}

      {vista && (
        <>
          <div className="vincular-painel">
            <div className="vincular-progresso" aria-label="Resumo da leitura">
              <div className="vincular-progresso-texto">
                <strong>
                  {vinculadas} de {total}
                </strong>{" "}
                issues vinculadas <span className="muted">· criadas desde {vista.desde}</span>
                {buscadaEm !== null && (
                  <span className="muted" title="Busque de novo para trazer as issues novas e as mudanças do rastreador">
                    {" "}
                    · buscadas em {quando(buscadaEm)}
                  </span>
                )}
              </div>
              <div
                className="vincular-barra"
                role="progressbar"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={progresso}
                aria-label="Issues vinculadas"
              >
                <span style={{ width: `${progresso}%` }} />
              </div>
            </div>
            <div className="vincular-chips" role="group" aria-label="Vínculo">
              {(
                [
                  ["todas", "Todas"],
                  ["pendentes", "A confirmar"],
                  ["confirmadas", "Confirmadas"],
                  ["sugeridas", "Com sugestão"],
                  ["sem", "Sem funcionalidade"],
                ] as [FiltroVinculo, string][]
              ).map(([v, nome]) => (
                <button
                  key={v}
                  type="button"
                  className={`vincular-chip chip-${v}`}
                  aria-pressed={vinculo === v}
                  onClick={() => setVinculo(v)}
                >
                  {nome} <span className="vincular-chip-n">{contagem[v]}</span>
                </button>
              ))}
            </div>
          </div>

          {vista.excedeuTeto && (
            <Alerta tom="atencao" titulo="Só as issues mais recentes foram trazidas." role="note">
              O período tem mais issues que o limite de leitura. Reduza o período para ver as demais.
            </Alerta>
          )}
          {vista.funcionalidades.length === 0 && (
            <Alerta
              tom="atencao"
              titulo="O mapa ainda não tem funcionalidades abertas para receber demandas."
              role="note"
            />
          )}

          <div className="vincular-barra-acoes">
            <div className="vincular-filtros" role="group" aria-label="Filtros">
              <label className="vincular-busca">
                <IconeBuscar />
                <input
                  type="search"
                  aria-label="Buscar"
                  placeholder="Chave ou título"
                  value={busca}
                  onChange={(e) => setBusca(e.target.value)}
                />
              </label>
              <select
                aria-label="Estado"
                value={estado}
                onChange={(e) => setEstado(e.target.value as FiltroEstado)}
              >
                <option value="todas">Todos os estados</option>
                <option value="aberta">Abertas</option>
                <option value="fechada">Fechadas</option>
              </select>
              {rotulos.length > 0 && (
                <select aria-label="Rótulo" value={rotulo} onChange={(e) => setRotulo(e.target.value)}>
                  <option value="">Todos os rótulos</option>
                  {rotulos.map((r) => (
                    <option key={r} value={r}>
                      {r}
                    </option>
                  ))}
                </select>
              )}
            </div>
            <div className="vincular-lote">
              <BotaoIcone
                rotulo="Restaurar sugestões"
                dica="Restaurar sugestões: desfaz as escolhas ainda não confirmadas"
                onClick={() =>
                  setEscolhas(Object.fromEntries((vista?.issues ?? []).map((i) => [chaveDe(i), inicial(i)])))
                }
              >
                <IconeRestaurar />
              </BotaoIcone>
              <BotaoIcone
                rotulo={`Sugerir com IA (${semSugestao.length})`}
                dica={`Sugerir com IA: o Claude propõe a funcionalidade das ${semSugestao.length} issue(s) sem sugestão`}
                disabled={semSugestao.length === 0 || vista.funcionalidades.length === 0}
                onClick={sugerirComIa}
              >
                <IconeIa />
                {semSugestao.length > 0 && <span className="icone-contador">{semSugestao.length}</span>}
              </BotaoIcone>
              <button
                type="button"
                className="btn btn-primary vincular-confirmar-lote"
                disabled={gravando || aGravar.length === 0}
                onClick={() => confirmar(aGravar)}
                title="Confirma de uma vez as issues da lista (com os filtros atuais) que têm funcionalidade marcada"
              >
                <IconeConfirmar />
                {gravando
                  ? "Confirmando…"
                  : `Confirmar ${aGravar.length} ${aGravar.length === 1 ? "issue" : "issues"}`}
              </button>
            </div>
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
            {visiveis.length === 0 && (
              <li>
                <EstadoVazio>Nenhuma issue com esses filtros.</EstadoVazio>
              </li>
            )}
          </ul>
        </>
      )}
    </section>
  );
}

/** Tipos de evidência de uma sugestão (`parcel… (nome)` → `nome`), sem repetir. */
function tiposDeEvidencia(evidencias: string[]): string[] {
  const tipos = evidencias.map((e) => {
    if (e.startsWith("IA:")) return "IA";
    if (e.startsWith("arquivo ")) return e.includes("PR") ? "arquivo do PR" : "arquivo citado";
    return /\(([^)]+)\)\s*$/.exec(e)?.[1] ?? e;
  });
  return [...new Set(tipos)].filter((t) => t !== "IA");
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
  const [escolhendo, setEscolhendo] = useState(false);
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
  const maior = Math.max(1, ...i.sugestoes.map((s) => s.pontuacao));
  const situacao = e.descartada
    ? "descartada"
    : ok
      ? "confirmada"
      : i.justificativa
        ? "justificada"
        : exibidas.length > 0
          ? "sugerida"
          : "sem";

  return (
    <li className={`vincular-issue ${situacao}`}>
      <div className="vincular-topo">
        <div className="vincular-titulo">
          <a
            className="vincular-chave"
            href={i.url}
            target="_blank"
            rel="noreferrer"
            title={`Abrir ${i.chave} no ${i.origem === "github" ? "GitHub" : "Jira"}`}
          >
            {i.chave}
            <IconeLinkExterno />
          </a>
          <strong>{i.titulo}</strong>
          <span className="vincular-meta">
            <Selo>{i.estado === "aberta" ? "aberta" : "fechada"}</Selo>
            {i.rotulos.map((r) => (
              <Selo key={r} tom="neutro">
                {r}
              </Selo>
            ))}
          </span>
        </div>
        <div className="vincular-acoes" role="group" aria-label={`Ações de ${i.chave}`}>
          {ok && (
            <Selo tom="concluido" titulo={`Demanda ${i.demanda}`}>
              Confirmada
            </Selo>
          )}
          {e.descartada ? (
            <BotaoIcone rotulo="Desfazer" dica="Desfazer o descarte" onClick={() => aoMudar((a) => ({ ...a, descartada: false }))}>
              <IconeDesfazer />
            </BotaoIcone>
          ) : (
            <>
              {livres.length > 0 && (
                <BotaoIcone
                  rotulo="Outra funcionalidade…"
                  dica="Escolher outra funcionalidade do mapa"
                  onClick={() => setEscolhendo(true)}
                >
                  <IconeMais />
                </BotaoIcone>
              )}
              {!ok && !i.justificativa && (
                <BotaoIcone
                  rotulo="Justificar sem funcionalidade"
                  dica="Justificar por que fica sem funcionalidade (fora do escopo, infraestrutura…)"
                  onClick={() => setJustificando((j) => !j)}
                >
                  <IconeSemFuncionalidade />
                </BotaoIcone>
              )}
              <BotaoIcone
                rotulo="Descartar"
                dica="Descartar: deixa a issue fora da confirmação em lote"
                onClick={() => aoMudar((a) => ({ ...a, descartada: true }))}
              >
                <IconeDescartar />
              </BotaoIcone>
              <BotaoIcone
                rotulo={ok ? "Confirmar as novas" : "Confirmar"}
                dica={
                  ok
                    ? "Confirmar as funcionalidades marcadas que ainda não estão ligadas"
                    : "Confirmar: cria a demanda e liga às funcionalidades marcadas"
                }
                tom="principal"
                disabled={desabilitado || !novas}
                onClick={aoConfirmar}
              >
                <IconeConfirmar />
              </BotaoIcone>
            </>
          )}
        </div>
      </div>

      {ok && (
        <p className="vincular-ligada">
          <IconeLigada /> Ligada a {i.ligadaA.map(nomeDe).join(", ")} · demanda {i.demanda}
          {novas && <span className="vincular-pendente"> · há funcionalidades novas a confirmar</span>}
        </p>
      )}

      {e.descartada ? (
        <p className="muted">Descartada: fica fora da confirmação.</p>
      ) : (
        <>
          {exibidas.length === 0 && !i.justificativa && (
            <p className="muted vincular-sem">Sem funcionalidade sugerida. Escolha uma com + ou justifique.</p>
          )}
          {exibidas.length > 0 && (
            <ul className="vincular-sugestoes" aria-label={`Funcionalidades de ${i.chave}`}>
              {exibidas.map((id) => {
                const s = i.sugestoes.find((x) => x.funcionalidade === id);
                const marcada = e.funcionalidades.includes(id);
                const ia = s?.evidencias.find((x) => x.startsWith("IA:"));
                return (
                  <li key={id} className={marcada ? "marcada" : undefined}>
                    <label title={s ? s.evidencias.join("\n") : "Escolhida por você"}>
                      <input type="checkbox" checked={marcada} onChange={() => alternar(id)} />
                      <span className="vincular-fid">{id}</span>
                      <span className="vincular-fnome">{nomeDe(id)}</span>
                    </label>
                    <span className="vincular-evidencia">
                      {s?.ia && (
                        <Selo tom="agente" titulo={ia}>
                          IA
                        </Selo>
                      )}
                      {s &&
                        tiposDeEvidencia(s.evidencias).map((t) => (
                          <Selo key={t} tom="neutro">
                            {t}
                          </Selo>
                        ))}
                      {ia && <span className="vincular-ia-texto">{ia.replace(/^IA:\s*/, "")}</span>}
                      {s && s.pontuacao > 0 && (
                        <span
                          className="vincular-pontos"
                          title={`${s.pontuacao} pontos de semelhança com o texto da issue`}
                        >
                          <span className="vincular-medidor">
                            <span style={{ width: `${Math.round((s.pontuacao / maior) * 100)}%` }} />
                          </span>
                          {s.pontuacao} pontos
                        </span>
                      )}
                    </span>
                  </li>
                );
              })}
            </ul>
          )}

          {escolhendo && (
            <EscolherFuncionalidades
              titulo={`${i.chave} - ${i.titulo}`}
              funcionalidades={funcionalidades}
              jaEscolhidas={e.funcionalidades}
              aoFechar={() => setEscolhendo(false)}
              aoAdicionar={(ids) => {
                aoMudar((atual) => ({
                  ...atual,
                  funcionalidades: [...new Set([...atual.funcionalidades, ...ids])],
                }));
                setEscolhendo(false);
              }}
            />
          )}

          {!ok && i.justificativa && (
            <p className="vincular-justificada">
              <IconeSemFuncionalidade />
              <span>
                Sem funcionalidade — <strong>{MOTIVOS[i.justificativa.motivo]}</strong>: {i.justificativa.texto}
              </span>
              <BotaoIcone rotulo="Desfazer justificativa" onClick={() => void aoJustificar(null)}>
                <IconeDesfazer />
              </BotaoIcone>
            </p>
          )}
          {!ok && !i.justificativa && justificando && (
            <form
              className="vincular-justificar"
              onSubmit={(ev) => {
                ev.preventDefault();
                if (!texto.trim()) return;
                void aoJustificar({ motivo, texto: texto.trim() }).then(() => {
                  setJustificando(false);
                  setTexto("");
                });
              }}
            >
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
                autoFocus
                aria-label={`Justificativa de ${i.chave}`}
                placeholder="Por que fica sem funcionalidade"
                value={texto}
                onChange={(ev) => setTexto(ev.target.value)}
              />
              <button
                type="submit"
                className="icone-acao icone-principal"
                aria-label="Salvar justificativa"
                title="Salvar justificativa"
                disabled={texto.trim() === ""}
              >
                <IconeConfirmar />
              </button>
              <BotaoIcone rotulo="Cancelar" onClick={() => setJustificando(false)}>
                <IconeDescartar />
              </BotaoIcone>
            </form>
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
