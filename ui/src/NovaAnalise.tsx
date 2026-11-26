import { useCallback, useEffect, useState, type FormEvent } from "react";
import {
  mensagemDe,
  ROTULO_ESTADO_ANALISE,
  type AmbienteClaude,
  type AnaliseCriada,
  type Conducao,
  type EstadoAnalise,
  type Fluxo,
} from "./analise";
import { lembrarConducao, ultimaConducao } from "./conducao";
import { ROTULO_FLUXO, type ApiConhecimento } from "./conhecimento";
import { EscolhaConducao } from "./EscolhaConducao";
import { Alerta, Selo, TituloSecao, type TomSelo } from "./componentes";
import type { DemandaVista } from "./demandas";
import type { IssueDoProjeto, OrigemIssue } from "./issuesProjeto";
import { PrepararAmbiente } from "./PrepararAmbiente";
import type { ProjetoResumo } from "./terminal";

/** Rótulo e exemplo da entrada de cada fluxo (spec, seções 4.3 a 4.5). */
const ENTRADA: Record<Fluxo, { rotulo: string; exemplo: string }> = {
  perguntar: {
    rotulo: "Pergunta",
    exemplo: "Ex.: Como é calculada a multa por atraso de pagamento?",
  },
  explicar: {
    rotulo: "Funcionalidade",
    exemplo: "Ex.: Cancelamento de fatura",
  },
  impacto: {
    rotulo: "Texto da demanda",
    exemplo: "Ex.: Permitir parcelamento da fatura em até 3 vezes para clientes pessoa jurídica.",
  },
  // A reunião tem formulário próprio (NovaReuniao), com o transcript.
  reuniao: { rotulo: "Transcript", exemplo: "" },
  revisar: {
    rotulo: "Requisito ou história a revisar",
    exemplo: "Cole o texto como o autor escreveu, com os critérios de aceite.",
  },
  // Histórias e cenários têm formulário próprio (NovaHistorias), com a análise de origem.
  historias: { rotulo: "Pedido", exemplo: "" },
  cenarios: { rotulo: "Pedido", exemplo: "" },
  // O manual do usuário nasce no painel da funcionalidade (#272).
  manual: { rotulo: "Funcionalidade", exemplo: "" },
  // O protótipo de telas parte de uma análise de origem aprovada (#381); o
  // formulário próprio vem com as entradas na UI (#379).
  prototipo: { rotulo: "Pedido", exemplo: "" },
};

/** Demanda do projeto (`D-0012`). */
const DEMANDA = /^D-\d+$/i;

/**
 * De onde vem a referência digitada no campo "Issue ou demanda": a demanda do projeto
 * (`D-0012`), uma chave do Jira (`PROJ-123`), o número da issue no rastreador do projeto
 * (`#142`) ou o link de uma issue do GitHub ou do GitLab. `null` quando não reconhece.
 */
export function origemDaReferencia(texto: string): string | null {
  const v = texto.trim();
  if (!v) return null;
  if (DEMANDA.test(v)) return "demanda do projeto";
  if (/^https?:\/\/[^/]*github\.com\//i.test(v)) return "issue do GitHub";
  if (/^https?:\/\/[^/]*gitlab[^/]*\//i.test(v)) return "issue do GitLab";
  if (/^https?:\/\/[^/]*(atlassian\.net|jira)[^/]*\//i.test(v) || /^[A-Z][A-Z0-9]+-\d+$/.test(v))
    return "issue do Jira";
  if (/^#?\d+$/.test(v)) return "issue do rastreador do projeto";
  return null;
}

/**
 * A chave como se compara: sem `#`, em maiúsculas; de um link, o número da issue
 * (GitHub, GitLab) ou a chave do Jira (`/browse/PROJ-12`).
 */
export function chaveNormalizada(texto: string): string {
  const v = texto.trim();
  const jira = v.match(/\/browse\/([A-Z][A-Z0-9]+-\d+)/i);
  if (jira) return jira[1].toUpperCase();
  const num = v.match(/^https?:\/\/.*\/(?:issues|-\/issues|work_items)\/(\d+)/i);
  if (num) return num[1];
  return v.replace(/^#/, "").toUpperCase();
}

const NOME_ORIGEM: Record<OrigemIssue, string> = {
  github: "issue do GitHub",
  gitlab: "issue do GitLab",
  jira: "issue do Jira",
};

/** Análise que a demanda (ou a issue) já tem. */
export interface AnaliseExistente {
  id: string;
  titulo: string;
  fluxo: Fluxo;
  estado: EstadoAnalise;
}

/** A demanda que a referência digitada aponta, com as análises que já tem. */
export type DemandaResolvida =
  | {
      achada: true;
      titulo: string;
      origem: string;
      demanda: string | null;
      /** A issue do rastreador, quando a referência é uma: para criar a demanda dela. */
      issue: { origem: OrigemIssue; chave: string; aberta: boolean } | null;
      analises: AnaliseExistente[];
    }
  | { achada: false };

/**
 * Procura a referência nas demandas do projeto (pelo `D-NNNN` ou pela chave no
 * rastreador) e nas issues do rastreador. Traz as análises já ligadas a ela, a mais
 * recente primeiro: as da demanda da base e as da issue, que nascem ligadas só pela
 * chave quando criadas direto da issue (#350). O analista decide se continua uma.
 */
export async function resolverDemanda(
  api: ApiConhecimento,
  projeto: string,
  referencia: string,
  demandas: DemandaVista[],
): Promise<DemandaResolvida> {
  const ref = referencia.trim();
  const chave = chaveNormalizada(ref);
  let titulo: string | null = null;
  let demanda: string | null = null;
  let origem = origemDaReferencia(ref) ?? "issue";
  let issueAchada: { origem: OrigemIssue; chave: string; aberta: boolean } | null = null;
  const ids: string[] = [];
  const daBase = DEMANDA.test(ref)
    ? demandas.find((d) => d.id.toUpperCase() === chave)
    : demandas.find((d) => d.chave && chaveNormalizada(d.chave) === chave);
  if (daBase) {
    titulo = daBase.titulo;
    demanda = daBase.id;
    if (DEMANDA.test(ref)) origem = "demanda do projeto";
  }
  // Pela demanda da base com chave no rastreador, a issue dela também conta: as análises
  // criadas direto da issue ficam ligadas só pela chave.
  const chaveDaIssue = DEMANDA.test(ref) ? (daBase?.chave ? chaveNormalizada(daBase.chave) : null) : chave;
  if (chaveDaIssue && api.issues) {
    const lista = await api.issues
      .listar(projeto, { busca: chaveDaIssue, estado: "todas" })
      .catch(() => null);
    const issue = lista?.issues.find(
      (i) => chaveNormalizada(i.chave) === chaveDaIssue || (!DEMANDA.test(ref) && i.url === ref),
    );
    if (issue) {
      titulo = titulo ?? issue.titulo;
      demanda = demanda ?? issue.demanda;
      if (!DEMANDA.test(ref)) origem = NOME_ORIGEM[issue.origem];
      issueAchada = { origem: issue.origem, chave: issue.chave, aberta: issue.estado === "aberta" };
      ids.push(...(issue.analises ?? (issue.analise ? [issue.analise] : [])));
    }
  }
  if (titulo === null) return { achada: false };
  if (demanda) {
    const painel = await api.painelDemanda(projeto, demanda).catch(() => null);
    for (const i of painel?.linhaDoTempo ?? []) if (i.tipo === "analise") ids.push(i.id);
  }
  let analises: AnaliseExistente[] = [];
  if (ids.length > 0) {
    const todas = await api.listarAnalises(projeto).catch(() => []);
    const ligadas = new Set(ids);
    // A lista do projeto vem da mais recente para a mais antiga.
    analises = todas
      .filter((a) => ligadas.has(a.id))
      .map((a) => ({ id: a.id, titulo: a.titulo, fluxo: a.fluxo, estado: a.estado }));
  }
  return { achada: true, titulo, origem, demanda, issue: issueAchada, analises };
}

const TOM_ESTADO: Record<EstadoAnalise, TomSelo> = {
  criada: "neutro",
  "em-andamento": "andamento",
  concluida: "atencao",
  aprovada: "concluido",
};

/**
 * O título e as análises da demanda que `referencia` aponta, um instante depois de
 * digitar (#350). `null` sem referência reconhecida (ou sem `api`); `"buscando"`
 * enquanto procura.
 */
export function useDemandaResolvida(
  api: ApiConhecimento | undefined,
  projeto: string,
  referencia: string,
  demandas: DemandaVista[],
  ligado = true,
): DemandaResolvida | "buscando" | null {
  const [resolvida, setResolvida] = useState<DemandaResolvida | "buscando" | null>(null);
  useEffect(() => {
    const ref = referencia.trim();
    if (!api || !ligado || !projeto || !origemDaReferencia(ref)) {
      setResolvida(null);
      return;
    }
    let ativo = true;
    setResolvida("buscando");
    const espera = setTimeout(() => {
      resolverDemanda(api, projeto, ref, demandas)
        .then((r) => ativo && setResolvida(r))
        .catch(() => ativo && setResolvida({ achada: false }));
    }, 350);
    return () => {
      ativo = false;
      clearTimeout(espera);
    };
  }, [api, projeto, referencia, demandas, ligado]);
  return resolvida;
}

/** A demanda achada: o título e as análises que já tem, para continuar uma (#350). */
export function DemandaAchada({
  achada,
  aoAbrirAnalise,
  aviso,
}: {
  achada: Extract<DemandaResolvida, { achada: true }>;
  aoAbrirAnalise?: (id: string) => void;
  /** Texto no lugar do padrão ("Continue por ela ou inicie uma nova abaixo"). */
  aviso?: string;
}) {
  const existentes = achada.analises;
  const continua = (e: EstadoAnalise) => e === "em-andamento" || e === "criada";
  return (
    <div className="demanda-achada">
      <strong>{achada.titulo}</strong>
      {aviso && <p className="muted">{aviso}</p>}
      {existentes.length > 0 ? (
        <>
          {!aviso && (
            <p className="muted">
              {existentes.length === 1
                ? "Esta demanda já tem uma análise. Continue por ela ou inicie uma nova abaixo."
                : `Esta demanda já tem ${existentes.length} análises. Continue por uma delas ou inicie uma nova abaixo.`}
            </p>
          )}
          <ul className="analises-existentes" aria-label="Análises desta demanda">
            {existentes.map((a) => (
              <li key={a.id}>
                <div>
                  <span>{a.titulo}</span>
                  <span className="muted">
                    <span className="id">{a.id}</span> · {ROTULO_FLUXO[a.fluxo]}
                  </span>
                </div>
                <Selo tom={TOM_ESTADO[a.estado]}>
                  {ROTULO_ESTADO_ANALISE[a.estado][0].toUpperCase() + ROTULO_ESTADO_ANALISE[a.estado].slice(1)}
                </Selo>
                {aoAbrirAnalise && (
                  <button
                    type="button"
                    className={continua(a.estado) ? "btn btn-primary" : "btn"}
                    aria-label={`${continua(a.estado) ? "Continuar" : "Abrir"} ${a.id}`}
                    onClick={() => aoAbrirAnalise(a.id)}
                  >
                    {continua(a.estado) ? "Continuar" : "Abrir"}
                  </button>
                )}
              </li>
            ))}
          </ul>
        </>
      ) : (
        !aviso && <p className="muted">Esta demanda ainda não tem análise.</p>
      )}
    </div>
  );
}

/** Uma sugestão do campo "Issue ou demanda": demanda do projeto ou issue do rastreador. */
interface Sugestao {
  chave: string;
  titulo: string;
}

/**
 * Demandas do projeto, sugeridas no campo "Issue ou demanda". Vazia sem `api`, sem
 * projeto ou desligada; sem a lista, a demanda continua valendo digitada.
 */
export function useDemandasDoProjeto(
  api: ApiConhecimento | undefined,
  projeto: string,
  ligado = true,
): DemandaVista[] {
  const [demandas, setDemandas] = useState<DemandaVista[]>([]);
  useEffect(() => {
    if (!api || !ligado || !projeto) return;
    let ativo = true;
    setDemandas([]);
    api
      .listarDemandas(projeto)
      .then((l) => ativo && setDemandas(l))
      .catch(() => undefined);
    return () => {
      ativo = false;
    };
  }, [api, projeto, ligado]);
  return demandas;
}

/** A demanda achada em `resolvida`, ou `null` (sem referência, procurando, não achada). */
export function demandaAchada(
  resolvida: DemandaResolvida | "buscando" | null,
): Extract<DemandaResolvida, { achada: true }> | null {
  return resolvida !== null && resolvida !== "buscando" && resolvida.achada ? resolvida : null;
}

/**
 * Campo "Issue ou demanda" (#323, #350): sugere as demandas abertas do projeto e as
 * issues abertas do rastreador, diz de onde vem o que foi digitado e, com a demanda
 * achada, mostra o título e as análises que ela já tem (#359: também na pergunta).
 */
export function CampoIssueOuDemanda({
  api,
  projeto,
  referencia,
  aoMudar,
  demandas,
  resolvida,
  cadastrada = false,
  ajudaVazia,
  aoAbrirAnalise,
  id = "nova-analise-referencia",
}: {
  api: ApiConhecimento;
  projeto: string;
  referencia: string;
  aoMudar: (referencia: string) => void;
  demandas: DemandaVista[];
  resolvida: DemandaResolvida | "buscando" | null;
  /** Obrigatório ("Demanda ou issue"), na análise da demanda cadastrada. */
  cadastrada?: boolean;
  /** Ajuda com o campo vazio, fora da demanda cadastrada. */
  ajudaVazia: string;
  aoAbrirAnalise?: (id: string) => void;
  id?: string;
}) {
  // Issues abertas do rastreador (#323), filtradas pelo que foi digitado; o que não
  // está na lista continua valendo digitado (o app busca a issue na hora).
  const [issues, setIssues] = useState<IssueDoProjeto[]>([]);
  const [buscando, setBuscando] = useState(false);
  useEffect(() => {
    if (!api.issues || !projeto || !buscando) {
      setIssues([]);
      return;
    }
    let ativo = true;
    api.issues
      .listar(projeto, { busca: referencia.trim(), estado: "abertas" })
      .then((l) => ativo && setIssues(l.issues.slice(0, 6)))
      // Sem a lista (sem rede, sem rastreador), o campo segue como texto livre.
      .catch(() => ativo && setIssues([]));
    return () => {
      ativo = false;
    };
  }, [api, projeto, referencia, buscando]);

  const ref = referencia.trim();
  const demandaEscolhida = DEMANDA.test(ref)
    ? demandas.find((d) => d.id.toUpperCase() === ref.toUpperCase())
    : undefined;
  const achada = demandaAchada(resolvida);
  const termo = ref.toLowerCase();
  const sugestoes: Sugestao[] = buscando
    ? [
        ...demandas
          .filter((d) => d.status === "aberta")
          .filter((d) => !termo || `${d.id} ${d.titulo}`.toLowerCase().includes(termo))
          .slice(0, 4)
          .map((d) => ({ chave: d.id, titulo: d.titulo })),
        ...issues.map((i) => ({ chave: i.chave, titulo: i.titulo })),
      ].filter((s) => s.chave.toLowerCase() !== termo)
    : [];
  const origem = origemDaReferencia(referencia);
  const ajuda = resolvida === "buscando"
    ? "Procurando a demanda…"
    : resolvida !== null && !achada
      ? `Não achei ${ref} nas demandas nem nas issues do projeto. Ao iniciar, o app busca a issue no rastreador.`
      : achada
        ? `Reconhecida: ${achada.origem}${achada.demanda && achada.origem !== "demanda do projeto" ? `, demanda ${achada.demanda}` : ""}.`
        : origem
    ? `Reconhecida: ${origem}${demandaEscolhida ? ` (${demandaEscolhida.titulo})` : ""}.`
    : ref
      ? "Não reconhecida. Use D-0012, PROJ-123, #142 ou o link da issue."
      : cadastrada
        ? "Aceita a demanda do projeto (D-0012), a chave do Jira (PROJ-123), o número da issue (#142) ou o link do GitHub ou do GitLab."
        : ajudaVazia;

  return (
    <div
      className="busca-issue"
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) setBuscando(false);
      }}
    >
      <label htmlFor={id}>
        {cadastrada ? "Demanda ou issue" : "Issue ou demanda"}{" "}
        <span className="muted">{cadastrada ? "(obrigatório)" : "(opcional)"}</span>
      </label>
      <input
        id={id}
        value={referencia}
        onChange={(e) => {
          aoMudar(e.target.value);
          setBuscando(true);
        }}
        onFocus={() => setBuscando(true)}
        placeholder={cadastrada ? "D-0012, PROJ-123, #142 ou o link da issue" : "Busque pelo número ou título"}
        spellCheck={false}
        autoComplete="off"
        aria-required={cadastrada}
        aria-describedby={`${id}-ajuda`}
        data-invalido={cadastrada && !ref ? "" : undefined}
      />
      {sugestoes.length > 0 && (
        <ul
          className="busca-issue-opcoes"
          aria-label="Demandas e issues do projeto"
          // No WebKit (macOS) o clique não dá foco ao botão: sem isto, o campo perde
          // o foco, a lista fecha no blur e o clique na sugestão se perde.
          onMouseDown={(e) => e.preventDefault()}
        >
          {sugestoes.map((o) => (
            <li key={o.chave}>
              <button
                type="button"
                onClick={() => {
                  aoMudar(o.chave);
                  setBuscando(false);
                }}
              >
                <span className="id">{o.chave}</span>
                <span>{o.titulo}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      <span
        id={`${id}-ajuda`}
        className="ajuda-campo"
        data-tom={
          resolvida === "buscando"
            ? undefined
            : (resolvida !== null ? achada : origem)
              ? "ok"
              : ref
                ? "atencao"
                : undefined
        }
        aria-live="polite"
      >
        {ajuda}
      </span>
      {achada && <DemandaAchada achada={achada} aoAbrirAnalise={aoAbrirAnalise} />}
    </div>
  );
}

/**
 * Formulário de uma análise nova: a entrada do fluxo, a issue ou a demanda a que ligar
 * e, se faltar, o preparo do ambiente do Claude com consentimento. O projeto vem de
 * fora (o seletor da barra de título); só com mais de um projeto em `projetos` o
 * formulário mostra a escolha. Pelo painel da demanda, `demandaFixa` liga a análise
 * àquela demanda, sem escolha de projeto, issue nem demanda.
 *
 * Com `cadastrada`, é "Analisar demanda cadastrada" (#350): a demanda ou a issue é
 * obrigatória e a entrada vira "O que observar" (opcional).
 */
export function NovaAnalise({
  api,
  fluxo,
  projetos,
  demandaFixa,
  cadastrada = false,
  referenciaInicial = "",
  semTitulo = false,
  aoCriar,
  aoCancelar,
  aoAbrirAnalise,
}: {
  api: ApiConhecimento;
  fluxo: Fluxo;
  projetos: ProjetoResumo[];
  /** Demanda (`D-0001`) do primeiro projeto de `projetos`. */
  demandaFixa?: string;
  /** "Analisar demanda cadastrada": a demanda ou a issue é obrigatória. */
  cadastrada?: boolean;
  /** Issue ou demanda já preenchida (ex.: a demanda recém-registrada). */
  referenciaInicial?: string;
  /** Sem o título do fluxo (quem chama já mostra o tipo escolhido). */
  semTitulo?: boolean;
  aoCriar: (projeto: string, criada: AnaliseCriada) => void;
  aoCancelar: () => void;
  /** Continua uma análise que a demanda já tem (na demanda cadastrada). */
  aoAbrirAnalise?: (id: string) => void;
}) {
  const [projeto, setProjeto] = useState(projetos[0]?.id ?? "");
  const [entrada, setEntrada] = useState("");
  const [titulo, setTitulo] = useState("");
  const [referencia, setReferencia] = useState(referenciaInicial);
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [criando, setCriando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  // Condução (#371): a última escolhida para o tipo; na primeira vez, a simples.
  const [conducao, setConducao] = useState<Conducao>(() => ultimaConducao(fluxo));

  const demandas = useDemandasDoProjeto(api, projeto, !demandaFixa);

  // O título e as análises da demanda, em todos os tipos com o campo (#350).
  const resolvida = useDemandaResolvida(api, projeto, referencia, demandas, !demandaFixa);

  const verificar = useCallback(() => {
    if (!projeto) return;
    setAmbiente(null);
    api
      .verificarAmbiente(projeto)
      .then(setAmbiente)
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto]);

  useEffect(verificar, [verificar]);

  const ref = demandaFixa ? "" : referencia.trim();
  const ehDemanda = DEMANDA.test(ref);
  const demandaEscolhida = ehDemanda ? demandas.find((d) => d.id.toUpperCase() === ref.toUpperCase()) : undefined;
  const achada = demandaAchada(resolvida);
  const existentes = achada?.analises ?? [];
  // Com a issue, o título e a descrição dela são a entrada; com a demanda do projeto,
  // só na análise da demanda cadastrada (a entrada sai do título da demanda).
  const dispensaEntrada = ref !== "" && (!ehDemanda || cadastrada);
  const temEntrada = entrada.trim() !== "";
  const pronto = ambiente?.pronto === true;
  const { rotulo: rotuloFluxo, exemplo: exemploFluxo } = ENTRADA[fluxo];
  const rotulo = cadastrada ? "O que observar" : rotuloFluxo;
  const exemplo = cadastrada ? "Ex.: Foque nas regras de cobrança e nos clientes PJ." : exemploFluxo;

  /** O que falta para iniciar; `null` quando nada falta. */
  const motivo: string | null = !projeto
    ? "Para iniciar, escolha o projeto na barra de título."
    : ambiente && !pronto
      ? "Para iniciar, ligue o agente a este projeto (acima)."
      : cadastrada && !ref
        ? "Para iniciar, informe a demanda ou a issue."
        : cadastrada && !origemDaReferencia(ref)
          ? "Para iniciar, escolha uma sugestão ou informe D-0012, PROJ-123, #142 ou o link da issue."
        : !cadastrada && !temEntrada && !dispensaEntrada
          ? ehDemanda
            ? `Para iniciar, preencha “${rotulo}”: a demanda ${ref.toUpperCase()} só liga o resultado.`
            : `Para iniciar, ligue uma issue ou preencha “${rotulo}”.`
          : null;

  const criar = (ev: FormEvent) => {
    ev.preventDefault();
    if (motivo || criando) return;
    setCriando(true);
    setFalha(null);
    // Na demanda cadastrada, sem título digitado, vale o da demanda; nos outros tipos, o
    // título sai do texto.
    const tituloFinal = titulo.trim() || (cadastrada ? achada?.titulo : undefined) || undefined;
    let pedido: Promise<AnaliseCriada>;
    if (cadastrada && !ehDemanda && achada?.issue?.aberta && api.issues) {
      // Como "Analisar impacto" em Demandas: cria (ou reaproveita) a demanda da issue e
      // liga a análise a ela, para as próximas acharem a mesma demanda.
      const { origem: o, chave } = achada.issue;
      pedido = api.issues.analisarImpacto(projeto, o, chave).then((r) => r.criada);
    } else if (ref && !ehDemanda) {
      pedido = api.criarAnaliseDeDemanda(projeto, fluxo, ref);
    } else if (ehDemanda) {
      const id = ref.toUpperCase();
      const texto =
        entrada.trim() ||
        `Analisar a demanda ${id}${demandaEscolhida ? `: ${demandaEscolhida.titulo}` : ""}.`;
      pedido = api.criarAnalise(
        projeto,
        fluxo,
        texto,
        tituloFinal ?? (cadastrada ? demandaEscolhida?.titulo : undefined),
        id,
      );
    } else if (demandaFixa) {
      // Pelo painel da demanda, a análise nasce ligada a ela.
      pedido = api.criarAnalise(projeto, fluxo, entrada.trim(), tituloFinal, demandaFixa);
    } else {
      pedido = api.criarAnalise(projeto, fluxo, entrada.trim(), tituloFinal);
    }
    const comConducao = (criada: AnaliseCriada): Promise<AnaliseCriada> => {
      if (!api.conducao) return Promise.resolve(criada);
      lembrarConducao(fluxo, conducao);
      if (conducao !== "workflow") return Promise.resolve(criada);
      return api.conducao
        .escolher(projeto, criada.analise.id, "workflow")
        .then((promptInicial) => ({ ...criada, promptInicial }));
    };
    pedido
      .then(comConducao)
      .then((criada) => aoCriar(projeto, criada))
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setCriando(false);
      });
  };

  return (
    <form className="nova-pergunta nova-analise" onSubmit={criar} aria-label={cadastrada ? "Analisar demanda cadastrada" : ROTULO_FLUXO[fluxo]}>
      {!semTitulo && (
        <TituloSecao nivel={3}>{cadastrada ? "Analisar demanda cadastrada" : ROTULO_FLUXO[fluxo]}</TituloSecao>
      )}
      {demandaFixa ? (
        <p className="muted">A análise fica ligada à demanda {demandaFixa}.</p>
      ) : (
        projetos.length > 1 && (
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
        )
      )}
      {ambiente && !pronto && (
        <PrepararAmbiente api={api} projeto={projeto} ambiente={ambiente} aoConcluir={verificar} />
      )}
      {!demandaFixa && (
        <CampoIssueOuDemanda
          api={api}
          projeto={projeto}
          referencia={referencia}
          aoMudar={setReferencia}
          demandas={demandas}
          resolvida={resolvida}
          cadastrada={cadastrada}
          ajudaVazia="Liga o resultado à demanda; com uma issue, o texto dela é a entrada da análise."
          aoAbrirAnalise={aoAbrirAnalise}
        />
      )}
      <label>
        <span>
          {rotulo}{" "}
          {(cadastrada || dispensaEntrada) && <span className="muted">(opcional)</span>}
        </span>
        <textarea
          value={entrada}
          onChange={(e) => setEntrada(e.target.value)}
          rows={cadastrada ? 3 : 5}
          placeholder={exemplo}
        />
      </label>
      <label>
        <span>
          Título <span className="muted">(opcional)</span>
        </span>
        <input
          value={titulo}
          onChange={(e) => setTitulo(e.target.value)}
          placeholder="Se ficar em branco, sai do texto"
        />
      </label>
      {api.conducao && projeto && (
        <EscolhaConducao
          api={api.conducao}
          projeto={projeto}
          fluxo={fluxo}
          valor={conducao}
          aoMudar={setConducao}
        />
      )}
      <div className="rodape-form">
        {motivo && (
          <p className="motivo-desabilitado" id="nova-analise-motivo">
            {motivo}
          </p>
        )}
        <div className="acoes">
          <button className="btn" type="button" onClick={aoCancelar}>
            Cancelar
          </button>
          <button
            type="submit"
            className="btn btn-primary"
            disabled={motivo !== null || criando}
            aria-describedby={motivo ? "nova-analise-motivo" : undefined}
          >
            {existentes.length > 0 ? "Iniciar uma análise nova" : "Iniciar a análise"}
          </button>
        </div>
      </div>
      {falha && <Alerta tom="erro" titulo={falha} />}
    </form>
  );
}
