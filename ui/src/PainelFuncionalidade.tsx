import Markdown from "markdown-to-jsx";
import { useEffect, useRef, useState } from "react";
import { mensagemDe, type Fluxo } from "./analise";
import { apiConhecimentoTauri, type ApiConhecimento } from "./conhecimento";
import { contar } from "./formatacao";
import { OPCOES_MARKDOWN_BASE } from "./markdownOpcoes";
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
  return (
    <section className="cartao-painel" aria-label="Manual do usuário">
      <header className="linha-cabecalho">
        <h3>Manual do usuário</h3>
        {m.situacao === "emAndamento" && (
          <span className="muted">
            {m.analise} · {ESTADO_ANALISE[m.estado]}
          </span>
        )}
        {m.situacao === "aprovado" &&
          (aRevisar ? (
            <span className="badge" data-tone="attention">
              a revisar
            </span>
          ) : (
            <span className="badge" data-tone="done">
              aprovado
            </span>
          ))}
      </header>
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
      <div className="acoes">
        {m.situacao === "nenhum" && aceita && (
          <button type="button" className="btn btn-primary" disabled={ocupado} onClick={aoGerar}>
            Gerar manual do usuário
          </button>
        )}
        {m.situacao === "emAndamento" && (
          <button type="button" className="btn" onClick={() => aoAbrirAnalise(m.analise)}>
            Abrir o manual {m.analise}
          </button>
        )}
        {m.situacao === "aprovado" && (
          <>
            {m.documento && (
              <button type="button" className="btn" onClick={() => aoAbrirDocumento(m.documento!)}>
                Abrir na base
              </button>
            )}
            <button type="button" className="btn" onClick={() => aoAbrirAnalise(m.analise)}>
              Abrir {m.analise}
            </button>
            {aceita && (
              <button
                type="button"
                className={aRevisar ? "btn btn-primary" : "btn"}
                disabled={ocupado}
                onClick={aoGerar}
              >
                Revisar o manual
              </button>
            )}
          </>
        )}
      </div>
    </section>
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
  return (
    <section className="cartao-painel demanda-painel" aria-label={`${d.id}: ${d.titulo}`}>
      <header className="linha-cabecalho">
        <button type="button" className="btn btn-link mono" onClick={aoAbrir}>
          {d.id}
        </button>
        <strong>{d.titulo}</strong>
        <span className="badge" data-tone={d.aberta ? undefined : "done"}>
          {d.aberta ? "aberta" : "encerrada"}
        </span>
        {d.criado && <span className="muted">desde {d.criado}</span>}
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
      <div className="acoes">
        {!d.impacto && d.aberta && (
          <button type="button" className="btn" disabled={ocupado} onClick={aoAnalisarImpacto}>
            Analisar impacto
          </button>
        )}
        {[d.impacto, d.historias, d.cenarios]
          .filter((e): e is EtapaDemanda => !!e)
          .map((e) => (
            <button key={e.analise} type="button" className="btn" onClick={() => aoAbrirAnalise(e.analise)}>
              Abrir {e.analise}
            </button>
          ))}
        <button type="button" className="btn" onClick={aoAbrir}>
          Abrir a demanda (matriz)
        </button>
        {aoDesligar && (
          <button type="button" className="btn" disabled={ocupado} onClick={aoDesligar}>
            Desligar
          </button>
        )}
      </div>
    </section>
  );
}

function NovaAnalise({
  painel,
  fluxoInicial,
  demandaInicial,
  ocupado,
  aoCriar,
}: {
  painel: Painel;
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
    <section className="cartao-painel" aria-label="Nova análise para esta funcionalidade">
      <header className="linha-cabecalho">
        <h3>Nova análise para esta funcionalidade</h3>
        <span className="muted">A entrada já vem com a funcionalidade e os pontos de entrada.</span>
      </header>
      <div className="novas-analises" role="group" aria-label="Fluxo">
        {NOVAS.map((n) => {
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
    </section>
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
  const menu = useRef<HTMLDetailsElement>(null);

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
  const escolherNoMenu = (acao: () => void) => {
    if (menu.current) menu.current.open = false;
    acao();
  };
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
            <span className="badge" data-tone={s === "aceita" ? "done" : undefined}>
              {s === "juntada" && f.juntadaEm ? `juntada em ${f.juntadaEm}` : SITUACAO[s]}
            </span>
            {f.documento ? (
              <span className="chip">documentada</span>
            ) : (
              f.explicacao && <span className="chip">em explicação</span>
            )}
            {f.semPontoDeEntrada && <span className="chip">sem ponto de entrada no código</span>}
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
        <div className="acoes">
          {s === "pendente" && (
            <button
              type="button"
              className="btn btn-primary"
              disabled={revisao.ocupado}
              onClick={() => revisar({ tipo: "decidir", funcionalidade: f.id, decisao: "aceitar" })}
            >
              Aceitar
            </button>
          )}
          {aberta && (
            <button type="button" className="btn" onClick={() => novaAnalise(null)}>
              Nova análise
            </button>
          )}
          {f.documento && (
            <button
              type="button"
              className="btn"
              onClick={() => navegar({ tipo: "base", documento: f.documento! })}
            >
              Abrir documento negocial
            </button>
          )}
          {s !== "juntada" && (
            <details className="menu-revisar" ref={menu}>
              <summary className="btn">Revisar mapeamento</summary>
              <div className="menu-itens" role="group" aria-label={`Revisão de ${f.id}`}>
                {aberta && (
                  <>
                    <button type="button" className="btn" onClick={() => escolherNoMenu(() => setForm("editar"))}>
                      Editar
                    </button>
                    {outras.length > 0 && (
                      <button type="button" className="btn" onClick={() => escolherNoMenu(() => setForm("juntar"))}>
                        Juntar
                      </button>
                    )}
                    {f.pontos.length > 1 && (
                      <button type="button" className="btn" onClick={() => escolherNoMenu(() => setForm("dividir"))}>
                        Dividir
                      </button>
                    )}
                    <button
                      type="button"
                      className="btn"
                      disabled={revisao.ocupado}
                      onClick={() =>
                        escolherNoMenu(() =>
                          revisar({ tipo: "decidir", funcionalidade: f.id, decisao: "descartar" }),
                        )
                      }
                    >
                      Descartar
                    </button>
                  </>
                )}
                {s !== "pendente" && (
                  <button
                    type="button"
                    className="btn"
                    disabled={revisao.ocupado}
                    onClick={() =>
                      escolherNoMenu(() =>
                        revisar({ tipo: "decidir", funcionalidade: f.id, decisao: "desfazer" }),
                      )
                    }
                  >
                    {s === "descartada" ? "Voltar a revisar" : "Desfazer a aceitação"}
                  </button>
                )}
              </div>
            </details>
          )}
        </div>
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

      {erro && <p className="erro">Não foi possível montar o painel: {erro}</p>}
      {falha && <p className="erro">{falha}</p>}
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
            <div role="status" className="alerta-revalidar">
              <span>
                <strong>O código mudou.</strong>{" "}
                {aRevalidar.length === 1
                  ? `A evidência de ${aRevalidar[0].id} não confere mais com o código.`
                  : `As evidências de ${aRevalidar.map((r) => r.id).join(", ")} não conferem mais com o código.`}
              </span>
              <button type="button" className="btn" disabled={ocupado} onClick={revalidar}>
                Revalidar
              </button>
            </div>
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
                  <section className="cartao-painel" aria-label="Próximos passos">
                    <h3>Próximos passos</h3>
                    <ol className="pf-passos">
                      {passos.map((x) => (
                        <li key={x.nome} className={x.feito ? "passo-feito" : undefined}>
                          <span>
                            <strong>{x.nome}</strong>
                            <span className="muted">{x.texto}</span>
                          </span>
                          <span className="muted">{x.feito ? "Feito" : "Pendente"}</span>
                        </li>
                      ))}
                    </ol>
                  </section>
                )}

                <section className="cartao-painel" aria-label="Documento negocial">
                  <header className="linha-cabecalho">
                    <h3>Documento negocial</h3>
                    {f.explicacao && (
                      <span className="muted">
                        Explicação {f.explicacao.id} · {ESTADO_ANALISE[f.explicacao.estado]}
                      </span>
                    )}
                  </header>
                  {f.documento ? (
                    <p>A explicação foi aprovada e o documento negocial está na base.</p>
                  ) : f.explicacao ? (
                    <p className="muted">A explicação ainda não foi aprovada.</p>
                  ) : (
                    <p className="muted">Ainda não há explicação desta funcionalidade.</p>
                  )}
                  <div className="acoes">
                    {f.documento && (
                      <button
                        type="button"
                        className="btn"
                        onClick={() => navegar({ tipo: "base", documento: f.documento! })}
                      >
                        Abrir na base
                      </button>
                    )}
                    {f.explicacao ? (
                      <button type="button" className="btn" onClick={() => abrirAnalise(f.explicacao!.id)}>
                        Abrir a explicação {f.explicacao.id}
                      </button>
                    ) : (
                      aberta && (
                        <button
                          type="button"
                          className="btn btn-primary"
                          disabled={ocupado}
                          onClick={() => criar("explicar", null, null)}
                        >
                          Explicar
                        </button>
                      )
                    )}
                  </div>
                </section>

                <CartaoManual
                  manual={p.manual}
                  aceita={s === "aceita"}
                  ocupado={ocupado}
                  aoGerar={() => criar("manual", null, null)}
                  aoAbrirAnalise={abrirAnalise}
                  aoAbrirDocumento={(d) => navegar({ tipo: "base", documento: d })}
                />

                <section className="cartao-painel" aria-label="Análises">
                  <header className="linha-cabecalho">
                    <h3>Análises</h3>
                    {p.analises.length > 0 && (
                      <button type="button" className="btn btn-link" onClick={() => setAba("analises")}>
                        Ver todas ({p.analises.length})
                      </button>
                    )}
                  </header>
                  {p.analises.length === 0 ? (
                    <p className="muted">Nenhuma ainda.</p>
                  ) : (
                    <ul className="painel-lista" aria-label="Análises recentes">
                      {p.analises.slice(0, 5).map((a) => (
                        <li key={a.id}>
                          <button type="button" className="linha-analise" onClick={() => abrirAnalise(a.id)}>
                            <span className="mono">{a.id}</span>
                            <span className="chip">{NOME_FLUXO[a.fluxo] ?? a.fluxo}</span>
                            <span className="titulo-analise">{a.titulo}</span>
                            <span>{ESTADO_ANALISE[a.estado]}</span>
                            <span className="muted">{dia(a.atualizadaMs)}</span>
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </section>

                <section className="cartao-painel" aria-label="Demandas">
                  <header className="linha-cabecalho">
                    <h3>Demandas</h3>
                    <button type="button" className="btn btn-link" onClick={() => setAba("demandas")}>
                      {p.demandas.length > 0 ? `Ver todas (${p.demandas.length})` : "Ligar uma demanda"}
                    </button>
                  </header>
                  {p.demandas.length === 0 ? (
                    <p className="muted">Nenhuma demanda ligada.</p>
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
                            <span className="chip">{d.aberta ? "aberta" : "encerrada"}</span>
                            <span className="titulo-analise">{d.titulo}</span>
                            <span className="muted">
                              {contar(d.perguntas.length, "pergunta", "perguntas")}
                            </span>
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </section>

                <section className="cartao-painel" aria-label="Regras de negócio">
                  <h3>Regras de negócio</h3>
                  {p.regras.length === 0 ? (
                    <p className="muted">Nenhuma regra da base toca esta funcionalidade ainda.</p>
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
                            <span className={r.status === "a-revalidar" ? "atencao" : "muted"}>
                              {r.status === "a-revalidar" ? "a revalidar" : r.status}
                            </span>
                          </div>
                          <div className="enunciado-regra">
                            <Markdown options={OPCOES_MARKDOWN_BASE}>{r.enunciado}</Markdown>
                          </div>
                        </li>
                      ))}
                    </ul>
                  )}
                </section>
              </div>

              <aside className="painel-contexto" aria-label="Contexto">
                <section className="cartao-painel" aria-label="Pendências">
                  <h3>Pendências</h3>
                  {p.perguntas.length === 0 && p.aRevisar.length === 0 && aRevalidar.length === 0 && (
                    <p className="muted">Nada pendente.</p>
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
                </section>

                <section className="cartao-painel" aria-label="Pontos de entrada">
                  <h3>Pontos de entrada</h3>
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
                </section>

                {(f.dadosLidos.length > 0 || f.dadosGravados.length > 0) && (
                  <section className="cartao-painel" aria-label="Dados">
                    <h3>Dados</h3>
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
                  </section>
                )}

                {p.relacionadas.length > 0 && (
                  <section className="cartao-painel" aria-label="Relacionadas">
                    <h3>Relacionadas</h3>
                    {p.relacionadas.map((r) => (
                      <button key={r.id} type="button" className="btn btn-link" onClick={() => aoEscolher(r.id)}>
                        {r.id}
                        {r.nome ? ` · ${r.nome}` : ""}
                      </button>
                    ))}
                  </section>
                )}

                <section className="cartao-painel" aria-label="Atividade recente">
                  <h3>Atividade recente</h3>
                  <ul className="painel-lista-simples">
                    {p.atividade.map((x, i) => (
                      <li key={`${x.quandoMs}-${i}`} className="atividade">
                        <span className="muted">{dia(x.quandoMs)}</span> {x.texto}
                      </li>
                    ))}
                  </ul>
                </section>
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
                  aoCriar={criar}
                />
              )}
              <section className="cartao-painel" aria-label="Análises da funcionalidade">
                <h3>Análises</h3>
                {p.analises.length === 0 ? (
                  <p className="muted">
                    Nenhuma ainda. Elas aparecem aqui quando forem abertas a partir desta
                    funcionalidade, pelas demandas ligadas ou quando tocarem o código dela.
                  </p>
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
                              {ESTADO_ANALISE[a.estado]}
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
              </section>
            </>
          )}

          {aba === "demandas" && (
            <>
              {aberta && (
                <form
                  className="cartao-painel ligar-demanda"
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
              )}
              {p.demandas.length === 0 && (
                <p className="muted">Nenhuma demanda ligada a esta funcionalidade.</p>
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
            <section className="cartao-painel" aria-label="Código da funcionalidade">
              <h3>Pontos de entrada</h3>
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
                    {x.foraDaLista && <span className="chip">fora da lista do mapa</span>}
                    <pre className="evidencia-ponto" aria-label={`Evidência de ${x.nome}`}>
                      <code>{x.trecho}</code>
                    </pre>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </>
      )}
    </article>
  );
}
