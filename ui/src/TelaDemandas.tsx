import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { mensagemDe } from "./analise";
import type { ApiConhecimento } from "./conhecimento";
import { acaoDaIssue, situacaoEmTexto, TOM_SITUACAO } from "./acaoIssue";
import { dobrar, parecidos } from "./demandasParecidas";
import {
  AcoesDaIssue,
  AvisoAbrirLink,
  DescricaoIssue,
  NaBaseIssue,
  ParecidasLista,
  ParteIssue,
  ROTULO_ORIGEM,
  useAbrirLink,
  useRegrasParecidas,
} from "./DetalheIssue";
import {
  ROTULO_STATUS_DEMANDA,
  ROTULO_TIPO_DEMANDA,
  type DemandaVista,
  type TipoDemanda,
} from "./demandas";
import {
  ROTULO_SITUACAO,
  frescor,
  idadeDe,
  type ApiIssues,
  type FiltroEstadoIssues,
  type IssueDoProjeto,
  type ListaIssues,
  type SituacaoNaBase,
} from "./issuesProjeto";
import { FiltroTipoDemanda, FormDemanda, ListaDemandas } from "./ListaDemandas";
import type { Tela } from "./Navegacao";
import { Alerta, BarraAcoes, EstadoVazio, Selo, TituloSecao } from "./componentes";

/** Situações do filtro, na ordem do fluxo da análise. */
const SITUACOES: SituacaoNaBase[] = ["sem-analise", "em-andamento", "em-revisao", "aprovada"];
const ESTADOS: { id: FiltroEstadoIssues; rotulo: string }[] = [
  { id: "abertas", rotulo: "Abertas" },
  { id: "encerradas", rotulo: "Encerradas" },
  { id: "todas", rotulo: "Todas" },
];

/**
 * Uma linha da lista única: a issue do rastreador ou a demanda criada à mão. O
 * `tipoDemanda` é o da demanda na base (#358): na issue, o da demanda ligada a ela.
 */
type Linha = { id: string; criada: string; tipoDemanda: TipoDemanda | null } & (
  | { tipo: "issue"; issue: IssueDoProjeto }
  | { tipo: "demanda"; demanda: DemandaVista }
);

/** A ação da issue mora em `acaoIssue.ts`; a re-exportação mantém o nome nesta tela. */
export { acaoDaIssue };

/** Abre a issue numa aba do app, ou a demanda dela quando a issue já tem uma (#398). */
function abrirIssueEmAba(navegar: (t: Tela) => void, projeto: string, i: IssueDoProjeto) {
  if (i.demanda) navegar({ tipo: "demanda", projeto, demanda: i.demanda });
  else navegar({ tipo: "issue", projeto, origem: i.origem, chave: i.chave });
}

const chaveDe = (l: Linha) => `${l.tipo}:${l.id}`;

/**
 * Área Demandas (#323, §8 da revisão de UX): uma lista só com as issues do rastreador
 * (lidas do cache, que se atualiza em segundo plano) e as demandas criadas à mão. A
 * ação de cada linha acompanha a situação na base. O painel ao lado mostra a issue, o
 * que já existe na base e o que é parecido. "Analisar impacto" cria a demanda (ou
 * reaproveita a que existe) e a análise de impacto e abre a análise com o agente.
 * Nada é escrito no rastreador. O vínculo com as funcionalidades parte daqui.
 *
 * Sem `api.issues`, mostra só a lista das demandas da base.
 */
export function TelaDemandas({
  api,
  projeto,
  navegar,
  situacaoInicial,
}: {
  api: ApiConhecimento;
  projeto: string;
  navegar: (t: Tela) => void;
  situacaoInicial?: SituacaoNaBase;
}) {
  if (!api.issues) {
    return (
      <>
        <TituloSecao>Demandas</TituloSecao>
        <ListaDemandas api={api} projeto={projeto} navegar={navegar} />
      </>
    );
  }
  return (
    <DemandasDoRastreador
      api={api}
      issues={api.issues}
      projeto={projeto}
      navegar={navegar}
      situacaoInicial={situacaoInicial}
    />
  );
}

function DemandasDoRastreador({
  api,
  issues,
  projeto,
  navegar,
  situacaoInicial,
}: {
  api: ApiConhecimento;
  issues: ApiIssues;
  projeto: string;
  navegar: (t: Tela) => void;
  situacaoInicial?: SituacaoNaBase;
}) {
  const [busca, setBusca] = useState("");
  const [estado, setEstado] = useState<FiltroEstadoIssues>("abertas");
  const [situacao, setSituacao] = useState<SituacaoNaBase | null>(situacaoInicial ?? null);
  const [rotulo, setRotulo] = useState("");
  const [lista, setLista] = useState<ListaIssues | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [lendo, setLendo] = useState(false);
  const [tipo, setTipo] = useState<TipoDemanda | null>(null);
  // Todas as demandas da base: as sem chave viram linhas; as outras dão o tipo das issues.
  const [daBase, setDaBase] = useState<DemandaVista[]>([]);
  const manuais = useMemo(() => daBase.filter((d) => !d.chave), [daBase]);
  const [selecionada, setSelecionada] = useState<string | null>(null);
  const [analisando, setAnalisando] = useState<string | null>(null);
  const [falhaAcao, setFalhaAcao] = useState<string | null>(null);
  const [criando, setCriando] = useState(false);
  const [agora, setAgora] = useState(() => Date.now());

  const filtro = useMemo(
    () => ({ busca, estado, situacao: situacao ?? undefined, rotulo: rotulo || undefined }),
    [busca, estado, situacao, rotulo],
  );
  const filtroRef = useRef(filtro);
  filtroRef.current = filtro;
  const pedido = useRef(0);

  const carregar = useCallback(
    (forcar: boolean) => {
      const vez = ++pedido.current;
      if (forcar) setLendo(true);
      issues
        .listar(projeto, filtroRef.current, forcar)
        .then((l) => {
          if (vez !== pedido.current) return;
          setLista(l);
          setFalha(null);
        })
        .catch((e: unknown) => {
          if (vez === pedido.current) setFalha(mensagemDe(e));
        })
        .finally(() => {
          if (vez === pedido.current) setLendo(false);
        });
    },
    [issues, projeto],
  );

  // A lista vem do cache a cada mudança de busca ou filtro; o rastreador só é
  // chamado na releitura em segundo plano ou no "Atualizar".
  useEffect(() => carregar(false), [carregar, filtro]);

  // A releitura em segundo plano avisa quando termina.
  useEffect(() => {
    let ativo = true;
    let parar: (() => void) | null = null;
    issues
      .aoAtualizar((p) => {
        if (p === projeto) carregar(false);
      })
      .then((f) => (ativo ? (parar = f) : f()))
      .catch(() => undefined);
    return () => {
      ativo = false;
      parar?.();
    };
  }, [issues, projeto, carregar]);

  // O "conferido há…" acompanha o relógio.
  useEffect(() => {
    const relogio = setInterval(() => setAgora(Date.now()), 30_000);
    return () => clearInterval(relogio);
  }, []);

  const carregarManuais = useCallback(() => {
    api
      .listarDemandas(projeto)
      .then(setDaBase)
      // Sem a lista da base, ficam as issues.
      .catch(() => undefined);
  }, [api, projeto]);
  useEffect(carregarManuais, [carregarManuais]);

  const linhas = useMemo<Linha[]>(() => {
    const tipoDe = new Map(daBase.map((d) => [d.id, d.tipo]));
    const deIssues: Linha[] = (lista?.issues ?? []).map((issue) => ({
      tipo: "issue",
      id: issue.chave,
      criada: issue.criadaEm,
      tipoDemanda: (issue.demanda && tipoDe.get(issue.demanda)) || null,
      issue,
    }));
    // Demanda à mão não tem situação nem rótulo: some quando o filtro os pede.
    const termo = dobrar(busca.trim());
    const deDemandas: Linha[] =
      situacao || rotulo
        ? []
        : manuais
            .filter((d) => {
              if (estado === "abertas" && d.status !== "aberta") return false;
              if (estado === "encerradas" && d.status !== "encerrada") return false;
              return !termo || dobrar(`${d.id} ${d.titulo}`).includes(termo);
            })
            .map((demanda) => ({
              tipo: "demanda",
              id: demanda.id,
              criada: demanda.criado,
              tipoDemanda: demanda.tipo,
              demanda,
            }));
    // O tipo é da demanda na base: a issue sem demanda não tem tipo e some com o filtro.
    return [...deIssues, ...deDemandas]
      .filter((l) => !tipo || l.tipoDemanda === tipo)
      .sort((a, b) => b.criada.localeCompare(a.criada));
  }, [lista, daBase, manuais, busca, estado, situacao, rotulo, tipo]);

  const escolhida = linhas.find((l) => chaveDe(l) === selecionada) ?? linhas[0] ?? null;

  const abrirAnalise = (id: string) =>
    navegar({ tipo: "analise", projeto, aberta: { id, abertura: { tipo: "reabrir" } } });

  const agir = (i: IssueDoProjeto) => {
    const acao = acaoDaIssue(i);
    if (acao.bloqueio) return;
    if (acao.tipo === "demanda" && i.demanda) {
      navegar({ tipo: "demanda", projeto, demanda: i.demanda });
    } else if (acao.tipo === "analise" && i.analise) {
      abrirAnalise(i.analise);
    } else if (acao.tipo === "analisar") {
      if (analisando) return;
      setAnalisando(i.chave);
      setFalhaAcao(null);
      issues
        .analisarImpacto(projeto, i.origem, i.chave)
        .then((r) =>
          navegar({
            tipo: "analise",
            projeto,
            aberta: {
              id: r.criada.analise.id,
              abertura: { tipo: "nova", promptInicial: r.criada.promptInicial },
            },
          }),
        )
        .catch((e: unknown) => {
          setFalhaAcao(mensagemDe(e));
          setAnalisando(null);
        });
    }
  };

  const atualizando = lendo || lista?.atualizando === true;
  const conferido = lista ? frescor(lista.lidaEm, agora) : "";
  const semResultado = lista !== null && linhas.length === 0;
  const filtrando =
    busca.trim() !== "" || situacao !== null || rotulo !== "" || tipo !== null || estado !== "abertas";

  return (
    <section className="demandas" aria-label="Demandas">
      <TituloSecao
        acoes={
          <BarraAcoes
            rotulo="Ações das demandas"
            secundarias={[
              {
                texto: "Vincular a funcionalidades",
                dica: "Liga as demandas às funcionalidades do mapa",
                aoEscolher: () => navegar({ tipo: "vincular", projeto }),
              },
              { texto: "Nova demanda à mão", aoEscolher: () => setCriando((v) => !v) },
              {
                texto: atualizando ? "Atualizando…" : "Atualizar",
                desabilitado: atualizando,
                aoEscolher: () => carregar(true),
              },
            ]}
          />
        }
      >
        Demandas
      </TituloSecao>
      <p className="muted demandas-sub">
        Issues do rastreador e demandas criadas à mão
        {conferido && (
          <>
            {" · "}
            <span data-testid="frescor">{conferido}</span>
          </>
        )}
      </p>

      {lista?.aviso && (
        <Alerta tom="atencao" titulo="Não foi possível atualizar a lista:">
          {lista.aviso} Mostrando a última leitura, {conferido || "antiga"}.
        </Alerta>
      )}
      {falha && <Alerta tom="erro" titulo={falha} />}
      {criando && (
        <FormDemanda
          api={api}
          projeto={projeto}
          aoCancelar={() => setCriando(false)}
          aoCriar={(d) => {
            setCriando(false);
            setSelecionada(`demanda:${d.id}`);
            carregarManuais();
          }}
        />
      )}

      <div className="demandas-filtros">
        <label className="sr-only" htmlFor="busca-demandas">
          Buscar demandas
        </label>
        <input
          id="busca-demandas"
          type="search"
          className="demandas-busca"
          placeholder="Número ou título"
          value={busca}
          onChange={(e) => setBusca(e.target.value)}
        />
        <div className="vincular-chips" role="group" aria-label="Estado no rastreador">
          {ESTADOS.map((e) => (
            <button
              key={e.id}
              type="button"
              className="vincular-chip"
              aria-pressed={estado === e.id}
              onClick={() => setEstado(e.id)}
            >
              {e.rotulo}
            </button>
          ))}
        </div>
        <div className="vincular-chips" role="group" aria-label="Situação na base">
          {SITUACOES.map((s) => (
            <button
              key={s}
              type="button"
              className="vincular-chip"
              aria-pressed={situacao === s}
              onClick={() => setSituacao(situacao === s ? null : s)}
            >
              {ROTULO_SITUACAO[s][0].toUpperCase() + ROTULO_SITUACAO[s].slice(1)}
            </button>
          ))}
        </div>
        <FiltroTipoDemanda id="tipo-demandas" valor={tipo} aoMudar={setTipo} />
        {lista && lista.rotulos.length > 0 && (
          <>
            <label className="sr-only" htmlFor="rotulo-demandas">
              Rótulo
            </label>
            <select id="rotulo-demandas" value={rotulo} onChange={(e) => setRotulo(e.target.value)}>
              <option value="">Todos os rótulos</option>
              {lista.rotulos.map((r) => (
                <option key={r} value={r}>
                  {r}
                </option>
              ))}
            </select>
          </>
        )}
      </div>

      {lista === null && !falha && <p className="muted">Carregando…</p>}
      {falhaAcao && <Alerta tom="erro" titulo={falhaAcao} />}

      {(lista !== null || manuais.length > 0) && (
        <div className="demandas-grade">
          <div className="panel demandas-lista">
            {semResultado ? (
              <EstadoVazio>
                {filtrando
                  ? "Nenhuma demanda com este filtro."
                  : "Nenhuma issue aberta. Use Atualizar para ler o rastreador de novo."}
              </EstadoVazio>
            ) : (
              <ul className="linhas" aria-label="Lista de demandas">
                {linhas.map((l) => (
                  <LinhaDaLista
                    key={chaveDe(l)}
                    linha={l}
                    agora={agora}
                    escolhida={escolhida !== null && chaveDe(escolhida) === chaveDe(l)}
                    analisando={l.tipo === "issue" && analisando === l.issue.chave}
                    aoEscolher={() => setSelecionada(chaveDe(l))}
                    aoAgir={() => (l.tipo === "issue" ? agir(l.issue) : abrirDemanda(l.demanda.id))}
                    aoAbrirAba={() =>
                      l.tipo === "issue" ? abrirIssueEmAba(navegar, projeto, l.issue) : abrirDemanda(l.demanda.id)
                    }
                  />
                ))}
              </ul>
            )}
            {lista && (
              <p className="muted demandas-rodape">
                {linhas.length} de {lista.total + manuais.length} demandas. Nada é escrito no
                rastreador: analisar cria a demanda na base e a análise ligada a ela.
                {lista.excedeuTeto && ` A leitura traz as issues mais recentes, criadas desde ${lista.desde}.`}
              </p>
            )}
          </div>
          {escolhida && (
            <PainelDaLinha
              key={chaveDe(escolhida)}
              api={api}
              linha={escolhida}
              todas={linhas}
              agora={agora}
              analisando={escolhida.tipo === "issue" && analisando === escolhida.issue.chave}
              navegar={navegar}
              projeto={projeto}
              aoAgir={() =>
                escolhida.tipo === "issue" ? agir(escolhida.issue) : abrirDemanda(escolhida.demanda.id)
              }
              aoAbrirAnalise={abrirAnalise}
            />
          )}
        </div>
      )}
    </section>
  );

  function abrirDemanda(id: string) {
    navegar({ tipo: "demanda", projeto, demanda: id });
  }
}

function LinhaDaLista({
  linha,
  agora,
  escolhida,
  analisando,
  aoEscolher,
  aoAgir,
  aoAbrirAba,
}: {
  linha: Linha;
  agora: number;
  escolhida: boolean;
  analisando: boolean;
  aoEscolher: () => void;
  aoAgir: () => void;
  /** Clique do meio na linha: abre a demanda numa aba do app (#398). */
  aoAbrirAba: () => void;
}) {
  const idade = idadeDe(linha.criada, agora);
  const issue = linha.tipo === "issue" ? linha.issue : null;
  const mao = linha.tipo === "demanda" ? linha.demanda : null;
  const acao = issue ? acaoDaIssue(issue) : null;
  const titulo = issue?.titulo ?? mao?.titulo ?? "";
  const tipo = linha.tipoDemanda && ROTULO_TIPO_DEMANDA[linha.tipoDemanda];
  const meta = issue
    ? [
        tipo,
        issue.estado === "aberta" ? `aberta ${idade}` : `encerrada (criada ${idade})`,
        ...issue.rotulos,
        issue.demanda,
      ]
    : [tipo, mao?.status === "aberta" ? `criada ${idade}` : "encerrada", "criada à mão"];
  return (
    <li
      className="linha-demanda"
      data-selecionada={escolhida}
      onAuxClick={(e) => {
        if (e.button !== 1) return;
        e.preventDefault();
        aoAbrirAba();
      }}
    >
      <button
        type="button"
        className="linha-demanda-main"
        aria-current={escolhida || undefined}
        onClick={aoEscolher}
      >
        <span className="id">{linha.id}</span>
        <span className="tit">
          <b>{titulo}</b>
          <br />
          <span className="muted">{meta.filter(Boolean).join(" · ")}</span>
        </span>
      </button>
      <span className="fim">
        {issue ? (
          <Selo tom={TOM_SITUACAO[issue.situacao]}>{situacaoEmTexto(issue)}</Selo>
        ) : (
          mao && (
            <Selo>{ROTULO_STATUS_DEMANDA[mao.status]}</Selo>
          )
        )}
        <button
          type="button"
          className={`btn btn-sm${acao?.tipo === "analisar" && escolhida && !acao.bloqueio ? " btn-primary" : ""}`}
          disabled={analisando || !!acao?.bloqueio}
          title={acao?.bloqueio}
          aria-label={`${analisando ? "Analisando" : (acao?.rotulo ?? "Abrir a demanda")} ${linha.id}`}
          onClick={aoAgir}
        >
          {analisando ? "Analisando…" : (acao?.rotulo ?? "Abrir a demanda")}
        </button>
      </span>
    </li>
  );
}

function PainelDaLinha({
  api,
  linha,
  todas,
  agora,
  analisando,
  navegar,
  projeto,
  aoAgir,
  aoAbrirAnalise,
}: {
  api: ApiConhecimento;
  linha: Linha;
  todas: Linha[];
  agora: number;
  analisando: boolean;
  navegar: (t: Tela) => void;
  projeto: string;
  aoAgir: () => void;
  aoAbrirAnalise: (id: string) => void;
}) {
  const issue = linha.tipo === "issue" ? linha.issue : null;
  const mao = linha.tipo === "demanda" ? linha.demanda : null;
  const titulo = issue?.titulo ?? mao?.titulo ?? "";
  const { abrir, falha } = useAbrirLink(projeto);
  const abrirNoRastreador = () => {
    if (issue?.url) void abrir(issue.url);
  };
  const similares = useMemo(
    () =>
      parecidos(
        { id: linha.id, titulo },
        todas.map((l) => ({
          id: l.id,
          titulo: l.tipo === "issue" ? l.issue.titulo : l.demanda.titulo,
          linha: l,
        })),
      ),
    [linha.id, titulo, todas],
  );
  const regras = useRegrasParecidas(api, projeto, titulo);

  return (
    <aside className="panel demandas-painel" aria-label="Demanda selecionada">
      <div className="demandas-topo">
        <h3 className="demandas-painel-cab">
          <span className="id">{linha.id}</span>
          {issue && (
            <Selo>{issue.estado === "aberta" ? "aberta" : "encerrada"}</Selo>
          )}
          {linha.tipoDemanda && (
            <Selo tom="neutro" titulo="Tipo da demanda">
              {ROTULO_TIPO_DEMANDA[linha.tipoDemanda]}
            </Selo>
          )}
          {issue?.url && (
            <button type="button" className="link" onClick={abrirNoRastreador}>
              Abrir no {ROTULO_ORIGEM[issue.origem]} ↗
            </button>
          )}
        </h3>
        <AcoesDaIssue
          acao={issue ? acaoDaIssue(issue) : { rotulo: "Abrir a demanda", tipo: "demanda" }}
          analisando={analisando}
          aoAgir={aoAgir}
          secundarias={
            issue
              ? [{ texto: "Abrir em nova aba", aoEscolher: () => abrirIssueEmAba(navegar, projeto, issue) }]
              : undefined
          }
        />
      </div>
      <p className="demandas-painel-titulo">{titulo}</p>
      {issue ? (
        <>
          <p className="muted">
            {[`criada ${idadeDe(issue.criadaEm, agora)}`, ...issue.rotulos.map((r) => `rótulo ${r}`)].join(" · ")}
          </p>
          <AvisoAbrirLink falha={falha} />
          <ParteIssue titulo="Descrição" nivel={4}>
            <DescricaoIssue issue={issue} abrir={(link) => void abrir(link)} />
          </ParteIssue>
          <ParteIssue titulo="Na base" nivel={4}>
            <NaBaseIssue issue={issue} projeto={projeto} navegar={navegar} aoAbrirAnalise={aoAbrirAnalise} />
          </ParteIssue>
        </>
      ) : (
        <p className="muted">
          Demanda criada à mão em {mao?.criado}. Abra a demanda para ver as atas, as
          análises e as sessões, e para ligar uma análise nova.
        </p>
      )}

      <ParecidasLista
        itens={similares.map((s) => ({
          chave: chaveDe(s.linha),
          id: s.id,
          titulo: s.titulo,
          analise: s.linha.tipo === "issue" ? s.linha.issue.analise : null,
        }))}
        regras={regras}
        navegar={navegar}
        nivel={4}
      />
    </aside>
  );
}
