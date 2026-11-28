import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { apiAnaliseTauri, mensagemDe, type ApiAnalise } from "./analise";
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
  Alerta,
  BarraAcoes,
  ItemLista,
  ListaItens,
  MenuAcoes,
  Secao,
  Selo,
  TituloSecao,
  type Acao,
} from "./componentes";
import { DocumentacaoDoMapa } from "./DocumentacaoDoMapa";
import type { ApiDocumentacao } from "./documentacaoMapa";
import { contar } from "./formatacao";
import {
  apiMapaTauri,
  NOME_MOTIVO,
  NOME_TIPO_ENTRADA,
  resumoReligacao,
  type ApiMapa,
  type Funcionalidade,
  type MapaFuncionalidades,
  type RevisaoDoMapa,
} from "./funcionalidades";
import type { Tela } from "./Navegacao";
import type { EstadoConversa } from "./PainelAnalise";
import type { ApiConhecimento } from "./conhecimento";
import { PainelFuncionalidade } from "./PainelFuncionalidade";
import { TerminalClaude, type EstadoTerminal } from "./TerminalClaude";
import { AjudaTermo } from "./AjudaTermo";
import { IconeReticencias } from "./icones";
import { lacunasPorPasta, passaNoFiltro, progressoDoMapa } from "./progressoMapa";
import type { ApiTerminal, ProjetoResumo } from "./terminal";
import type { TelaTerminal } from "./telaXterm";

/** Pontos ainda sem funcionalidade mostrados de uma vez (projetos grandes têm milhares). */
const MOSTRADOS = 100;
/** Pastas listadas em "Ainda sem mapa"; a lista completa fica em Código. */
const LACUNAS_MOSTRADAS = 5;

type Filtro = "todas" | "pendentes" | "aceitas" | "sem-ponto";

/** Visão do mapa: a lista para revisar ou a documentação por funcionalidade (#462). */
type Visao = "funcionalidades" | "documentacao";

const FILTROS: { valor: Filtro; rotulo: string }[] = [
  { valor: "todas", rotulo: "Todas" },
  { valor: "pendentes", rotulo: "A revisar" },
  { valor: "aceitas", rotulo: "Aceitas" },
  { valor: "sem-ponto", rotulo: "Sem ponto de entrada" },
];

function passa(f: Funcionalidade, filtro: Filtro, busca: string): boolean {
  if (filtro === "pendentes" && f.situacao !== "pendente") return false;
  if (filtro === "aceitas" && f.situacao !== "aceita") return false;
  if (filtro === "sem-ponto" && !f.semPontoDeEntrada) return false;
  const t = busca.trim().toLowerCase();
  if (!t) return true;
  return [f.id, f.nome, f.descricao, ...f.pontos.map((p) => p.nome)].some((x) =>
    x.toLowerCase().includes(t),
  );
}

function LinhaFuncionalidade({
  f,
  outras,
  revisao,
  aoAbrir,
  aoExplicar,
  aoAbrirExplicacao,
  aoAbrirDocumento,
}: {
  f: Funcionalidade;
  outras: Funcionalidade[];
  revisao: Revisao;
  /** Abre o painel da funcionalidade. */
  aoAbrir: (f: Funcionalidade) => void;
  aoExplicar?: (f: Funcionalidade) => void;
  aoAbrirExplicacao?: (analise: string) => void;
  aoAbrirDocumento?: (documento: string) => void;
}) {
  const [form, setForm] = useState<Form>(null);
  const { ocupado } = revisao;
  const s = f.situacao;
  const aberta = s === "pendente" || s === "aceita";
  const revisar = (r: RevisaoDoMapa) =>
    revisao
      .aoRevisar(r)
      .then(() => setForm(null))
      .catch(() => undefined);
  const decidir = (decisao: "aceitar" | "descartar" | "desfazer") =>
    revisar({ tipo: "decidir", funcionalidade: f.id, decisao });
  const podeExplicar = aberta && !!aoExplicar && !f.explicacao;

  // Uma ação principal por linha; o resto vai para o menu "⋯".
  let principal: { texto: string; aoClicar: () => void } | null = null;
  if (s === "pendente") principal = { texto: "Aceitar", aoClicar: () => void decidir("aceitar") };
  else if (podeExplicar) principal = { texto: "Explicar", aoClicar: () => aoExplicar!(f) };
  else if (s === "descartada")
    principal = { texto: "Voltar a revisar", aoClicar: () => void decidir("desfazer") };

  const itens: Acao[] = [{ texto: "Abrir o painel", aoEscolher: () => aoAbrir(f) }];
  if (aberta) {
    itens.push({ texto: "Editar", aoEscolher: () => setForm("editar") });
    if (outras.length > 0) itens.push({ texto: "Juntar", aoEscolher: () => setForm("juntar") });
    if (f.pontos.length > 1) itens.push({ texto: "Dividir", aoEscolher: () => setForm("dividir") });
    if (s === "pendente" && podeExplicar)
      itens.push({ texto: "Explicar", aoEscolher: () => aoExplicar!(f) });
    itens.push({ texto: "Descartar", aoEscolher: () => void decidir("descartar") });
  }
  if (s === "aceita") itens.push({ texto: "Desfazer", aoEscolher: () => void decidir("desfazer") });
  if (f.explicacao && aoAbrirExplicacao) {
    const id = f.explicacao.id;
    itens.push({
      texto: `Explicação ${id} (${ESTADO_ANALISE[f.explicacao.estado]})`,
      aoEscolher: () => aoAbrirExplicacao(id),
    });
  }
  if (f.documento && aoAbrirDocumento) {
    const documento = f.documento;
    itens.push({ texto: "Documento negocial", aoEscolher: () => aoAbrirDocumento(documento) });
  }
  const menu = s === "juntada" ? itens.slice(0, 1) : itens;

  return (
    <li
      id={idDoCartao(f.id)}
      className={`linha-funcionalidade situacao-${s}`}
      aria-label={`${f.id}: ${f.nome}`}
    >
      <div className="linha-principal">
        <button type="button" className="linha-nome" onClick={() => aoAbrir(f)}>
          <span className="painel-id">{f.id}</span> <strong>{f.nome}</strong>
        </button>
        <span className="linha-etiquetas">
          {f.documento ? <Selo>documentada</Selo> : f.explicacao && <Selo>em explicação</Selo>}
          {f.semPontoDeEntrada && <Selo>sem ponto de entrada</Selo>}
          <Selo>{s === "juntada" && f.juntadaEm ? `juntada em ${f.juntadaEm}` : SITUACAO[s]}</Selo>
        </span>
        {principal && (
          <button
            type="button"
            className={s === "pendente" ? "btn btn-sm btn-primary" : "btn btn-sm"}
            disabled={ocupado}
            aria-label={`${principal.texto} ${f.id}`}
            onClick={principal.aoClicar}
          >
            {principal.texto}
          </button>
        )}
        <MenuAcoes rotulo={`Mais ações para ${f.id}`} icone={<IconeReticencias />} itens={menu} />
      </div>
      {form === "editar" && (
        <FormEditar
          f={f}
          ocupado={ocupado}
          aoCancelar={() => setForm(null)}
          aoSalvar={(d) => revisar({ tipo: "editar", funcionalidade: f.id, ...d })}
        />
      )}
      {form === "juntar" && (
        <FormJuntar
          f={f}
          outras={outras}
          ocupado={ocupado}
          aoCancelar={() => setForm(null)}
          aoJuntar={(destino) => revisar({ tipo: "juntar", origem: f.id, destino })}
        />
      )}
      {form === "dividir" && (
        <FormDividir
          f={f}
          ocupado={ocupado}
          aoCancelar={() => setForm(null)}
          aoDividir={(d) => revisar({ tipo: "dividir", funcionalidade: f.id, ...d })}
        />
      )}
    </li>
  );
}

const idDoCartao = (id: string) => `funcionalidade-${id}`;

/**
 * Árvore lateral: as áreas e, em cada uma, as funcionalidades, com a situação.
 * Escolher uma rola até o cartão dela; as que o filtro ou a busca escondem ficam
 * esmaecidas e, escolhidas, limpam o filtro.
 */
function ArvoreDoMapa({
  mapa,
  visivel,
  selecionada,
  aoEscolher,
}: {
  mapa: MapaFuncionalidades;
  visivel: (f: Funcionalidade) => boolean;
  selecionada: string | null;
  aoEscolher: (f: Funcionalidade) => void;
}) {
  const [fechadas, setFechadas] = useState<ReadonlySet<string>>(new Set());
  const alternar = (area: string) =>
    setFechadas((atual) => {
      const novo = new Set(atual);
      if (novo.has(area)) novo.delete(area);
      else novo.add(area);
      return novo;
    });
  return (
    <nav className="arvore-mapa" aria-label="Árvore de funcionalidades">
      <ul>
        {mapa.areas.map((a) => {
          const aberta = !fechadas.has(a.area);
          const pendentes = a.funcionalidades.filter((f) => f.situacao === "pendente").length;
          return (
            <li key={a.area}>
              <button
                type="button"
                className="arvore-area"
                aria-expanded={aberta}
                onClick={() => alternar(a.area)}
              >
                <span aria-hidden="true">{aberta ? "▾" : "▸"}</span> {a.nome}{" "}
                <span className="muted">
                  {a.funcionalidades.length}
                  {pendentes > 0 ? ` · ${pendentes} a revisar` : ""}
                </span>
              </button>
              {aberta && (
                <ul aria-label={a.nome}>
                  {a.funcionalidades.map((f) => (
                    <li key={f.id}>
                      <button
                        type="button"
                        className={`arvore-funcionalidade situacao-${f.situacao}${visivel(f) ? "" : " fora-do-filtro"}`}
                        aria-current={selecionada === f.id ? "true" : undefined}
                        title={`${f.id} · ${f.nome} (${SITUACAO[f.situacao]})`}
                        onClick={() => aoEscolher(f)}
                      >
                        <span className="arvore-marca" aria-hidden="true">
                          {f.situacao === "aceita" ? "✓" : "•"}
                        </span>
                        <span className="painel-id">{f.id}</span> {f.nome}
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </li>
          );
        })}
      </ul>
    </nav>
  );
}

/** Sessão de mapeamento: o recorte e a orientação, depois o terminal do Claude. */
function SessaoDeMapeamento({
  projeto,
  api,
  pastaInicial = "",
  apiTerminal,
  criarTela,
  aoFechar,
  aoMudarConversa,
}: {
  projeto: string;
  api: ApiMapa;
  /** Recorte já preenchido (o "Mapear" de uma lacuna da cobertura). */
  pastaInicial?: string;
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
  aoFechar: () => void;
  aoMudarConversa?: (estado: EstadoConversa) => void;
}) {
  const [pasta, setPasta] = useState(pastaInicial);
  const [area, setArea] = useState("");
  const [orientacao, setOrientacao] = useState("");
  const [pedido, setPedido] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const vazio = (t: string) => (t.trim() ? t.trim() : null);
  // Quem fecha a aba do mapa (a navegação) pergunta antes de encerrar o Claude vivo (#257).
  const [terminal, setTerminal] = useState<EstadoTerminal | null>(null);
  const estadoConversa: EstadoConversa = terminal?.fase === "aberto" ? "aberta" : "nenhuma";
  const avisarConversa = useRef(aoMudarConversa);
  avisarConversa.current = aoMudarConversa;
  useEffect(() => {
    avisarConversa.current?.(estadoConversa);
  }, [estadoConversa]);
  useEffect(() => () => avisarConversa.current?.("nenhuma"), []);
  return (
    <Secao
      titulo="Mapear com o Claude"
      nivel={3}
      rotulo="Sessão de mapeamento"
      className="sessao-mapeamento"
      acoes={
        <button type="button" className="btn" onClick={aoFechar}>
          {pedido ? "Encerrar a sessão" : "Cancelar"}
        </button>
      }
    >
      {pedido === null ? (
        <form
          className="form-mapeamento"
          aria-label="Começar o mapeamento"
          onSubmit={(e) => {
            e.preventDefault();
            setErro(null);
            api
              .pedido(vazio(pasta), vazio(area), vazio(orientacao))
              .then(setPedido)
              .catch((e: unknown) => setErro(mensagemDe(e)));
          }}
        >
          <p className="muted">
            O agente lê os pontos de entrada do código, agrupa-os em funcionalidades de negócio e
            registra cada uma neste mapa, que você revisa aqui. Uma sessão nova continua o mapa de
            onde parou. Em projetos grandes, vá por pasta ou área.
          </p>
          <div className="field-pair">
            <label className="field">
              Pasta (opcional)
              <input
                type="text"
                value={pasta}
                placeholder="db, backend/src…"
                onChange={(e) => setPasta(e.target.value)}
              />
            </label>
            <label className="field">
              Área (opcional)
              <input
                type="text"
                value={area}
                placeholder="Cobrança"
                onChange={(e) => setArea(e.target.value)}
              />
            </label>
          </div>
          <label className="field">
            Orientação ao agente (opcional)
            <textarea
              rows={4}
              value={orientacao}
              placeholder="Ex.: comece pelo faturamento; ignore os utilitários de log."
              onChange={(e) => setOrientacao(e.target.value)}
            />
          </label>
          {erro && <Alerta tom="erro" titulo={erro} />}
          <div className="acoes-form-mapeamento">
            <button type="submit" className="btn btn-primary">
              Começar
            </button>
          </div>
        </form>
      ) : (
        <div className="terminal-mapeamento">
          <TerminalClaude
            projeto={projeto}
            pedido={{ promptInicial: pedido }}
            api={apiTerminal}
            criarTela={criarTela}
            aoMudarEstado={setTerminal}
          />
        </div>
      )}
    </Secao>
  );
}

/** O mapa do projeto, relido a cada aviso de mudança (`mudanca: funcionalidade`, alvo `mapa`). */
function useMapa(api: ApiMapa, vigiar: ApiAnalise["vigiar"], projeto: string) {
  const [mapa, setMapa] = useState<MapaFuncionalidades | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const ativo = useRef(true);
  const carregar = useCallback(() => {
    Promise.resolve()
      .then(() => api.mapa(projeto))
      .then((m) => {
        if (!ativo.current) return;
        setMapa(m);
        setErro(null);
      })
      .catch((e: unknown) => ativo.current && setErro(mensagemDe(e)));
  }, [api, projeto]);
  useEffect(() => {
    ativo.current = true;
    let parar: (() => void) | null = null;
    let desmontado = false;
    Promise.resolve()
      .then(() =>
        vigiar(projeto, (aviso) => {
          if (aviso.tipo === "reinicio") carregar();
          else if (aviso.analise === "mapa" && aviso.mudanca === "funcionalidade") carregar();
          // Explicação criada ou aprovada: o vínculo e o documento negocial mudam.
          else if (aviso.mudanca === "criada" || aviso.mudanca === "estado") carregar();
        }),
      )
      .then((f) => {
        if (desmontado) f?.();
        else parar = f ?? null;
      })
      .catch(() => undefined)
      .finally(() => {
        if (!desmontado) carregar();
      });
    return () => {
      desmontado = true;
      ativo.current = false;
      parar?.();
    };
  }, [vigiar, projeto, carregar]);
  return { mapa, setMapa, erro, recarregar: carregar };
}

function MapaDoProjeto({
  projeto,
  api,
  apiConhecimento,
  apiDocumentacao,
  vigiar,
  apiTerminal,
  criarTela,
  navegar,
  aoMudarConversa,
}: {
  projeto: string;
  api: ApiMapa;
  apiConhecimento?: ApiConhecimento;
  apiDocumentacao?: ApiDocumentacao;
  vigiar: ApiAnalise["vigiar"];
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
  navegar: (t: Tela) => void;
  aoMudarConversa?: (estado: EstadoConversa) => void;
}) {
  const { mapa, setMapa, erro, recarregar } = useMapa(api, vigiar, projeto);
  const [visao, setVisao] = useState<Visao>("funcionalidades");
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [sessao, setSessao] = useState(false);
  const [filtro, setFiltro] = useState<Filtro>("todas");
  const [busca, setBusca] = useState("");
  const [pastaSessao, setPastaSessao] = useState("");
  const [evidencias, setEvidencias] = useState(false);
  /** Funcionalidade aberta na página própria (escolhida na árvore). */
  const [selecionada, setSelecionada] = useState<string | null>(null);
  const escolher = (f: Funcionalidade) => setSelecionada(f.id);
  // Cada releitura do mapa (registro do agente, revisão, análise criada) relê o painel.
  const [recarga, setRecarga] = useState(0);
  useEffect(() => setRecarga((n) => n + 1), [mapa]);

  const revisao: Revisao = {
    ocupado,
    aoRevisar: (r) => {
      setOcupado(true);
      setFalha(null);
      return api
        .revisar(projeto, r)
        .then(setMapa)
        .catch((e: unknown) => {
          setFalha(mensagemDe(e));
          throw e;
        })
        .finally(() => setOcupado(false));
    },
  };

  const explicar = (f: Funcionalidade) => {
    setFalha(null);
    api
      .explicar(projeto, f.id)
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
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };
  const abrirDocumento = (documento: string) => navegar({ tipo: "base", documento });
  const abrirExplicacao = (id: string) =>
    navegar({ tipo: "analise", projeto, aberta: { id, abertura: { tipo: "reabrir" } } });
  const exportar = (formato: "markdown" | "word") => {
    setFalha(null);
    setAviso(null);
    api
      .escolherDestino(formato)
      .then((destino) => (destino ? api.exportar(projeto, destino, formato) : null))
      .then((gravado) => gravado && setAviso(`Exportado em ${gravado}`))
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  const religar = () => {
    if (!api.religar) return;
    setFalha(null);
    setAviso(null);
    api
      .religar(projeto)
      .then((r) => setAviso(resumoReligacao(r)))
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  const abertas = mapa?.areas.flatMap((a) => a.funcionalidades) ?? [];
  const pagina =
    (selecionada &&
      [...abertas, ...(mapa?.fechadas ?? [])].find((f) => f.id === selecionada)) ||
    null;
  const semTratamento = mapa?.completude?.semTratamento ?? [];
  const mostrados = semTratamento.slice(0, MOSTRADOS);
  const progresso = mapa && mapa.total > 0 ? progressoDoMapa(mapa) : null;
  const lacunas = lacunasPorPasta(semTratamento);
  const mapearPasta = (pasta: string) => {
    setPastaSessao(pasta);
    setSessao(true);
  };
  const contagem = (valor: Filtro) => abertas.filter((f) => passaNoFiltro(f, valor)).length;
  const documento = mapa?.documento;
  const documentoDoMapa: Acao[] = documento
    ? [
        { texto: "Abrir o documento", aoEscolher: () => abrirDocumento(documento) },
        { texto: "Exportar Markdown", aoEscolher: () => exportar("markdown") },
        { texto: "Exportar Word", aoEscolher: () => exportar("word") },
      ]
    : [];

  return (
    <div className={sessao ? "mapa-colunas com-sessao" : "mapa-colunas"}>
      {mapa && mapa.total > 0 && (
        <ArvoreDoMapa
          mapa={mapa}
          visivel={(f) => passa(f, filtro, busca)}
          selecionada={selecionada}
          aoEscolher={escolher}
        />
      )}
      <div className="mapa-revisao">
        <header className="mapa-cabecalho">
          <p className="muted" aria-label="Resumo do mapa">
            {mapa
              ? [
                  contar(mapa.total, "funcionalidade", "funcionalidades"),
                  contar(mapa.areas.length, "área", "áreas"),
                  `${mapa.pendentes} a revisar`,
                  `${mapa.documentadas} documentadas`,
                  mapa.completude
                    ? `${contar(mapa.completude.total, "ponto de entrada", "pontos de entrada")}, ${semTratamento.length} ainda sem funcionalidade`
                    : null,
                ]
                  .filter(Boolean)
                  .join(" · ")
              : "Carregando…"}
          </p>
          {mapa && mapa.total > 0 && !pagina && (
            <div className="mapa-visoes" role="tablist" aria-label="Visão">
              {(
                [
                  ["funcionalidades", "Funcionalidades"],
                  ["documentacao", "Documentação"],
                ] as const
              ).map(([valor, rotulo]) => (
                <button
                  key={valor}
                  type="button"
                  role="tab"
                  aria-selected={visao === valor}
                  className="btn"
                  onClick={() => setVisao(valor)}
                >
                  {rotulo}
                </button>
              ))}
            </div>
          )}
          <label className="opcao-evidencias">
            <input
              type="checkbox"
              checked={evidencias}
              onChange={(e) => setEvidencias(e.target.checked)}
            />
            Mostrar evidências de código
          </label>
          <BarraAcoes
            principal={
              sessao ? undefined : { texto: "Mapear com o Claude", aoEscolher: () => mapearPasta("") }
            }
            secundarias={documentoDoMapa}
            rotulo="Ações do mapa"
            alinhamento="fim"
          >
            <MenuAcoes
              rotulo="Mais"
              icone={<IconeReticencias />}
              itens={[
                ...(api.religar && mapa && mapa.total > 0
                  ? [{ texto: "Religar documentos", aoEscolher: religar }]
                  : []),
                { texto: "Atualizar", aoEscolher: recarregar },
              ]}
            />
          </BarraAcoes>
        </header>
        {erro && (
          <Alerta tom="erro" titulo="Não foi possível ler o mapa.">
            {erro}
          </Alerta>
        )}
        {falha && <Alerta tom="erro" titulo={falha} />}
        {aviso && <p className="muted">{aviso}</p>}
        {mapa?.erroCompletude && <Alerta tom="atencao" titulo={mapa.erroCompletude} />}

        {progresso && (
          <section className="progresso-mapa" aria-label="Progresso do mapa">
            <ol className="progresso-etapas">
              {[
                { rotulo: "mapeadas", n: progresso.mapeadas },
                { rotulo: "aceitas", n: progresso.aceitas },
                { rotulo: "explicadas", n: progresso.explicadas },
                { rotulo: "com manual", n: progresso.comManual },
              ].map((e) => (
                <li key={e.rotulo}>
                  <strong>{e.n ?? "—"}</strong>
                  {e.n !== null && <span className="muted"> de {progresso.mapeadas}</span>} {e.rotulo}
                </li>
              ))}
            </ol>
            {progresso.codigoMapeado !== null && (
              <div className="progresso-codigo">
                <span id="rotulo-codigo-mapeado">Código já mapeado</span>
                <div
                  className="barra-progresso"
                  role="progressbar"
                  aria-labelledby="rotulo-codigo-mapeado"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={progresso.codigoMapeado}
                >
                  <div style={{ width: `${progresso.codigoMapeado}%` }} />
                </div>
                <span className="muted">{progresso.codigoMapeado}%</span>
              </div>
            )}
          </section>
        )}

        {mapa && mapa.total === 0 && (
          <Secao rotulo="Mapa vazio" className="mapa-vazio">
            <p>
              <strong>O mapa ainda está vazio.</strong> Ele é o inventário, em linguagem de negócio,
              do que o sistema faz.
            </p>
            <ol>
              <li>O Claude lê o código e agrupa os pontos de entrada em funcionalidades.</li>
              <li>Você revisa: aceita, edita, junta, divide ou descarta.</li>
              <li>Depois explica cada uma e gera o manual do usuário.</li>
            </ol>
            {!sessao && (
              <button type="button" className="btn btn-primary" onClick={() => mapearPasta("")}>
                Mapear com o Claude
              </button>
            )}
            {semTratamento.length > 0 && (
              <details>
                <summary>Ver os pontos de entrada</summary>
                <ul aria-label="Ainda sem funcionalidade">
                  {mostrados.map((p) => (
                    <li key={p.chave}>
                      <code>{p.nome}</code>{" "}
                      <span className="muted">
                        · {NOME_TIPO_ENTRADA[p.tipo]} · {p.arquivo}:{p.linha}
                      </span>
                    </li>
                  ))}
                </ul>
                {semTratamento.length > mostrados.length && (
                  <p className="muted">
                    e mais {semTratamento.length - mostrados.length}: a lista completa está em Código.
                  </p>
                )}
              </details>
            )}
          </Secao>
        )}

        {pagina ? (
          <PainelFuncionalidade
            projeto={projeto}
            f={pagina}
            outras={abertas.filter((o) => o.id !== pagina.id)}
            recarga={recarga}
            api={api}
            apiConhecimento={apiConhecimento}
            revisao={revisao}
            evidencias={evidencias}
            navegar={navegar}
            aoVoltar={() => setSelecionada(null)}
            aoEscolher={setSelecionada}
            apiDocumentacao={apiDocumentacao}
            apiTerminal={apiTerminal}
            criarTela={criarTela}
          />
        ) : visao === "documentacao" && mapa && mapa.total > 0 ? (
          <DocumentacaoDoMapa
            projeto={projeto}
            api={apiDocumentacao}
            recarga={recarga}
            apiTerminal={apiTerminal}
            criarTela={criarTela}
            aoAbrirFuncionalidade={setSelecionada}
            aoRevisar={(r) =>
              navegar({ tipo: "revisao", projeto, documento: r.caminho, titulo: r.titulo })
            }
            aoConfigurar={() => navegar({ tipo: "configurar", projeto })}
          />
        ) : (
          <>
            {mapa && mapa.total > 0 && (
              <Secao rotulo="Funcionalidades">
                <div className="mapa-filtros" role="group" aria-label="Filtro">
                  {FILTROS.map((f) => (
                    <button
                      key={f.valor}
                      type="button"
                      className="btn"
                      aria-pressed={filtro === f.valor}
                      onClick={() => setFiltro(f.valor)}
                    >
                      {f.rotulo} ({contagem(f.valor)})
                    </button>
                  ))}
                  <input
                    type="search"
                    aria-label="Buscar funcionalidade"
                    placeholder="Buscar…"
                    value={busca}
                    onChange={(e) => setBusca(e.target.value)}
                  />
                </div>
                {mapa.areas.map((a) => {
                  const lista = a.funcionalidades.filter((f) => passa(f, filtro, busca));
                  if (lista.length === 0) return null;
                  return (
                    <div key={a.area} className="area-funcionalidades">
                      <TituloSecao nivel={3}>
                        {a.nome}{" "}
                        <span className="muted">
                          ({contar(a.funcionalidades.length, "funcionalidade", "funcionalidades")})
                        </span>
                      </TituloSecao>
                      <ul className="lista-compacta" aria-label={`Funcionalidades de ${a.nome}`}>
                        {lista.map((f) => (
                          <LinhaFuncionalidade
                            key={f.id}
                            f={f}
                            outras={abertas.filter((o) => o.id !== f.id)}
                            revisao={revisao}
                            aoAbrir={escolher}
                            aoExplicar={explicar}
                            aoAbrirExplicacao={abrirExplicacao}
                            aoAbrirDocumento={abrirDocumento}
                          />
                        ))}
                      </ul>
                    </div>
                  );
                })}
                {mapa.fechadas.length > 0 && (
                  <details className="funcionalidades-fechadas">
                    <summary>
                      {contar(mapa.fechadas.length, "descartada ou juntada", "descartadas ou juntadas")}
                    </summary>
                    <ul className="lista-compacta" aria-label="Descartadas e juntadas">
                      {mapa.fechadas.map((f) => (
                        <LinhaFuncionalidade
                          key={f.id}
                          f={f}
                          outras={[]}
                          revisao={revisao}
                          aoAbrir={escolher}
                        />
                      ))}
                    </ul>
                  </details>
                )}
              </Secao>
            )}

            {mapa && mapa.total > 0 && (lacunas.length > 0 || mapa.justificados.length > 0) && (
              <Secao
                rotulo="Ainda sem mapa"
                className="pontos-sem-funcionalidade"
                titulo={
                  <>
                    Ainda sem mapa
                    <AjudaTermo termo="ponto de entrada">
                      Telas, serviços e rotinas por onde o sistema é usado.
                    </AjudaTermo>
                  </>
                }
              >
                {lacunas.length > 0 && (
                  <>
                    <p>
                      {contar(
                        semTratamento.length,
                        "ponto de entrada ainda sem funcionalidade",
                        "pontos de entrada ainda sem funcionalidade",
                      )}
                      , nestas pastas:
                    </p>
                    <ListaItens rotulo="Lacunas da cobertura">
                      {lacunas.slice(0, LACUNAS_MOSTRADAS).map((l) => (
                        <ItemLista
                          key={l.pasta}
                          meta={contar(l.pontos, "ponto", "pontos")}
                          acoes={
                            <button
                              type="button"
                              className="btn btn-sm"
                              disabled={sessao}
                              aria-label={`Mapear ${l.pasta}`}
                              onClick={() => mapearPasta(l.pasta)}
                            >
                              Mapear
                            </button>
                          }
                        >
                          <code>{l.pasta}</code>
                        </ItemLista>
                      ))}
                    </ListaItens>
                    <button
                      type="button"
                      className="btn btn-link"
                      onClick={() => navegar({ tipo: "codigo", projeto })}
                    >
                      Ver todas as pastas em Código
                    </button>
                  </>
                )}
                {mapa.justificados.length > 0 && (
                  <details>
                    <summary>
                      {contar(mapa.justificados.length, "ponto justificado", "pontos justificados")}
                    </summary>
                    <ul aria-label="Justificados">
                      {mapa.justificados.map((j) => (
                        <li key={j.chave}>
                          <code>{j.nome}</code>{" "}
                          <span className="muted">
                            · {NOME_TIPO_ENTRADA[j.tipo]} · {NOME_MOTIVO[j.motivo]}:
                          </span>{" "}
                          {j.justificativa}{" "}
                          <button
                            type="button"
                            className="btn btn-link"
                            disabled={ocupado}
                            onClick={() =>
                              revisao
                                .aoRevisar({ tipo: "desfazer-justificativa", ponto: j.chave })
                                .catch(() => undefined)
                            }
                          >
                            Desfazer
                          </button>
                        </li>
                      ))}
                    </ul>
                  </details>
                )}
              </Secao>
            )}
          </>
        )}
      </div>
      {sessao && (
        <SessaoDeMapeamento
          key={pastaSessao}
          projeto={projeto}
          api={api}
          pastaInicial={pastaSessao}
          apiTerminal={apiTerminal}
          criarTela={criarTela}
          aoFechar={() => setSessao(false)}
          aoMudarConversa={aoMudarConversa}
        />
      )}
    </div>
  );
}

/**
 * Mapa de funcionalidades (#208), item próprio do menu lateral: o inventário das
 * funcionalidades do projeto em linguagem de negócio, que o agente monta a partir
 * dos pontos de entrada do código e o analista revisa aqui (aceitar, editar,
 * juntar, dividir, descartar). Não é uma análise: cada sessão de mapeamento
 * continua o mesmo mapa, e os identificadores ficam estáveis.
 */
export function TelaMapa({
  projetos,
  projetoInicial,
  api = apiMapaTauri,
  apiAnalise = apiAnaliseTauri,
  apiConhecimento,
  apiDocumentacao,
  apiTerminal,
  criarTela,
  navegar,
  aoMudarConversa,
}: {
  projetos: ProjetoResumo[];
  projetoInicial?: string;
  /** Para testes. */
  api?: ApiMapa;
  apiAnalise?: ApiAnalise;
  /** Para testes: a revalidação do painel. */
  apiConhecimento?: ApiConhecimento;
  /** Para testes: a visão "Documentação" (#462). */
  apiDocumentacao?: ApiDocumentacao;
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
  navegar: (t: Tela) => void;
  /** Estado da sessão de mapeamento, para a aba pedir confirmação ao fechar (#257). */
  aoMudarConversa?: (estado: EstadoConversa) => void;
}) {
  const [escolhido, setEscolhido] = useState(projetoInicial ?? "");
  const projeto = escolhido || projetos[0]?.id || "";
  const vigiar = useMemo<ApiAnalise["vigiar"]>(
    () => (p, aoAviso) => apiAnalise.vigiar(p, aoAviso),
    [apiAnalise],
  );
  return (
    <section className="tela-mapa page" aria-label="Mapa de funcionalidades">
      <header className="tela-mapa-titulo">
        <h2>Mapa de funcionalidades</h2>
        {projetos.length > 1 && (
          <label>
            Projeto{" "}
            <select value={projeto} onChange={(e) => setEscolhido(e.target.value)}>
              {projetos.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.nome}
                </option>
              ))}
            </select>
          </label>
        )}
      </header>
      {!projeto && <p className="muted">Nenhum projeto cadastrado.</p>}
      {projeto && (
        <MapaDoProjeto
          key={projeto}
          projeto={projeto}
          api={api}
          apiConhecimento={apiConhecimento}
          apiDocumentacao={apiDocumentacao}
          vigiar={vigiar}
          apiTerminal={apiTerminal}
          criarTela={criarTela}
          navegar={navegar}
          aoMudarConversa={aoMudarConversa}
        />
      )}
    </section>
  );
}
