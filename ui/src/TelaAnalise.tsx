import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import {
  apiAnaliseTauri,
  mensagemDe,
  type AmbienteClaude,
  type AnaliseCriada,
  type AnaliseResumo,
  type ApiAnalise,
  type EstadoAnalise,
  type Fluxo,
  type Situacao,
  ROTULO_ESTADO_ANALISE,
} from "./analise";
import { ROTULO_FLUXO, type ApiConhecimento } from "./conhecimento";
import type { DemandaVista } from "./demandas";
import { FormDemandaNegocial } from "./DemandaNegocial";
import { NovaAnalise } from "./NovaAnalise";
import { NovaHistorias } from "./NovaHistorias";
import { NovaReuniao } from "./NovaReuniao";
import { PainelAnalise, type Abertura, type EstadoConversa } from "./PainelAnalise";
import { PrepararAmbiente } from "./PrepararAmbiente";
import type { ApiTerminal, ProjetoResumo } from "./terminal";
import type { TelaTerminal } from "./telaXterm";

interface Props {
  /** Para testes. */
  api?: ApiAnalise;
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
  /** Projeto escolhido ao abrir: o da barra de título (ou o recém-cadastrado no primeiro uso). */
  projetoInicial?: string;
  /** Abre a lista já filtrada por situação (link da Visão geral). */
  situacaoInicial?: EstadoAnalise;
  /** Abre o painel "Nova análise" (ação global da Visão geral e das Atividades). */
  novaInicial?: boolean;
  /** Para os tipos de análise além da pergunta; sem ele, só "Perguntar ao sistema". */
  apiConhecimento?: ApiConhecimento;
  /** Pergunta já preenchida (a sugestão do fim do primeiro uso). */
  perguntaInicial?: string;
  /** Abre o assistente do primeiro uso quando falta a base ou um projeto. */
  aoPrimeiroUso?: () => void;
  /** Análise já aberta (criada pelo início ou escolhida numa lista); exige `projetoInicial`. */
  aberturaInicial?: { id: string; abertura: Abertura };
  /**
   * Abre a análise fora desta tela (na navegação, numa aba própria, que fica viva
   * enquanto aberta: #212). Sem ele, a análise abre aqui mesmo, no lugar da lista.
   */
  aoAbrirAnalise?: (projeto: string, id: string, abertura: Abertura) => void;
  /** Abre a demanda (a recém-registrada pela Demanda Negocial) na navegação. */
  aoAbrirDemanda?: (projeto: string, demanda: string) => void;
  /** "← Análises" fora desta tela (na navegação, a aba da lista). Sem ele, volta aqui. */
  aoVoltar?: (projeto: string) => void;
  /** A conversa da análise aberta (a navegação confirma antes de fechar a aba). */
  aoMudarConversa?: (estado: EstadoConversa) => void;
}

const ESTADO = ROTULO_ESTADO_ANALISE;
const SITUACOES: EstadoAnalise[] = ["em-andamento", "concluida", "aprovada", "criada"];
/** Rótulo do chip de cada situação (plural, como na lista de filtros). */
const ROTULO_CHIP: Record<EstadoAnalise, string> = {
  "em-andamento": "Em andamento",
  concluida: "Aguardando revisão",
  aprovada: "Aprovadas",
  criada: "Criadas",
};
/** A ação principal da linha, pela situação da análise. */
const ACAO: Record<EstadoAnalise, string> = {
  criada: "Continuar",
  "em-andamento": "Acompanhar",
  concluida: "Revisar",
  aprovada: "Abrir",
};
const TIPOS: Fluxo[] = [
  "perguntar",
  "explicar",
  "impacto",
  "reuniao",
  "revisar",
  "historias",
  "cenarios",
];

/** O que cada tipo faz, na escolha do tipo do painel "Nova análise" (#350). */
const DESCRICAO_FLUXO: Partial<Record<Fluxo, string>> = {
  perguntar:
    "Tire uma dúvida sobre como o sistema funciona hoje. A resposta vem do código e da base de conhecimento.",
  explicar: "Receba a explicação de uma funcionalidade que já existe, com as regras e o passo a passo.",
  impacto: "Descreva uma mudança e veja o que ela afeta: telas, regras, código e testes.",
  reuniao: "Cole o transcript de uma reunião e receba as decisões, as pendências e as demandas.",
  revisar:
    "Aponte lacunas, ambiguidades e critérios de aceite que faltam num requisito ou numa história.",
  historias: "Quebre uma análise aprovada em histórias com critérios de aceite.",
  cenarios: "Gere cenários de teste a partir de uma análise aprovada.",
};

/** O que se escolhe no primeiro passo do painel "Nova análise". */
type Escolha = Fluxo | "demanda-negocial" | "cadastrada";

const NOME_ESCOLHA = (e: Escolha) =>
  e === "demanda-negocial"
    ? "Demanda Negocial"
    : e === "cadastrada"
      ? "Analisar demanda cadastrada"
      : ROTULO_FLUXO[e];

const DESCRICAO_DEMANDA_NEGOCIAL =
  "Registre um pedido do negócio: corrigir um bug, criar uma funcionalidade ou melhorar o que já existe. Depois você analisa a demanda a partir dela.";
const DESCRICAO_CADASTRADA =
  "Analise uma demanda que já existe: do projeto ou uma issue do Jira, do GitHub ou do GitLab.";

const DESCRICAO_ESCOLHA = (e: Escolha) =>
  e === "demanda-negocial"
    ? DESCRICAO_DEMANDA_NEGOCIAL
    : e === "cadastrada"
      ? DESCRICAO_CADASTRADA
      : (DESCRICAO_FLUXO[e] ?? "");

const dobrar = (t: string) =>
  t
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();

/** Aplica situação, tipo e busca (id, título, demanda ou chave, sem acento nem caixa) à lista. */
export function filtrarAnalises(
  l: AnaliseResumo[],
  f: { situacao: EstadoAnalise | null; tipo: Fluxo | null; busca: string },
): AnaliseResumo[] {
  const termo = dobrar(f.busca.trim());
  return l.filter(
    (a) =>
      (!f.situacao || a.estado === f.situacao) &&
      (!f.tipo || a.fluxo === f.tipo) &&
      (!termo || dobrar(`${a.id} ${a.titulo} ${a.demanda ?? ""} ${a.chave ?? ""}`).includes(termo)),
  );
}

/** "há 20 min", "há 2 h", "ontem", "12 nov": quando a análise mudou pela última vez. */
export function quandoMudou(ms: number, agora = Date.now()): string {
  const min = Math.floor((agora - ms) / 60000);
  if (min < 1) return "agora";
  if (min < 60) return `há ${min} min`;
  const h = Math.floor(min / 60);
  const dia = new Date(ms);
  const hoje = new Date(agora);
  const ontem = new Date(agora);
  ontem.setDate(hoje.getDate() - 1);
  const mesmoDia = (a: Date, b: Date) => a.toDateString() === b.toDateString();
  if (mesmoDia(dia, hoje)) return `há ${h} h`;
  if (mesmoDia(dia, ontem)) return "ontem";
  return dia
    .toLocaleDateString("pt-BR", {
      day: "numeric",
      month: "short",
      ...(dia.getFullYear() !== hoje.getFullYear() ? { year: "numeric" } : {}),
    })
    .replace(".", "");
}

/**
 * Análises do projeto da barra de título (#326, #350): a lista é o conteúdo da tela,
 * com filtros por situação (com contagem), tipo e busca; "Nova análise" abre um painel
 * lateral com a escolha do tipo, explicada, e depois o formulário do tipo. A análise
 * aberta fica no lugar da lista (ou numa aba, pela navegação).
 */
export function TelaAnalise({
  api = apiAnaliseTauri,
  apiTerminal,
  criarTela,
  projetoInicial = "",
  perguntaInicial = "",
  situacaoInicial,
  novaInicial,
  apiConhecimento,
  aoPrimeiroUso,
  aberturaInicial,
  aoAbrirAnalise,
  aoAbrirDemanda,
  aoVoltar,
  aoMudarConversa,
}: Props) {
  const [situacao, setSituacao] = useState<Situacao | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [projeto, setProjeto] = useState(projetoInicial);
  const [aberta, setAberta] = useState<{ id: string; abertura: Abertura } | null>(
    aberturaInicial ?? null,
  );

  const recarregar = useCallback(() => {
    api
      .situacao()
      .then((s) => {
        setSituacao(s);
        setErro(null);
        setProjeto((atual) =>
          s.projetos.some((x) => x.id === atual) ? atual : (s.projetos[0]?.id ?? ""),
        );
      })
      .catch((e: unknown) => setErro(mensagemDe(e)));
  }, [api]);

  useEffect(recarregar, [recarregar]);

  const abrir = (p: string, id: string, abertura: Abertura) => {
    if (aoAbrirAnalise) aoAbrirAnalise(p, id, abertura);
    else setAberta({ id, abertura });
  };

  if (aberta && projeto) {
    return (
      <section className="tela-analise tela-analise-aberta" aria-label="Análise">
        <PainelAnalise
          key={aberta.id}
          api={api}
          projeto={projeto}
          analise={aberta.id}
          abertura={aberta.abertura}
          aoVoltar={aoVoltar ? () => aoVoltar(projeto) : () => setAberta(null)}
          apiTerminal={apiTerminal}
          criarTela={criarTela}
          aoAbrirAnalise={(id, abertura) => abrir(projeto, id, abertura)}
          aoMudarConversa={aoMudarConversa}
        />
      </section>
    );
  }

  const carregou = situacao?.base && situacao.projetos.length > 0;

  return (
    <section className="tela-analise page" aria-label="Análises">
      {!carregou && <h2>Análises</h2>}
      {erro && (
        <p className="error-text" role="alert">
          {erro}
        </p>
      )}
      {!situacao && !erro && <p className="muted">Carregando…</p>}
      {situacao && (!situacao.base || situacao.projetos.length === 0) && (
        <p>
          {!situacao.base
            ? "Falta escolher a pasta da base de conhecimento."
            : "Nenhum projeto cadastrado."}{" "}
          {aoPrimeiroUso && (
            <button className="btn" type="button" onClick={aoPrimeiroUso}>
              Abrir o primeiro uso
            </button>
          )}
        </p>
      )}
      {carregou && projeto && (
        <Projeto
          key={projeto}
          api={api}
          projeto={projeto}
          perguntaInicial={perguntaInicial}
          situacaoInicial={situacaoInicial}
          novaInicial={novaInicial}
          apiConhecimento={apiConhecimento}
          resumoProjeto={situacao.projetos.find((x) => x.id === projeto)}
          aoAbrir={(id, abertura) => abrir(projeto, id, abertura)}
          aoAbrirDemanda={aoAbrirDemanda ? (d) => aoAbrirDemanda(projeto, d) : undefined}
        />
      )}
    </section>
  );
}

function Projeto({
  api,
  projeto,
  perguntaInicial,
  situacaoInicial,
  novaInicial,
  apiConhecimento,
  resumoProjeto,
  aoAbrir,
  aoAbrirDemanda,
}: {
  api: ApiAnalise;
  projeto: string;
  perguntaInicial: string;
  situacaoInicial?: EstadoAnalise;
  novaInicial?: boolean;
  apiConhecimento?: ApiConhecimento;
  resumoProjeto?: ProjetoResumo;
  aoAbrir: (id: string, abertura: Abertura) => void;
  aoAbrirDemanda?: (demanda: string) => void;
}) {
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [falhaAmbiente, setFalhaAmbiente] = useState<string | null>(null);
  const [analises, setAnalises] = useState<AnaliseResumo[] | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [busca, setBusca] = useState("");
  const [situacao, setSituacao] = useState<EstadoAnalise | null>(situacaoInicial ?? null);
  const [tipo, setTipo] = useState<Fluxo | null>(null);
  // Chave no rastreador de cada demanda da base, para a análise ligada só à demanda.
  const [chaves, setChaves] = useState<Record<string, string>>({});
  // O painel "Nova análise": fechado (null), na escolha do tipo ou num tipo escolhido.
  // A pergunta sugerida abre direto o formulário da pergunta.
  const [novo, setNovo] = useState<Escolha | "escolha" | null>(
    perguntaInicial ? "perguntar" : novaInicial ? "escolha" : null,
  );

  const verificar = useCallback(() => {
    setFalhaAmbiente(null);
    api
      .verificarAmbiente(projeto)
      .then(setAmbiente)
      .catch((e: unknown) => setFalhaAmbiente(mensagemDe(e)));
  }, [api, projeto]);

  useEffect(verificar, [verificar]);

  useEffect(() => {
    let ativo = true;
    api
      .listarAnalises(projeto)
      .then((l) => ativo && setAnalises(l))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto]);

  useEffect(() => {
    if (!apiConhecimento) return;
    let ativo = true;
    apiConhecimento
      .listarDemandas(projeto)
      .then(
        (l) =>
          ativo &&
          setChaves(Object.fromEntries(l.flatMap((d) => (d.chave ? [[d.id, d.chave]] : [])))),
      )
      // Sem a lista, a linha mostra só o que a análise guardou.
      .catch(() => undefined);
    return () => {
      ativo = false;
    };
  }, [apiConhecimento, projeto]);

  const pronto = ambiente?.pronto === true;
  const total = analises?.length ?? 0;
  // A contagem de cada situação respeita o tipo e a busca, para o número bater com a lista.
  const semSituacao = analises ? filtrarAnalises(analises, { situacao: null, tipo, busca }) : [];
  const visiveis = situacao ? semSituacao.filter((a) => a.estado === situacao) : semSituacao;
  const filtrando = busca.trim() !== "" || situacao !== null || tipo !== null;
  const tiposNaLista = TIPOS.filter((t) => analises?.some((a) => a.fluxo === t));
  const nomeProjeto = resumoProjeto?.nome ?? projeto;

  return (
    <>
      <div className="analises-cabecalho">
        <div>
          <h2>Análises</h2>
          {analises && (
            <p className="muted">
              {total === 1 ? "1 análise" : `${total} análises`} em {nomeProjeto}
            </p>
          )}
        </div>
        <button
          className="btn btn-primary"
          type="button"
          aria-haspopup="dialog"
          aria-expanded={novo !== null}
          onClick={() => setNovo("escolha")}
        >
          Nova análise
        </button>
      </div>

      {falhaAmbiente && (
        <p className="error-text" role="alert">
          Não foi possível verificar se o agente está ligado a este projeto: {falhaAmbiente}
        </p>
      )}
      {ambiente && !pronto && novo === null && (
        <PrepararAmbiente api={api} projeto={projeto} ambiente={ambiente} aoConcluir={verificar} />
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}

      <section aria-label="Análises do projeto" className="analises-lista">
        {analises && analises.length > 0 && (
          <div className="analises-filtros">
            <div className="vincular-chips" role="group" aria-label="Situação da análise">
              <button
                type="button"
                className="vincular-chip"
                aria-pressed={situacao === null}
                onClick={() => setSituacao(null)}
              >
                Todas <span className="vincular-chip-n">{semSituacao.length}</span>
              </button>
              {SITUACOES.map((s) => (
                <button
                  key={s}
                  type="button"
                  className="vincular-chip"
                  aria-pressed={situacao === s}
                  onClick={() => setSituacao(situacao === s ? null : s)}
                >
                  {ROTULO_CHIP[s]}{" "}
                  <span className="vincular-chip-n">
                    {semSituacao.filter((a) => a.estado === s).length}
                  </span>
                </button>
              ))}
            </div>
            {tiposNaLista.length > 1 && (
              <>
                <label className="sr-only" htmlFor="tipo-analises">
                  Tipo de análise
                </label>
                <select
                  id="tipo-analises"
                  className="chip-select"
                  value={tipo ?? ""}
                  onChange={(e) => setTipo((e.target.value || null) as Fluxo | null)}
                >
                  <option value="">Tipo: todos</option>
                  {tiposNaLista.map((t) => (
                    <option key={t} value={t}>
                      {ROTULO_FLUXO[t]}
                    </option>
                  ))}
                </select>
              </>
            )}
            <label className="sr-only" htmlFor="busca-analises">
              Buscar análise
            </label>
            <input
              id="busca-analises"
              className="analises-busca"
              type="search"
              placeholder="Buscar por número, título ou demanda"
              value={busca}
              onChange={(e) => setBusca(e.target.value)}
            />
          </div>
        )}
        {analises?.length === 0 && (
          <p className="muted">Nenhuma análise ainda. Comece por “Nova análise”.</p>
        )}
        {analises && analises.length > 0 && visiveis.length === 0 && (
          <p className="muted">Nenhuma análise com este filtro.</p>
        )}
        {visiveis.length > 0 && (
          <div className="analises-tabela">
          <div className="analises-tabela-cabecalho" aria-hidden="true">
            <span>Análise</span>
            <span>Demanda</span>
            <span>Atualizada</span>
            <span>Situação</span>
            <span />
          </div>
          <ul>
            {visiveis.map((a) => (
              <li key={a.id}>
                <div className="analise-titulo">
                  <button
                    className="link-titulo"
                    type="button"
                    onClick={() => aoAbrir(a.id, { tipo: "reabrir" })}
                  >
                    {a.titulo}
                  </button>
                  <span className="muted">
                    <span className="id">{a.id}</span> · {ROTULO_FLUXO[a.fluxo]}
                  </span>
                </div>
                <span className="analise-demanda">
                  {(() => {
                    const chave = a.chave ?? (a.demanda ? chaves[a.demanda] : undefined);
                    if (!chave && !a.demanda) return <span className="muted">sem demanda</span>;
                    return (
                      <>
                        {chave && (
                          <span className="analise-chave" title={a.link ?? undefined}>
                            {chave}
                          </span>
                        )}
                        {a.demanda &&
                          (aoAbrirDemanda ? (
                            <button
                              type="button"
                              className="link"
                              aria-label={`Abrir a demanda ${a.demanda}`}
                              onClick={() => aoAbrirDemanda(a.demanda!)}
                            >
                              {a.demanda}
                            </button>
                          ) : (
                            <span className="muted">{a.demanda}</span>
                          ))}
                      </>
                    );
                  })()}
                </span>
                <span className="analise-quando muted" title={new Date(a.atualizadaMs).toLocaleString("pt-BR")}>
                  {quandoMudou(a.atualizadaMs)}
                </span>
                <span className="analise-situacao">
                  <span className="badge" data-tone={TOM[a.estado]}>
                    {ESTADO[a.estado][0].toUpperCase() + ESTADO[a.estado].slice(1)}
                  </span>
                </span>
                <span className="analise-acao">
                  <button
                    type="button"
                    className={a.estado === "concluida" ? "btn btn-primary" : "btn"}
                    aria-label={`${ACAO[a.estado]} ${a.id}`}
                    onClick={() => aoAbrir(a.id, { tipo: "reabrir" })}
                  >
                    {ACAO[a.estado]}
                  </button>
                </span>
              </li>
            ))}
          </ul>
          </div>
        )}
        {filtrando && analises && analises.length > 0 && (
          <button
            type="button"
            className="link"
            onClick={() => {
              setBusca("");
              setSituacao(null);
              setTipo(null);
            }}
          >
            Limpar filtros
          </button>
        )}
      </section>

      {novo !== null && (
        <PainelNova
          api={api}
          apiConhecimento={apiConhecimento}
          projeto={projeto}
          nomeProjeto={nomeProjeto}
          resumoProjeto={resumoProjeto}
          pronto={pronto}
          ambiente={ambiente}
          perguntaInicial={perguntaInicial}
          escolha={novo}
          aoEscolher={setNovo}
          aoFechar={() => setNovo(null)}
          aoAbrir={aoAbrir}
          aoAbrirDemanda={aoAbrirDemanda}
          aoPreparado={verificar}
        />
      )}
    </>
  );
}

const TOM: Record<EstadoAnalise, "attention" | "running" | "done" | undefined> = {
  criada: undefined,
  "em-andamento": "running",
  concluida: "attention",
  aprovada: "done",
};

/**
 * Painel lateral "Nova análise" (#350): primeiro a escolha do tipo, cada um com o que
 * faz; depois o formulário do tipo, com "Trocar" para voltar. O projeto é o da barra de
 * título. Esc ou tocar fora fecha.
 */
function PainelNova({
  api,
  apiConhecimento,
  projeto,
  nomeProjeto,
  resumoProjeto,
  pronto,
  ambiente,
  perguntaInicial,
  escolha,
  aoEscolher,
  aoFechar,
  aoAbrir,
  aoAbrirDemanda,
  aoPreparado,
}: {
  api: ApiAnalise;
  apiConhecimento?: ApiConhecimento;
  projeto: string;
  nomeProjeto: string;
  resumoProjeto?: ProjetoResumo;
  pronto: boolean;
  ambiente: AmbienteClaude | null;
  perguntaInicial: string;
  escolha: Escolha | "escolha";
  aoEscolher: (e: Escolha | "escolha") => void;
  aoFechar: () => void;
  aoAbrir: (id: string, abertura: Abertura) => void;
  aoAbrirDemanda?: (demanda: string) => void;
  /** O ambiente foi preparado pelo formulário da pergunta: relê a verificação. */
  aoPreparado: () => void;
}) {
  const painel = useRef<HTMLElement>(null);
  const [registrada, setRegistrada] = useState<DemandaVista | null>(null);
  const [referencia, setReferencia] = useState("");

  // Ao abrir e a cada passo, o foco vai para o primeiro controle do painel.
  useEffect(() => {
    // No formulário, o primeiro campo; na escolha do tipo, o primeiro botão.
    const corpo = painel.current?.querySelector(".painel-nova-corpo");
    const alvo =
      corpo?.querySelector<HTMLElement>("form input, form textarea, form select") ??
      corpo?.querySelector<HTMLElement>("button");
    alvo?.focus();
  }, [escolha, registrada]);

  const projetos = resumoProjeto ? [resumoProjeto] : [];
  const criada = (_p: string, c: AnaliseCriada) =>
    aoAbrir(c.analise.id, { tipo: "nova", promptInicial: c.promptInicial });
  const abrirExistente = (id: string) => aoAbrir(id, { tipo: "reabrir" });
  const voltar = () => {
    setRegistrada(null);
    aoEscolher("escolha");
  };

  let corpo: ReactNode;
  if (registrada) {
    corpo = (
      <div className="demanda-registrada" role="status">
        <p>
          <strong>Demanda {registrada.id} registrada.</strong> Ela já aparece em Demandas.
        </p>
        <p className="muted">{registrada.titulo}</p>
        <div className="acoes">
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => {
              setReferencia(registrada.id);
              setRegistrada(null);
              aoEscolher("cadastrada");
            }}
          >
            Analisar esta demanda
          </button>
          {aoAbrirDemanda && (
            <button type="button" className="btn" onClick={() => aoAbrirDemanda(registrada.id)}>
              Abrir a demanda
            </button>
          )}
          <button type="button" className="btn" onClick={aoFechar}>
            Fechar
          </button>
        </div>
      </div>
    );
  } else if (escolha === "escolha") {
    corpo = (
      <>
        <p className="painel-nova-pergunta">O que você quer fazer?</p>
        {apiConhecimento && (
          <button
            type="button"
            className="opcao-tipo opcao-destaque"
            aria-describedby="opcao-desc-demanda-negocial"
            onClick={() => aoEscolher("demanda-negocial")}
          >
            <strong>Demanda Negocial</strong>
            <span className="muted" id="opcao-desc-demanda-negocial" aria-hidden="true">
              {DESCRICAO_DEMANDA_NEGOCIAL}
            </span>
          </button>
        )}
        <p className="painel-nova-grupo">{apiConhecimento ? "Ou peça uma análise" : "Peça uma análise"}</p>
        <div className="opcoes-tipo" role="group" aria-label="Tipo de análise">
          {apiConhecimento && (
            <button
              type="button"
              className="opcao-tipo"
              aria-describedby="opcao-desc-cadastrada"
              onClick={() => aoEscolher("cadastrada")}
            >
              <strong>Analisar demanda cadastrada</strong>
              <span className="muted" id="opcao-desc-cadastrada" aria-hidden="true">
                {DESCRICAO_CADASTRADA}
              </span>
            </button>
          )}
          {TIPOS.filter((t) => t === "perguntar" || apiConhecimento).map((t) => (
            <button
              key={t}
              type="button"
              className="opcao-tipo"
              aria-describedby={`opcao-desc-${t}`}
              onClick={() => aoEscolher(t)}
            >
              <strong>{ROTULO_FLUXO[t]}</strong>
              <span className="muted" id={`opcao-desc-${t}`} aria-hidden="true">
                {DESCRICAO_FLUXO[t]}
              </span>
            </button>
          ))}
        </div>
      </>
    );
  } else {
    let form: ReactNode = null;
    if (escolha === "perguntar") {
      form = (
        <FormPergunta
          api={api}
          projeto={projeto}
          pronto={pronto}
          ambiente={ambiente}
          inicial={perguntaInicial}
          aoAbrir={aoAbrir}
          aoCancelar={aoFechar}
          aoPreparado={aoPreparado}
        />
      );
    } else if (apiConhecimento && escolha === "demanda-negocial") {
      form = (
        <FormDemandaNegocial
          api={apiConhecimento}
          projeto={projeto}
          aoRegistrar={setRegistrada}
          aoCancelar={aoFechar}
          aoAbrirAnalise={abrirExistente}
          aoAnalisarExistente={(ref) => {
            setReferencia(ref);
            aoEscolher("cadastrada");
          }}
        />
      );
    } else if (apiConhecimento && escolha === "cadastrada") {
      form = (
        <NovaAnalise
          key={`cadastrada:${referencia}`}
          api={apiConhecimento}
          fluxo="impacto"
          cadastrada
          referenciaInicial={referencia}
          semTitulo
          projetos={projetos}
          aoCancelar={aoFechar}
          aoCriar={criada}
          aoAbrirAnalise={abrirExistente}
        />
      );
    } else if (apiConhecimento && escolha === "reuniao") {
      form = (
        <NovaReuniao
          api={apiConhecimento}
          projetos={projetos}
          semTitulo
          aoCancelar={aoFechar}
          aoCriar={criada}
          aoAbrirAnalise={abrirExistente}
        />
      );
    } else if (apiConhecimento && (escolha === "historias" || escolha === "cenarios")) {
      form = (
        <NovaHistorias
          key={escolha}
          fluxo={escolha}
          api={apiConhecimento}
          projetos={projetos}
          semTitulo
          aoCancelar={aoFechar}
          aoCriar={criada}
        />
      );
    } else if (apiConhecimento && escolha !== "demanda-negocial" && escolha !== "cadastrada") {
      form = (
        <NovaAnalise
          key={escolha}
          api={apiConhecimento}
          fluxo={escolha}
          semTitulo
          projetos={projetos}
          aoCancelar={aoFechar}
          aoCriar={criada}
          aoAbrirAnalise={abrirExistente}
        />
      );
    }
    corpo = (
      <>
        <div className="tipo-escolhido">
          <div>
            <span className="muted">Tipo</span>
            <strong>{NOME_ESCOLHA(escolha)}</strong>
            <span className="muted">{DESCRICAO_ESCOLHA(escolha)}</span>
          </div>
          <button type="button" className="btn" onClick={voltar}>
            Trocar
          </button>
        </div>
        {form}
      </>
    );
  }

  return (
    <>
      <div className="painel-nova-scrim" aria-hidden="true" onClick={aoFechar} />
      <aside
        ref={painel}
        className="painel-nova"
        role="dialog"
        aria-modal="true"
        aria-labelledby="painel-nova-titulo"
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            aoFechar();
          }
        }}
      >
        <header className="painel-nova-topo">
          <div>
            <h3 id="painel-nova-titulo">Nova análise</h3>
            <p className="muted">Em {nomeProjeto}, o projeto da barra de título.</p>
          </div>
          <button type="button" className="btn-icone" aria-label="Fechar" onClick={aoFechar}>
            ✕
          </button>
        </header>
        <div className="painel-nova-corpo">{corpo}</div>
      </aside>
    </>
  );
}

function FormPergunta({
  api,
  projeto,
  pronto,
  ambiente,
  inicial,
  aoAbrir,
  aoCancelar,
  aoPreparado,
}: {
  api: ApiAnalise;
  projeto: string;
  pronto: boolean;
  ambiente: AmbienteClaude | null;
  inicial: string;
  aoAbrir: (id: string, abertura: Abertura) => void;
  aoCancelar: () => void;
  aoPreparado: () => void;
}) {
  const [entrada, setEntrada] = useState(inicial);
  const [titulo, setTitulo] = useState("");
  const [criando, setCriando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const motivo =
    ambiente && !pronto
      ? "Para perguntar, ligue o agente a este projeto (acima)."
      : !entrada.trim()
        ? "Para perguntar, escreva a pergunta."
        : null;

  const perguntar = (ev: FormEvent) => {
    ev.preventDefault();
    if (!entrada.trim()) return;
    setCriando(true);
    setFalha(null);
    api
      .criarAnalise(projeto, entrada.trim(), titulo.trim() || undefined)
      .then((criada) =>
        aoAbrir(criada.analise.id, { tipo: "nova", promptInicial: criada.promptInicial }),
      )
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setCriando(false);
      });
  };

  return (
    <>
      {ambiente && !pronto && (
        <PrepararAmbiente api={api} projeto={projeto} ambiente={ambiente} aoConcluir={aoPreparado} />
      )}
      <form className="nova-pergunta nova-analise" onSubmit={perguntar} aria-label="Nova pergunta">
        <label>
          Pergunta
          <textarea
            value={entrada}
            onChange={(e) => setEntrada(e.target.value)}
            rows={5}
            placeholder="Ex.: Como é calculada a multa por atraso de pagamento?"
          />
        </label>
        <label>
          <span>
            Título <span className="muted">(opcional)</span>
          </span>
          <input
            value={titulo}
            onChange={(e) => setTitulo(e.target.value)}
            placeholder="Se ficar em branco, sai da pergunta"
          />
        </label>
        <div className="rodape-form">
          {motivo && (
            <p className="motivo-desabilitado" id="pergunta-motivo">
              {motivo}
            </p>
          )}
          <div className="acoes">
            <button className="btn" type="button" onClick={aoCancelar}>
              Cancelar
            </button>
            <button
              className="btn btn-primary"
              type="submit"
              disabled={!pronto || criando || !entrada.trim()}
              aria-describedby={motivo ? "pergunta-motivo" : undefined}
            >
              Perguntar
            </button>
          </div>
        </div>
      </form>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </>
  );
}
