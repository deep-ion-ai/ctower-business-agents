import { useCallback, useEffect, useState, type FormEvent } from "react";
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
  /** Projeto escolhido ao abrir (ex.: o recém-cadastrado no primeiro uso). */
  projetoInicial?: string;
  /** Abre a lista já filtrada por situação (link da Visão geral). */
  situacaoInicial?: EstadoAnalise;
  /** Abre o painel "Nova análise" (ação global do menu e da Visão geral). */
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
  /** "← Análises" fora desta tela (na navegação, a aba da lista). Sem ele, volta aqui. */
  aoVoltar?: (projeto: string) => void;
  /** A conversa da análise aberta (a navegação confirma antes de fechar a aba). */
  aoMudarConversa?: (estado: EstadoConversa) => void;
}

const ESTADO = ROTULO_ESTADO_ANALISE;
const SITUACOES: EstadoAnalise[] = ["criada", "em-andamento", "concluida", "aprovada"];
const TIPOS: Fluxo[] = [
  "perguntar",
  "explicar",
  "impacto",
  "reuniao",
  "revisar",
  "historias",
  "cenarios",
];

const dobrar = (t: string) =>
  t
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();

/** Aplica situação, tipo e busca (id ou título, sem acento nem caixa) à lista. */
export function filtrarAnalises(
  l: AnaliseResumo[],
  f: { situacao: EstadoAnalise | null; tipo: Fluxo | null; busca: string },
): AnaliseResumo[] {
  const termo = dobrar(f.busca.trim());
  return l.filter(
    (a) =>
      (!f.situacao || a.estado === f.situacao) &&
      (!f.tipo || a.fluxo === f.tipo) &&
      (!termo || dobrar(`${a.id} ${a.titulo}`).includes(termo)),
  );
}

/**
 * Pergunta ao sistema, do início ao documento: escolher o projeto (preparando o
 * ambiente com consentimento, se faltar), perguntar, acompanhar a análise ao vivo e
 * reabrir as anteriores. A base e o cadastro ficam no primeiro uso.
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

  return (
    <section className="tela-analise page" aria-label="Análises">
      <h2>Análises</h2>
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
      {situacao?.base && situacao.projetos.length > 0 && (
        <>
          <label>
            Projeto{" "}
            <select value={projeto} onChange={(e) => setProjeto(e.target.value)}>
              {situacao.projetos.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.nome}
                </option>
              ))}
            </select>
          </label>
          {projeto && (
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
            />
          )}
        </>
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
}: {
  api: ApiAnalise;
  projeto: string;
  perguntaInicial: string;
  situacaoInicial?: EstadoAnalise;
  novaInicial?: boolean;
  apiConhecimento?: ApiConhecimento;
  resumoProjeto?: ProjetoResumo;
  aoAbrir: (id: string, abertura: Abertura) => void;
}) {
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [falhaAmbiente, setFalhaAmbiente] = useState<string | null>(null);
  const [analises, setAnalises] = useState<AnaliseResumo[] | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [busca, setBusca] = useState("");
  const [situacao, setSituacao] = useState<EstadoAnalise | null>(situacaoInicial ?? null);
  const [tipo, setTipo] = useState<Fluxo | null>(null);
  // A pergunta sugerida abre direto o formulário da pergunta.
  const [novo, setNovo] = useState<Fluxo | "escolha" | null>(
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

  const pronto = ambiente?.pronto === true;
  const visiveis = analises ? filtrarAnalises(analises, { situacao, tipo, busca }) : [];
  const filtrando = busca.trim() !== "" || situacao !== null || tipo !== null;
  const tiposNaLista = TIPOS.filter((t) => analises?.some((a) => a.fluxo === t));
  const criada = (_p: string, c: AnaliseCriada) =>
    aoAbrir(c.analise.id, { tipo: "nova", promptInicial: c.promptInicial });
  const projetos = resumoProjeto ? [resumoProjeto] : [];

  return (
    <>
      {falhaAmbiente && (
        <p className="error-text" role="alert">
          Não foi possível verificar se o agente está ligado a este projeto: {falhaAmbiente}
        </p>
      )}
      {ambiente && !pronto && (
        <PrepararAmbiente api={api} projeto={projeto} ambiente={ambiente} aoConcluir={verificar} />
      )}

      <div className="titulo-com-acao">
        <button
          className="btn btn-primary"
          type="button"
          aria-expanded={novo !== null}
          onClick={() => setNovo(novo === null ? "escolha" : null)}
        >
          Nova análise
        </button>
      </div>
      {novo !== null && (
        <section aria-label="Nova análise" className="fluxos">
          <div className="botoes-fluxo" role="group" aria-label="Tipo de análise">
            {TIPOS.filter((t) => t === "perguntar" || apiConhecimento).map((t) => (
              <button
                key={t}
                type="button"
                className="btn"
                aria-pressed={novo === t}
                onClick={() => setNovo(novo === t ? "escolha" : t)}
              >
                {ROTULO_FLUXO[t]}
              </button>
            ))}
          </div>
          {novo === "perguntar" && (
            <FormPergunta
              api={api}
              projeto={projeto}
              pronto={pronto}
              ambiente={ambiente}
              inicial={perguntaInicial}
              aoAbrir={aoAbrir}
            />
          )}
          {apiConhecimento && novo === "reuniao" && (
            <NovaReuniao
              api={apiConhecimento}
              projetos={projetos}
              aoCancelar={() => setNovo("escolha")}
              aoCriar={criada}
            />
          )}
          {apiConhecimento && (novo === "historias" || novo === "cenarios") && (
            <NovaHistorias
              key={novo}
              fluxo={novo}
              api={apiConhecimento}
              projetos={projetos}
              aoCancelar={() => setNovo("escolha")}
              aoCriar={criada}
            />
          )}
          {apiConhecimento &&
            novo !== "escolha" &&
            novo !== "perguntar" &&
            novo !== "reuniao" &&
            novo !== "historias" &&
            novo !== "cenarios" && (
              <NovaAnalise
                key={novo}
                api={apiConhecimento}
                fluxo={novo}
                projetos={projetos}
                aoCancelar={() => setNovo("escolha")}
                aoCriar={criada}
              />
            )}
        </section>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}

      <section aria-label="Análises do projeto">
        <h3>Análises</h3>
        {analises && analises.length > 0 && (
          <div className="demandas-filtros">
            <label className="sr-only" htmlFor="busca-analises">
              Buscar análise
            </label>
            <input
              id="busca-analises"
              type="search"
              placeholder="Número ou título"
              value={busca}
              onChange={(e) => setBusca(e.target.value)}
            />
            <div className="vincular-chips" role="group" aria-label="Situação da análise">
              {SITUACOES.map((s) => (
                <button
                  key={s}
                  type="button"
                  className="vincular-chip"
                  aria-pressed={situacao === s}
                  onClick={() => setSituacao(situacao === s ? null : s)}
                >
                  {ESTADO[s][0].toUpperCase() + ESTADO[s].slice(1)}
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
                  value={tipo ?? ""}
                  onChange={(e) => setTipo((e.target.value || null) as Fluxo | null)}
                >
                  <option value="">Todos os tipos</option>
                  {tiposNaLista.map((t) => (
                    <option key={t} value={t}>
                      {ROTULO_FLUXO[t]}
                    </option>
                  ))}
                </select>
              </>
            )}
          </div>
        )}
        {analises?.length === 0 && <p className="muted">Nenhuma análise ainda.</p>}
        {analises && analises.length > 0 && visiveis.length === 0 && (
          <p className="muted">Nenhuma análise com este filtro.</p>
        )}
        {visiveis.length > 0 && (
          <ul className="lista-analises">
            {visiveis.map((a) => (
              <li key={a.id}>
                <button
                  className="link"
                  type="button"
                  onClick={() => aoAbrir(a.id, { tipo: "reabrir" })}
                >
                  {a.id}: {a.titulo}
                </button>{" "}
                <span className="muted">{ROTULO_FLUXO[a.fluxo]}</span>{" "}
                <span className="badge" data-tone={TOM[a.estado]}>
                  {ESTADO[a.estado]}
                </span>
              </li>
            ))}
          </ul>
        )}
        {filtrando && analises && analises.length > 0 && (
          <button
            type="button"
            className="btn"
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
    </>
  );
}

const TOM: Record<EstadoAnalise, "attention" | "running" | "done" | undefined> = {
  criada: undefined,
  "em-andamento": "running",
  concluida: "attention",
  aprovada: "done",
};

function FormPergunta({
  api,
  projeto,
  pronto,
  ambiente,
  inicial,
  aoAbrir,
}: {
  api: ApiAnalise;
  projeto: string;
  pronto: boolean;
  ambiente: AmbienteClaude | null;
  inicial: string;
  aoAbrir: (id: string, abertura: Abertura) => void;
}) {
  const [entrada, setEntrada] = useState(inicial);
  const [titulo, setTitulo] = useState("");
  const [criando, setCriando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

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
      <form className="nova-pergunta" onSubmit={perguntar} aria-label="Nova pergunta">
        <label>
          Pergunta
          <textarea
            value={entrada}
            onChange={(e) => setEntrada(e.target.value)}
            rows={3}
            placeholder="Ex.: Como é calculada a multa por atraso de pagamento?"
          />
        </label>
        <label>
          Título (opcional)
          <input value={titulo} onChange={(e) => setTitulo(e.target.value)} />
        </label>
        <button className="btn" type="submit" disabled={!pronto || criando || !entrada.trim()}>
          Perguntar
        </button>
        {!pronto && ambiente && (
          <span className="muted"> Ligue o agente a este projeto para perguntar.</span>
        )}
      </form>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </>
  );
}
