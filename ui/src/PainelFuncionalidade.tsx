import { useEffect, useState } from "react";
import { mensagemDe, type Fluxo } from "./analise";
import { apiConhecimentoTauri, type ApiConhecimento } from "./conhecimento";
import { useFluxosOcultos } from "./fluxosDoProjeto";
import { contar } from "./formatacao";
import {
  Alerta,
  BarraAcoes,
  Cartao,
  EstadoVazio,
  MenuAcoes,
  Secao,
  Selo,
  TextoRico,
  type Acao,
} from "./componentes";
import {
  ESTADO_ANALISE,
  FormDividir,
  FormEditar,
  FormJuntar,
  SITUACAO,
  type Form,
  type Revisao,
} from "./FormulariosFuncionalidade";
import {
  NOME_LIGACAO,
  NOME_TIPO_ENTRADA,
  type ApiMapa,
  type DemandaNoPainel,
  type EtapaDemanda,
  type Funcionalidade,
  type PainelFuncionalidade as Painel,
  type RevisaoDoMapa,
  type SituacaoManual,
} from "./funcionalidades";
import type { Tela } from "./Navegacao";
import { ROTULO_ESTADO_RASTREADOR } from "./demandas";
import { EstadoNoRastreador } from "./EstadoNoRastreador";
import { DocumentacaoDoMapa } from "./DocumentacaoDoMapa";
import type { ApiDocumentacao } from "./documentacaoMapa";
import type { ApiTerminal } from "./terminal";
import type { TelaTerminal } from "./telaXterm";

/** O estado no rastreador, quando conferido (pode ser "em andamento"); senão, o local. */
function rotuloDaDemanda(d: DemandaNoPainel): string {
  if (d.rastreador) return ROTULO_ESTADO_RASTREADOR[d.rastreador.estado];
  return d.aberta ? "aberta" : "encerrada";
}

type Aba = "visao" | "analises" | "demandas" | "codigo";

const NOME_FLUXO: Partial<Record<Fluxo, string>> = {
  explicar: "Explicar",
  perguntar: "Perguntar",
  impacto: "Impacto",
  historias: "Histórias",
  cenarios: "Cenários",
  revisar: "Revisar",
  reuniao: "Reunião",
  manual: "Manual",
};

/** Fluxos que o painel abre, com a descrição do atalho. */
const NOVAS: { fluxo: Fluxo; nome: string; descricao: string }[] = [
  { fluxo: "explicar", nome: "Explicar", descricao: "Gera o documento negocial" },
  { fluxo: "perguntar", nome: "Perguntar", descricao: "Uma dúvida sobre o comportamento" },
  { fluxo: "impacto", nome: "Analisar impacto", descricao: "De uma demanda nesta funcionalidade" },
  { fluxo: "historias", nome: "Histórias", descricao: "Da explicação aprovada ou de uma demanda" },
  { fluxo: "cenarios", nome: "Cenários de teste", descricao: "Das regras, pela explicação ou demanda" },
];

/**
 * Manual do usuário da funcionalidade (#272): gerar (só com a funcionalidade
 * aceita), continuar o que está em andamento ou abrir o aprovado; o aprovado
 * fica "a revisar" quando a funcionalidade muda, e a revisão começa com os
 * passos e os prints dele.
 */
function CartaoManual({
  manual,
  aceita,
  ocupado,
  aoGerar,
  aoAbrirAnalise,
  aoAbrirDocumento,
}: {
  manual?: SituacaoManual;
  aceita: boolean;
  ocupado: boolean;
  aoGerar: () => void;
  aoAbrirAnalise: (id: string) => void;
  aoAbrirDocumento: (documento: string) => void;
}) {
  const m = manual ?? { situacao: "nenhum" as const };
  const aRevisar = m.situacao === "aprovado" && m.aRevisar.length > 0;
  let principal: Acao | undefined;
  const secundarias: Acao[] = [];
  if (m.situacao === "nenhum" && aceita) {
    principal = { texto: "Gerar manual do usuário", aoEscolher: aoGerar, desabilitado: ocupado };
  } else if (m.situacao === "emAndamento") {
    secundarias.push({ texto: `Abrir o manual ${m.analise}`, aoEscolher: () => aoAbrirAnalise(m.analise) });
  } else if (m.situacao === "aprovado") {
    const documento = m.documento;
    if (documento) secundarias.push({ texto: "Abrir na base", aoEscolher: () => aoAbrirDocumento(documento) });
    secundarias.push({ texto: `Abrir ${m.analise}`, aoEscolher: () => aoAbrirAnalise(m.analise) });
    if (aceita) {
      const revisar = { texto: "Revisar o manual", aoEscolher: aoGerar, desabilitado: ocupado };
      if (aRevisar) principal = revisar;
      else secundarias.push(revisar);
    }
  }
  return (
    <Secao
      titulo="Manual do usuário"
      nivel={3}
      acoes={
        m.situacao === "emAndamento" ? (
          <span className="muted">
            {m.analise} · <Selo>{ESTADO_ANALISE[m.estado]}</Selo>
          </span>
        ) : m.situacao === "aprovado" ? (
          <Selo>{aRevisar ? "a revisar" : "aprovado"}</Selo>
        ) : undefined
      }
    >
      {m.situacao === "nenhum" && (
        <p className="muted">
          {aceita
            ? "Gere o passo a passo para o usuário final; os prints de tela entram em cada passo."
            : "Aceite o mapeamento da funcionalidade para gerar o manual."}
        </p>
      )}
      {m.situacao === "emAndamento" && (
        <p>
          {contar(m.passos, "passo", "passos")}
          {m.semPrint > 0 && ` · ${contar(m.semPrint, "passo sem print", "passos sem print")}`}
        </p>
      )}
      {m.situacao === "aprovado" &&
        (aRevisar ? (
          <>
            <p>A funcionalidade mudou depois da aprovação; os prints foram mantidos.</p>
            <ul className="motivos-revisar">
              {m.aRevisar.map((x) => (
                <li key={x}>{x}</li>
              ))}
            </ul>
          </>
        ) : (
          <p>O manual foi aprovado e está na base, ao lado dos prints.</p>
        ))}
      {(principal || secundarias.length > 0) && (
        <BarraAcoes principal={principal} secundarias={secundarias} rotulo="Ações do manual" />
      )}
    </Secao>
  );
}

const dia = (ms: number) =>
  new Date(ms).toLocaleDateString("pt-BR", { day: "2-digit", month: "2-digit" });

function Etapa({ nome, etapa }: { nome: string; etapa: EtapaDemanda | null }) {
  return (
    <li className={`etapa ${etapa ? `etapa-${etapa.estado}` : "etapa-vazia"}`}>
      <strong>{nome}</strong>
      <span>{etapa ? `${etapa.analise} · ${ESTADO_ANALISE[etapa.estado]}` : "não iniciada"}</span>
    </li>
  );
}

function CartaoDemanda({
  d,
  ocupado,
  aoAbrir,
  aoAbrirAnalise,
  aoDesligar,
  aoAnalisarImpacto,
}: {
  d: DemandaNoPainel;
  ocupado: boolean;
  aoAbrir: () => void;
  aoAbrirAnalise: (id: string) => void;
  aoDesligar?: () => void;
  aoAnalisarImpacto: () => void;
}) {
  const etapas = [d.impacto, d.historias, d.cenarios].filter((e): e is EtapaDemanda => !!e);
  const acoes: Acao[] = [
    ...etapas.map((e) => ({ texto: `Abrir ${e.analise}`, aoEscolher: () => aoAbrirAnalise(e.analise) })),
    { texto: "Abrir a demanda (matriz)", aoEscolher: aoAbrir },
    ...(aoDesligar ? [{ texto: "Desligar", aoEscolher: aoDesligar, desabilitado: ocupado }] : []),
  ];
  return (
    <Cartao rotulo={`${d.id}: ${d.titulo}`}>
      <header className="linha-cabecalho">
        <button type="button" className="btn btn-link mono" onClick={aoAbrir}>
          {d.id}
        </button>
        <strong>{d.titulo}</strong>
        <Selo>{rotuloDaDemanda(d)}</Selo>
        {d.criado && <span className="muted">desde {d.criado}</span>}
        <EstadoNoRastreador rastreador={d.rastreador} aviso={d.avisoRastreador} />
        {d.chave &&
          (d.origem ? (
            <a href={d.origem} target="_blank" rel="noreferrer" className="mono">
              {d.chave}
            </a>
          ) : (
            <span className="mono muted">{d.chave}</span>
          ))}
      </header>
      <ol className="pf-etapas" aria-label={`Etapas de ${d.id}`}>
        <Etapa nome="Impacto" etapa={d.impacto} />
        <Etapa nome="Histórias" etapa={d.historias} />
        <Etapa nome="Cenários" etapa={d.cenarios} />
      </ol>
      {d.perguntas.length > 0 && (
        <ul className="perguntas-painel" aria-label={`Perguntas em aberto de ${d.id}`}>
          {d.perguntas.map((p) => (
            <li key={`${p.analise}/${p.numero}`}>
              {p.texto}{" "}
              <span className="muted">
                · {p.destinatario ? `para ${p.destinatario}` : "sem destinatário"} · {p.analise}
              </span>
            </li>
          ))}
        </ul>
      )}
      <BarraAcoes
        principal={
          !d.impacto && d.aberta
            ? { texto: "Analisar impacto", aoEscolher: aoAnalisarImpacto, desabilitado: ocupado }
            : undefined
        }
        secundarias={acoes}
        rotulo={`Ações de ${d.id}`}
      />
    </Cartao>
  );
}

function NovaAnalise({
  painel,
  fluxoInicial,
  demandaInicial,
  ocupado,
  ocultos,
  aoCriar,
}: {
  painel: Painel;
  /** Fluxos que o projeto não oferece (#465). */
  ocultos: Fluxo[];
  fluxoInicial: Fluxo | null;
  demandaInicial: string;
  ocupado: boolean;
  aoCriar: (fluxo: Fluxo, texto: string | null, demanda: string | null) => void;
}) {
  const f = painel.funcionalidade;
  const [fluxo, setFluxo] = useState<Fluxo | null>(fluxoInicial);
  const [texto, setTexto] = useState("");
  const [demanda, setDemanda] = useState(demandaInicial);
  useEffect(() => {
    setFluxo(fluxoInicial);
    setDemanda(demandaInicial);
  }, [fluxoInicial, demandaInicial]);
  const explicacaoAprovada = f.explicacao?.estado === "aprovada";
  const desabilitado = (fl: Fluxo): string | null => {
    if (fl === "explicar" && f.explicacao) return `Já explicada em ${f.explicacao.id}`;
    if (fl === "impacto" && painel.demandasDoProjeto.length === 0) return "Nenhuma demanda aberta";
    return null;
  };
  const demandas = painel.demandasDoProjeto;
  const valido =
    fluxo !== null &&
    (fluxo !== "perguntar" || texto.trim().length > 0) &&
    (fluxo !== "impacto" || demanda !== "") &&
    (!(fluxo === "historias" || fluxo === "cenarios") || explicacaoAprovada || demanda !== "");
  return (
    <Secao titulo="Nova análise para esta funcionalidade" nivel={3}>
      <p className="muted">A entrada já vem com a funcionalidade e os pontos de entrada.</p>
      <div className="novas-analises" role="group" aria-label="Fluxo">
        {NOVAS.filter((n) => !ocultos.includes(n.fluxo)).map((n) => {
          const motivo = desabilitado(n.fluxo);
          return (
            <button
              key={n.fluxo}
              type="button"
              className="opcao-fluxo"
              aria-pressed={fluxo === n.fluxo}
              disabled={!!motivo}
              onClick={() => setFluxo(n.fluxo)}
            >
              <strong>{n.nome}</strong>
              <span>{motivo ?? n.descricao}</span>
            </button>
          );
        })}
      </div>
      {fluxo && (
        <form
          className="form-funcionalidade"
          aria-label={`Criar ${NOME_FLUXO[fluxo] ?? fluxo}`}
          onSubmit={(e) => {
            e.preventDefault();
            if (valido) aoCriar(fluxo, texto.trim() || null, demanda || null);
          }}
        >
          {fluxo === "perguntar" && (
            <label>
              Pergunta
              <textarea rows={2} value={texto} onChange={(e) => setTexto(e.target.value)} />
            </label>
          )}
          {fluxo !== "explicar" && (
            <label>
              {fluxo === "impacto" ? "Demanda" : "Demanda (opcional)"}
              <select value={demanda} onChange={(e) => setDemanda(e.target.value)}>
                <option value="">
                  {fluxo === "historias" || fluxo === "cenarios"
                    ? explicacaoAprovada
                      ? `Da explicação aprovada (${f.explicacao!.id})`
                      : "Escolha a demanda"
                    : fluxo === "impacto"
                      ? "Escolha a demanda"
                      : "Nenhuma"}
                </option>
                {demandas.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.id} · {d.titulo}
                  </option>
                ))}
              </select>
            </label>
          )}
          {fluxo !== "perguntar" && fluxo !== "explicar" && (
            <label>
              Orientação ao agente (opcional)
              <textarea rows={2} value={texto} onChange={(e) => setTexto(e.target.value)} />
            </label>
          )}
          {(fluxo === "historias" || fluxo === "cenarios") && !explicacaoAprovada && !demanda && (
            <p className="muted">Sem explicação aprovada, escolha a demanda de onde partir.</p>
          )}
          <div className="acoes">
            <button type="submit" className="btn btn-primary" disabled={ocupado || !valido}>
              Criar e abrir
            </button>
          </div>
        </form>
      )}
    </Secao>
  );
}

/**
 * Painel da funcionalidade (#208): a página de uma funcionalidade do mapa como
 * ponto de partida do analista: o mapeamento e a revisão dele, o documento
 * negocial, as análises e as demandas ligadas, as regras, as perguntas em aberto,
 * as pendências e a atividade recente.
 */
export function PainelFuncionalidade({
  projeto,
  f,
  outras,
  recarga,
  api,
  apiConhecimento = apiConhecimentoTauri,
  revisao,
  evidencias,
  navegar,
  aoVoltar,
  aoEscolher,
  apiDocumentacao,
  apiTerminal,
  criarTela,
}: {
  projeto: string;
  /** A funcionalidade como o mapa a tem (atualiza com a revisão). */
  f: Funcionalidade;
  outras: Funcionalidade[];
  /** Muda a cada releitura do mapa: o painel relê junto. */
  recarga: number;
  api: ApiMapa;
  apiConhecimento?: ApiConhecimento;
  revisao: Revisao;
  evidencias: boolean;
  navegar: (t: Tela) => void;
  aoVoltar: () => void;
  aoEscolher: (id: string) => void;
  /** Os documentos vivos da funcionalidade (entregáveis, #499). */
  apiDocumentacao?: ApiDocumentacao;
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
}) {
  const [painel, setPainel] = useState<Painel | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [aba, setAba] = useState<Aba>("visao");
  const [form, setForm] = useState<Form>(null);
  const [nova, setNova] = useState<{ fluxo: Fluxo | null; demanda: string }>({
    fluxo: null,
    demanda: "",
  });
  const [ocupado, setOcupado] = useState(false);
  const [aviso, setAviso] = useState<string | null>(null);
  const [ligar, setLigar] = useState("");
  const [releitura, setReleitura] = useState(0);
  const fluxosOcultos = useFluxosOcultos(apiConhecimento, projeto);

  useEffect(() => {
    let vivo = true;
    Promise.resolve()
      .then(() => api.painel(projeto, f.id))
      .then((p) => {
        if (!vivo) return;
        setPainel(p);
        setErro(null);
      })
      .catch((e: unknown) => vivo && setErro(mensagemDe(e)));
    return () => {
      vivo = false;
    };
  }, [api, projeto, f.id, recarga, releitura]);

  useEffect(() => {
    setAba("visao");
    setForm(null);
    setNova({ fluxo: null, demanda: "" });
  }, [f.id]);

  const abrirAnalise = (id: string) =>
    navegar({ tipo: "analise", projeto, aberta: { id, abertura: { tipo: "reabrir" } } });
  const revisar = (r: RevisaoDoMapa) =>
    revisao
      .aoRevisar(r)
      .then(() => setForm(null))
      .catch(() => undefined);
  const criar = (fluxo: Fluxo, texto: string | null, demanda: string | null) => {
    setOcupado(true);
    setFalha(null);
    api
      .criarAnalise(projeto, f.id, fluxo, texto, demanda)
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
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };
  const novaAnalise = (fluxo: Fluxo | null, demanda = "") => {
    setNova({ fluxo, demanda });
    setAba("analises");
  };
  const revalidar = () => {
    setOcupado(true);
    setAviso(null);
    apiConhecimento
      .revalidar(projeto)
      .then((r) => {
        setAviso(`Conferidas ${r.evidencias} evidências; ${r.desatualizadas} não conferem mais.`);
        setReleitura((n) => n + 1);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };
  const ligarDemanda = (demanda: string, sim: boolean) =>
    revisar({ tipo: "ligar-demanda", funcionalidade: f.id, demanda, ligar: sim }).then(() =>
      setLigar(""),
    );

  const s = f.situacao;
  const aberta = s === "pendente" || s === "aceita";
  const p = painel;
  const aRevalidar = p?.regras.filter((r) => r.status === "a-revalidar") ?? [];
  const emAndamento = p?.analises.filter((a) => a.estado === "em-andamento").length ?? 0;
  const tipos = [...new Set(f.pontos.map((x) => (x.tipo ? NOME_TIPO_ENTRADA[x.tipo] : "fora da lista")))];
  const passos = [
    { feito: s === "aceita", nome: "Revisar o mapeamento", texto: "Confira nome, área e pontos de entrada; aceite ou ajuste." },
    { feito: !!f.explicacao, nome: "Explicar a funcionalidade", texto: "Gera o documento negocial com as regras e as evidências." },
    { feito: (p?.demandas.length ?? 0) > 0, nome: "Ligar demandas", texto: "As demandas que mexem nesta funcionalidade aparecem aqui." },
  ];
  const decidir = (decisao: "aceitar" | "descartar" | "desfazer") =>
    revisar({ tipo: "decidir", funcionalidade: f.id, decisao });
  const documento = f.documento;
  const acoesDoCabecalho: Acao[] = [
    ...(aberta ? [{ texto: "Nova análise", aoEscolher: () => novaAnalise(null) }] : []),
    ...(documento
      ? [{ texto: "Abrir documento negocial", aoEscolher: () => navegar({ tipo: "base", documento }) }]
      : []),
  ];
  const itensRevisao: Acao[] = [
    ...(aberta
      ? [
          { texto: "Editar", aoEscolher: () => setForm("editar") },
          ...(outras.length > 0 ? [{ texto: "Juntar", aoEscolher: () => setForm("juntar") }] : []),
          ...(f.pontos.length > 1 ? [{ texto: "Dividir", aoEscolher: () => setForm("dividir") }] : []),
          { texto: "Descartar", aoEscolher: () => void decidir("descartar"), desabilitado: revisao.ocupado },
        ]
      : []),
    ...(s !== "pendente"
      ? [
          {
            texto: s === "descartada" ? "Voltar a revisar" : "Desfazer a aceitação",
            aoEscolher: () => void decidir("desfazer"),
            desabilitado: revisao.ocupado,
          },
        ]
      : []),
  ];

  return (
    <article className="pagina-funcionalidade painel-funcionalidade" aria-label={`Página de ${f.id}`}>
      <nav className="pagina-trilha" aria-label="Trilha">
        <button type="button" className="btn btn-link" onClick={aoVoltar}>
          ← Todas as funcionalidades
        </button>
        <span className="muted">
          {" "}
          / {f.areaNome} / {f.id}
        </span>
      </nav>

      <header className="painel-cabecalho">
        <div className="pf-titulo">
          <div className="linha-cabecalho">
            <span className="painel-id">{f.id}</span>
            <Selo>{s === "juntada" && f.juntadaEm ? `juntada em ${f.juntadaEm}` : SITUACAO[s]}</Selo>
            {f.documento ? <Selo>documentada</Selo> : f.explicacao && <Selo>em explicação</Selo>}
            {f.semPontoDeEntrada && <Selo>sem ponto de entrada no código</Selo>}
            <span className="muted">Área {f.areaNome}</span>
          </div>
          <h3>{f.nome}</h3>
          <p>
            {f.descricao}
            {f.editada && <span className="muted editada"> (editada pelo analista)</span>}
          </p>
          {f.atores.length > 0 && (
            <p className="muted">
              <strong>Atores:</strong> {f.atores.join(" · ")}
            </p>
          )}
        </div>
        <BarraAcoes
          principal={
            s === "pendente"
              ? { texto: "Aceitar", aoEscolher: () => void decidir("aceitar"), desabilitado: revisao.ocupado }
              : undefined
          }
          secundarias={acoesDoCabecalho}
          rotulo={`Ações de ${f.id}`}
          alinhamento="fim"
        >
          {s !== "juntada" && itensRevisao.length > 0 && (
            <MenuAcoes rotulo="Revisar mapeamento" itens={itensRevisao} />
          )}
        </BarraAcoes>
      </header>

      {form === "editar" && (
        <FormEditar
          f={f}
          ocupado={revisao.ocupado}
          aoCancelar={() => setForm(null)}
          aoSalvar={(d) => revisar({ tipo: "editar", funcionalidade: f.id, ...d })}
        />
      )}
      {form === "juntar" && (
        <FormJuntar
          f={f}
          outras={outras}
          ocupado={revisao.ocupado}
          aoCancelar={() => setForm(null)}
          aoJuntar={(destino) => revisar({ tipo: "juntar", origem: f.id, destino })}
        />
      )}
      {form === "dividir" && (
        <FormDividir
          f={f}
          ocupado={revisao.ocupado}
          aoCancelar={() => setForm(null)}
          aoDividir={(d) => revisar({ tipo: "dividir", funcionalidade: f.id, ...d })}
        />
      )}

      {erro && (
        <Alerta tom="erro" titulo="Não foi possível montar o painel.">
          {erro}
        </Alerta>
      )}
      {falha && <Alerta tom="erro" titulo={falha} />}
      {aviso && <p className="muted">{aviso}</p>}

      {p && (
        <>
          <section className="indicadores" aria-label="Indicadores">
            <button type="button" className="indicador" onClick={() => setAba("analises")}>
              <span>Análises</span>
              <strong>{p.analises.length}</strong>
              <span>
                {emAndamento} em andamento · {p.aRevisar.length} a revisar
              </span>
            </button>
            <button type="button" className="indicador" onClick={() => setAba("demandas")}>
              <span>Demandas</span>
              <strong>{p.demandas.length}</strong>
              <span>{p.demandas.filter((d) => d.aberta).length} abertas</span>
            </button>
            <div className="indicador">
              <span>Regras de negócio</span>
              <strong>{p.regras.length}</strong>
              <span className={aRevalidar.length > 0 ? "atencao" : undefined}>
                {aRevalidar.length} a revalidar
              </span>
            </div>
            <div className="indicador">
              <span>Perguntas em aberto</span>
              <strong>{p.perguntas.length}</strong>
              <span>
                {p.perguntas.filter((x) => !x.destinatario).length} sem destinatário
              </span>
            </div>
            <button type="button" className="indicador" onClick={() => setAba("codigo")}>
              <span>Pontos de entrada</span>
              <strong>{f.pontos.length}</strong>
              <span>{tipos.join(" · ")}</span>
            </button>
          </section>

          {aRevalidar.length > 0 && (
            <Alerta
              tom="atencao"
              titulo="O código mudou."
              acoes={
                <button type="button" className="btn" disabled={ocupado} onClick={revalidar}>
                  Revalidar
                </button>
              }
            >
              {aRevalidar.length === 1
                ? `A evidência de ${aRevalidar[0].id} não confere mais com o código.`
                : `As evidências de ${aRevalidar.map((r) => r.id).join(", ")} não conferem mais com o código.`}
            </Alerta>
          )}

          <nav className="abas-painel" aria-label="Seções do painel">
            {(
              [
                ["visao", "Visão geral"],
                ["analises", `Análises (${p.analises.length})`],
                ["demandas", `Demandas (${p.demandas.length})`],
                ["codigo", "Código"],
              ] as [Aba, string][]
            ).map(([valor, rotulo]) => (
              <button
                key={valor}
                type="button"
                className="aba-painel"
                aria-current={aba === valor ? "page" : undefined}
                onClick={() => setAba(valor)}
              >
                {rotulo}
              </button>
            ))}
          </nav>

          {aba === "visao" && (
            <div className="pf-colunas">
              <div className="painel-principal">
                {passos.some((x) => !x.feito) && (
                  <Secao titulo="Próximos passos" nivel={3}>
                    <ol className="pf-passos">
                      {passos.map((x) => (
                        <li key={x.nome} className={x.feito ? "passo-feito" : undefined}>
                          <span>
                            <strong>{x.nome}</strong>
                            <span className="muted">{x.texto}</span>
                          </span>
                          <Selo tom={x.feito ? "concluido" : "atencao"}>
                            {x.feito ? "Feito" : "Pendente"}
                          </Selo>
                        </li>
                      ))}
                    </ol>
                  </Secao>
                )}

                <Secao
                  titulo="Documento negocial"
                  nivel={3}
                  acoes={
                    f.explicacao && (
                      <span className="muted">
                        Explicação {f.explicacao.id} · <Selo>{ESTADO_ANALISE[f.explicacao.estado]}</Selo>
                      </span>
                    )
                  }
                >
                  {f.documento ? (
                    <p>A explicação foi aprovada e o documento negocial está na base.</p>
                  ) : f.explicacao ? (
                    <p className="muted">A explicação ainda não foi aprovada.</p>
                  ) : (
                    <EstadoVazio>Ainda não há explicação desta funcionalidade.</EstadoVazio>
                  )}
                  <BarraAcoes
                    principal={
                      !f.explicacao && aberta
                        ? { texto: "Explicar", aoEscolher: () => criar("explicar", null, null), desabilitado: ocupado }
                        : undefined
                    }
                    secundarias={[
                      ...(documento
                        ? [{ texto: "Abrir na base", aoEscolher: () => navegar({ tipo: "base", documento }) }]
                        : []),
                      ...(f.explicacao
                        ? [
                            {
                              texto: `Abrir a explicação ${f.explicacao.id}`,
                              aoEscolher: () => abrirAnalise(f.explicacao!.id),
                            },
                          ]
                        : []),
                    ]}
                    rotulo="Ações do documento negocial"
                  />
                </Secao>

                <CartaoManual
                  manual={p.manual}
                  aceita={s === "aceita"}
                  ocupado={ocupado}
                  aoGerar={() => criar("manual", null, null)}
                  aoAbrirAnalise={abrirAnalise}
                  aoAbrirDocumento={(d) => navegar({ tipo: "base", documento: d })}
                />

                {aberta && (
                  <Secao titulo="Entregáveis" nivel={3} rotulo="Entregáveis da funcionalidade">
                    <p className="muted">
                      Os documentos vivos da metodologia do projeto. Cada um começa na versão
                      inicial e evolui com as demandas; todo rascunho espera a revisão final.
                    </p>
                    <DocumentacaoDoMapa
                      projeto={projeto}
                      funcionalidade={f.id}
                      recarga={recarga + releitura}
                      api={apiDocumentacao}
                      apiTerminal={apiTerminal}
                      criarTela={criarTela}
                      aoExplicar={
                        !f.explicacao ? () => criar("explicar", null, null) : undefined
                      }
                      aoRevisar={(r) =>
                        navegar({ tipo: "revisao", projeto, documento: r.caminho, titulo: r.titulo })
                      }
                    />
                  </Secao>
                )}

                <Secao
                  titulo="Análises"
                  nivel={3}
                  acoes={
                    p.analises.length > 0 && (
                      <button type="button" className="btn btn-link" onClick={() => setAba("analises")}>
                        Ver todas ({p.analises.length})
                      </button>
                    )
                  }
                >
                  {p.analises.length === 0 ? (
                    <EstadoVazio>Nenhuma ainda.</EstadoVazio>
                  ) : (
                    <ul className="painel-lista" aria-label="Análises recentes">
                      {p.analises.slice(0, 5).map((a) => (
                        <li key={a.id}>
                          <button type="button" className="linha-analise" onClick={() => abrirAnalise(a.id)}>
                            <span className="mono">{a.id}</span>
                            <Selo>{NOME_FLUXO[a.fluxo] ?? a.fluxo}</Selo>
                            <span className="titulo-analise">{a.titulo}</span>
                            <Selo>{ESTADO_ANALISE[a.estado]}</Selo>
                            <span className="muted">{dia(a.atualizadaMs)}</span>
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </Secao>

                <Secao
                  titulo="Demandas"
                  nivel={3}
                  acoes={
                    <button type="button" className="btn btn-link" onClick={() => setAba("demandas")}>
                      {p.demandas.length > 0 ? `Ver todas (${p.demandas.length})` : "Ligar uma demanda"}
                    </button>
                  }
                >
                  {p.demandas.length === 0 ? (
                    <EstadoVazio>Nenhuma demanda ligada.</EstadoVazio>
                  ) : (
                    <ul className="painel-lista">
                      {p.demandas.map((d) => (
                        <li key={d.id}>
                          <button
                            type="button"
                            className="linha-analise"
                            onClick={() => navegar({ tipo: "demanda", projeto, demanda: d.id })}
                          >
                            <span className="mono">{d.id}</span>
                            <Selo>{rotuloDaDemanda(d)}</Selo>
                            <span className="titulo-analise">{d.titulo}</span>
                            <span className="muted">
                              {contar(d.perguntas.length, "pergunta", "perguntas")}
                            </span>
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </Secao>

                <Secao titulo="Regras de negócio" nivel={3}>
                  {p.regras.length === 0 ? (
                    <EstadoVazio>Nenhuma regra da base toca esta funcionalidade ainda.</EstadoVazio>
                  ) : (
                    <ul className="painel-lista" aria-label="Regras">
                      {p.regras.map((r) => (
                        <li key={r.id} className="linha-regra">
                          <div className="cabecalho-regra">
                            <button
                              type="button"
                              className="btn btn-link mono"
                              title="Abrir a regra na base"
                              onClick={() => navegar({ tipo: "base", documento: r.caminho })}
                            >
                              {r.id}
                            </button>
                            <strong className="titulo-regra">{r.titulo}</strong>
                            <Selo>{r.status === "a-revalidar" ? "a revalidar" : r.status}</Selo>
                          </div>
                          <div className="enunciado-regra">
                            <TextoRico texto={r.enunciado} />
                          </div>
                        </li>
                      ))}
                    </ul>
                  )}
                </Secao>
              </div>

              <aside className="painel-contexto" aria-label="Contexto">
                <Secao titulo="Pendências" nivel={3}>
                  {p.perguntas.length === 0 && p.aRevisar.length === 0 && aRevalidar.length === 0 && (
                    <EstadoVazio>Nada pendente.</EstadoVazio>
                  )}
                  {p.perguntas.length > 0 && (
                    <p>{contar(p.perguntas.length, "pergunta em aberto", "perguntas em aberto")}</p>
                  )}
                  {aRevalidar.length > 0 && (
                    <p>{contar(aRevalidar.length, "regra a revalidar", "regras a revalidar")}</p>
                  )}
                  {p.aRevisar.map((id) => (
                    <button key={id} type="button" className="btn btn-link" onClick={() => abrirAnalise(id)}>
                      {id} concluída, aguardando a sua revisão
                    </button>
                  ))}
                </Secao>

                <Secao titulo="Pontos de entrada" nivel={3}>
                  {evidencias ? (
                    <ul className="painel-lista-simples">
                      {f.pontos.map((x, i) => (
                        <li key={x.chave ?? `${x.nome}-${i}`}>
                          <code>{x.nome}</code>
                          <span className="muted">
                            {x.tipo ? NOME_TIPO_ENTRADA[x.tipo] : "fora da lista"} · {x.arquivo}:
                            {x.linhaInicial}
                          </span>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p className="muted">{contar(f.pontos.length, "ponto de entrada", "pontos de entrada")}</p>
                  )}
                  <button type="button" className="btn btn-link" onClick={() => setAba("codigo")}>
                    Ver o código
                  </button>
                </Secao>

                {(f.dadosLidos.length > 0 || f.dadosGravados.length > 0) && (
                  <Secao titulo="Dados" nivel={3}>
                    {f.dadosLidos.length > 0 && (
                      <p>
                        <strong>Lê:</strong> {f.dadosLidos.join(", ")}
                      </p>
                    )}
                    {f.dadosGravados.length > 0 && (
                      <p>
                        <strong>Grava:</strong> {f.dadosGravados.join(", ")}
                      </p>
                    )}
                  </Secao>
                )}

                {p.relacionadas.length > 0 && (
                  <Secao titulo="Relacionadas" nivel={3}>
                    {p.relacionadas.map((r) => (
                      <button key={r.id} type="button" className="btn btn-link" onClick={() => aoEscolher(r.id)}>
                        {r.id}
                        {r.nome ? ` · ${r.nome}` : ""}
                      </button>
                    ))}
                  </Secao>
                )}

                <Secao titulo="Atividade recente" nivel={3}>
                  <ul className="painel-lista-simples">
                    {p.atividade.map((x, i) => (
                      <li key={`${x.quandoMs}-${i}`} className="atividade">
                        <span className="muted">{dia(x.quandoMs)}</span> {x.texto}
                      </li>
                    ))}
                  </ul>
                </Secao>
              </aside>
            </div>
          )}

          {aba === "analises" && (
            <>
              {aberta && (
                <NovaAnalise
                  painel={p}
                  fluxoInicial={nova.fluxo}
                  demandaInicial={nova.demanda}
                  ocupado={ocupado}
                  ocultos={fluxosOcultos}
                  aoCriar={criar}
                />
              )}
              <Secao titulo="Análises" nivel={3} rotulo="Análises da funcionalidade">
                {p.analises.length === 0 ? (
                  <EstadoVazio>
                    Nenhuma ainda. Elas aparecem aqui quando forem abertas a partir desta
                    funcionalidade, pelas demandas ligadas ou quando tocarem o código dela.
                  </EstadoVazio>
                ) : (
                  <div className="tabela-rolavel">
                    <table className="tabela-painel">
                      <thead>
                        <tr>
                          <th scope="col">Análise</th>
                          <th scope="col">Fluxo</th>
                          <th scope="col">Título</th>
                          <th scope="col">Ligação</th>
                          <th scope="col">Demanda</th>
                          <th scope="col">Estado</th>
                          <th scope="col">Atualizada</th>
                        </tr>
                      </thead>
                      <tbody>
                        {p.analises.map((a) => (
                          <tr key={a.id}>
                            <td>
                              <button type="button" className="btn btn-link mono" onClick={() => abrirAnalise(a.id)}>
                                {a.id}
                              </button>
                            </td>
                            <td>{NOME_FLUXO[a.fluxo] ?? a.fluxo}</td>
                            <td>{a.titulo}</td>
                            <td className="muted">{NOME_LIGACAO[a.ligacao]}</td>
                            <td>{a.demanda ?? "—"}</td>
                            <td>
                              <Selo>{ESTADO_ANALISE[a.estado]}</Selo>
                              {a.perguntasAbertas > 0 && (
                                <span className="muted"> · {contar(a.perguntasAbertas, "pergunta", "perguntas")}</span>
                              )}
                            </td>
                            <td className="muted">{dia(a.atualizadaMs)}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
                <p className="muted">
                  "Pelo código": a análise cita nas evidências um arquivo de um ponto de entrada
                  desta funcionalidade, sem ter sido aberta por ela.
                </p>
              </Secao>
            </>
          )}

          {aba === "demandas" && (
            <>
              {aberta && (
                <Secao>
                  <form
                    className="ligar-demanda"
                    aria-label="Ligar demanda"
                    onSubmit={(e) => {
                      e.preventDefault();
                      if (ligar) ligarDemanda(ligar, true);
                    }}
                  >
                    <label>
                      Ligar uma demanda a esta funcionalidade
                      <select value={ligar} onChange={(e) => setLigar(e.target.value)}>
                        <option value="">Escolha a demanda</option>
                        {p.demandasDoProjeto
                          .filter((d) => !p.demandas.some((x) => x.id === d.id))
                          .map((d) => (
                            <option key={d.id} value={d.id}>
                              {d.id} · {d.titulo}
                            </option>
                          ))}
                      </select>
                    </label>
                    <button type="submit" className="btn" disabled={revisao.ocupado || !ligar}>
                      Ligar
                    </button>
                  </form>
                </Secao>
              )}
              {p.demandas.length === 0 && (
                <EstadoVazio>Nenhuma demanda ligada a esta funcionalidade.</EstadoVazio>
              )}
              {p.demandas.map((d) => (
                <CartaoDemanda
                  key={d.id}
                  d={d}
                  ocupado={ocupado || revisao.ocupado}
                  aoAbrir={() => navegar({ tipo: "demanda", projeto, demanda: d.id })}
                  aoAbrirAnalise={abrirAnalise}
                  aoDesligar={d.ligadaNoPainel ? () => ligarDemanda(d.id, false) : undefined}
                  aoAnalisarImpacto={() => novaAnalise("impacto", d.id)}
                />
              ))}
            </>
          )}

          {aba === "codigo" && (
            <Secao titulo="Pontos de entrada" nivel={3} rotulo="Código da funcionalidade">
              <ul className="pontos-entrada">
                {f.pontos.map((x, i) => (
                  <li key={x.chave ?? `${x.nome}-${i}`}>
                    <code>{x.nome}</code>
                    {x.tipo && <span className="muted"> · {NOME_TIPO_ENTRADA[x.tipo]}</span>}
                    <span className="muted">
                      {" "}
                      · {x.arquivo}:{x.linhaInicial}
                      {x.linhaFinal !== x.linhaInicial ? `-${x.linhaFinal}` : ""}
                    </span>
                    {x.foraDaLista && <Selo>fora da lista do mapa</Selo>}
                    <pre className="evidencia-ponto" aria-label={`Evidência de ${x.nome}`}>
                      <code>{x.trecho}</code>
                    </pre>
                  </li>
                ))}
              </ul>
            </Secao>
          )}
        </>
      )}
    </article>
  );
}
