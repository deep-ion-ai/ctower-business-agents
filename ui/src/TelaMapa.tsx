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
import { contar } from "./formatacao";
import {
  apiMapaTauri,
  NOME_MOTIVO,
  NOME_TIPO_ENTRADA,
  type ApiMapa,
  type Funcionalidade,
  type MapaFuncionalidades,
  type RevisaoDoMapa,
} from "./funcionalidades";
import type { Tela } from "./Navegacao";
import type { ApiConhecimento } from "./conhecimento";
import { PainelFuncionalidade } from "./PainelFuncionalidade";
import { TerminalClaude } from "./TerminalClaude";
import type { ApiTerminal, ProjetoResumo } from "./terminal";
import type { TelaTerminal } from "./telaXterm";



/** Pontos sem tratamento mostrados de uma vez (projetos grandes têm milhares). */
const MOSTRADOS = 100;

type Filtro = "todas" | "pendentes" | "aceitas" | "sem-ponto";

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

function CartaoFuncionalidade({
  f,
  outras,
  revisao,
  evidencias = false,
  aoExplicar,
  aoAbrirExplicacao,
  aoAbrirDocumento,
  selecionada = false,
}: {
  f: Funcionalidade;
  outras: Funcionalidade[];
  revisao: Revisao;
  /** Escolhida na árvore lateral. */
  selecionada?: boolean;
  /** Mostra o trecho de código citado em cada ponto de entrada. */
  evidencias?: boolean;
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
  return (
    <section
      id={idDoCartao(f.id)}
      className={`funcionalidade situacao-${s}${selecionada ? " selecionada" : ""}`}
      aria-label={`${f.id}: ${f.nome}`}
      aria-current={selecionada ? "true" : undefined}
    >
      <header className="historia-cabecalho">
        <span className="painel-id">{f.id}</span>
        <strong>{f.nome}</strong>
        {f.documento ? (
          <span className="chip">documentada</span>
        ) : (
          f.explicacao && <span className="chip">em explicação</span>
        )}
        {f.semPontoDeEntrada && <span className="chip">sem ponto de entrada no código</span>}
        <span className="badge" data-tone={s === "aceita" ? "done" : undefined}>
          {s === "juntada" && f.juntadaEm ? `juntada em ${f.juntadaEm}` : SITUACAO[s]}
        </span>
      </header>
      {form === "editar" ? (
        <FormEditar
          f={f}
          ocupado={ocupado}
          aoCancelar={() => setForm(null)}
          aoSalvar={(d) => revisar({ tipo: "editar", funcionalidade: f.id, ...d })}
        />
      ) : (
        <p>
          {f.descricao}
          {f.editada && <span className="muted editada"> (editada pelo analista)</span>}
        </p>
      )}
      {f.atores.length > 0 && (
        <p className="muted">
          <strong>Atores:</strong> {f.atores.join(", ")}
        </p>
      )}
      {evidencias ? (
        <ul className="pontos-entrada" aria-label={`Pontos de entrada de ${f.id}`}>
          {f.pontos.map((p, i) => (
            <li key={p.chave ?? `${p.nome}-${i}`}>
              <code>{p.nome}</code>
              {p.tipo && <span className="muted"> · {NOME_TIPO_ENTRADA[p.tipo]}</span>}
              <span className="muted">
                {" "}
                · {p.arquivo}:{p.linhaInicial}
                {p.linhaFinal !== p.linhaInicial ? `-${p.linhaFinal}` : ""}
              </span>
              {p.foraDaLista && <span className="chip">fora da lista do mapa</span>}
              <pre className="evidencia-ponto" aria-label={`Evidência de ${p.nome}`}>
                <code>{p.trecho}</code>
              </pre>
            </li>
          ))}
        </ul>
      ) : (
        <p className="muted">
          {contar(f.pontos.length, "ponto de entrada", "pontos de entrada")}
          {f.pontos.some((p) => p.foraDaLista) && " (algum fora da lista do mapa)"}
        </p>
      )}
      {(f.dadosLidos.length > 0 || f.dadosGravados.length > 0) && (
        <p className="muted">
          {f.dadosLidos.length > 0 && (
            <>
              <strong>Lê:</strong> {f.dadosLidos.join(", ")}{" "}
            </>
          )}
          {f.dadosGravados.length > 0 && (
            <>
              <strong>Grava:</strong> {f.dadosGravados.join(", ")}
            </>
          )}
        </p>
      )}
      {f.relacionadas.length > 0 && (
        <p className="muted">
          <strong>Relacionadas:</strong> {f.relacionadas.join(", ")}
        </p>
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
      {s !== "juntada" && form === null && (
        <div className="acoes-revisao" role="group" aria-label={`Revisão de ${f.id}`}>
          {s === "pendente" && (
            <button
              type="button"
              className="btn btn-primary"
              disabled={ocupado}
              onClick={() => revisar({ tipo: "decidir", funcionalidade: f.id, decisao: "aceitar" })}
            >
              Aceitar
            </button>
          )}
          {aberta && (
            <>
              <button type="button" className="btn" disabled={ocupado} onClick={() => setForm("editar")}>
                Editar
              </button>
              {outras.length > 0 && (
                <button
                  type="button"
                  className="btn"
                  disabled={ocupado}
                  onClick={() => setForm("juntar")}
                >
                  Juntar
                </button>
              )}
              {f.pontos.length > 1 && (
                <button
                  type="button"
                  className="btn"
                  disabled={ocupado}
                  onClick={() => setForm("dividir")}
                >
                  Dividir
                </button>
              )}
              <button
                type="button"
                className="btn"
                disabled={ocupado}
                onClick={() =>
                  revisar({ tipo: "decidir", funcionalidade: f.id, decisao: "descartar" })
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
              disabled={ocupado}
              onClick={() => revisar({ tipo: "decidir", funcionalidade: f.id, decisao: "desfazer" })}
            >
              {s === "descartada" ? "Voltar a revisar" : "Desfazer"}
            </button>
          )}
          {aberta && aoExplicar && (
            <button
              type="button"
              className="btn"
              disabled={ocupado || !!f.explicacao}
              title={f.explicacao ? `Já explicada na análise ${f.explicacao.id}` : undefined}
              onClick={() => aoExplicar(f)}
            >
              Explicar
            </button>
          )}
          {f.explicacao && aoAbrirExplicacao && (
            <button
              type="button"
              className="btn"
              onClick={() => aoAbrirExplicacao(f.explicacao!.id)}
            >
              Explicação {f.explicacao.id} ({ESTADO_ANALISE[f.explicacao.estado]})
            </button>
          )}
          {f.documento && aoAbrirDocumento && (
            <button type="button" className="btn" onClick={() => aoAbrirDocumento(f.documento!)}>
              Documento negocial
            </button>
          )}
        </div>
      )}
    </section>
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
  apiTerminal,
  criarTela,
  aoFechar,
}: {
  projeto: string;
  api: ApiMapa;
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
  aoFechar: () => void;
}) {
  const [pasta, setPasta] = useState("");
  const [area, setArea] = useState("");
  const [orientacao, setOrientacao] = useState("");
  const [pedido, setPedido] = useState<string | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const vazio = (t: string) => (t.trim() ? t.trim() : null);
  return (
    <section className="sessao-mapeamento" aria-label="Sessão de mapeamento">
      <header className="sessao-cabecalho">
        <h3>Mapear com o Claude</h3>
        <button type="button" className="btn" onClick={aoFechar}>
          {pedido ? "Encerrar a sessão" : "Cancelar"}
        </button>
      </header>
      {pedido === null ? (
        <form
          className="form-funcionalidade"
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
          <label>
            Pasta (opcional)
            <input
              value={pasta}
              placeholder="db, backend/src…"
              onChange={(e) => setPasta(e.target.value)}
            />
          </label>
          <label>
            Área (opcional)
            <input value={area} placeholder="Cobrança" onChange={(e) => setArea(e.target.value)} />
          </label>
          <label>
            Orientação ao agente (opcional)
            <textarea
              rows={3}
              value={orientacao}
              onChange={(e) => setOrientacao(e.target.value)}
            />
          </label>
          {erro && <p className="erro">{erro}</p>}
          <div className="acoes">
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
          />
        </div>
      )}
    </section>
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
  vigiar,
  apiTerminal,
  criarTela,
  navegar,
}: {
  projeto: string;
  api: ApiMapa;
  apiConhecimento?: ApiConhecimento;
  vigiar: ApiAnalise["vigiar"];
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
  navegar: (t: Tela) => void;
}) {
  const { mapa, setMapa, erro, recarregar } = useMapa(api, vigiar, projeto);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [sessao, setSessao] = useState(false);
  const [filtro, setFiltro] = useState<Filtro>("todas");
  const [busca, setBusca] = useState("");
  const [todosSemTratamento, setTodosSemTratamento] = useState(false);
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

  const abertas = mapa?.areas.flatMap((a) => a.funcionalidades) ?? [];
  const pagina =
    (selecionada &&
      [...abertas, ...(mapa?.fechadas ?? [])].find((f) => f.id === selecionada)) ||
    null;
  const semTratamento = mapa?.completude?.semTratamento ?? [];
  const mostrados = todosSemTratamento ? semTratamento : semTratamento.slice(0, MOSTRADOS);

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
                    ? `${contar(mapa.completude.total, "ponto de entrada", "pontos de entrada")}, ${semTratamento.length} sem tratamento`
                    : null,
                ]
                  .filter(Boolean)
                  .join(" · ")
              : "Carregando…"}
          </p>
          <div className="acoes">
            <label className="opcao-evidencias">
              <input
                type="checkbox"
                checked={evidencias}
                onChange={(e) => setEvidencias(e.target.checked)}
              />
              Mostrar evidências de código
            </label>
            {!sessao && (
              <button type="button" className="btn btn-primary" onClick={() => setSessao(true)}>
                Mapear com o Claude
              </button>
            )}
            {mapa?.documento && (
              <>
                <button type="button" className="btn" onClick={() => abrirDocumento(mapa.documento!)}>
                  Abrir o documento
                </button>
                <button type="button" className="btn" onClick={() => exportar("markdown")}>
                  Exportar Markdown
                </button>
                <button type="button" className="btn" onClick={() => exportar("word")}>
                  Exportar Word
                </button>
              </>
            )}
            {mapa && mapa.total > 0 && (
              <button
                type="button"
                className="btn"
                title="Abre, numa aba própria, as issues do rastreador para vincular às funcionalidades"
                onClick={() => navegar({ tipo: "vincular", projeto })}
              >
                Vincular demandas
              </button>
            )}
            <button type="button" className="btn" onClick={recarregar}>
              Atualizar
            </button>
          </div>
        </header>
        {erro && <p className="erro">Não foi possível ler o mapa: {erro}</p>}
        {falha && <p className="erro">{falha}</p>}
        {aviso && <p className="muted">{aviso}</p>}
        {mapa?.erroCompletude && <p className="aviso">{mapa.erroCompletude}</p>}

        {mapa && mapa.total === 0 && (
          <p className="empty">
            O mapa ainda está vazio. Use "Mapear com o Claude" para o agente agrupar os pontos de
            entrada do código em funcionalidades de negócio.
          </p>
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
          />
        ) : (
          <>
        {mapa && mapa.total > 0 && (
          <section aria-label="Funcionalidades" className="funcionalidades-ao-vivo">
            <div className="mapa-filtros" role="group" aria-label="Filtro">
              {FILTROS.map((f) => (
                <button
                  key={f.valor}
                  type="button"
                  className="btn"
                  aria-pressed={filtro === f.valor}
                  onClick={() => setFiltro(f.valor)}
                >
                  {f.rotulo}
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
                  <h4>
                    {a.nome}{" "}
                    <span className="muted">({contar(a.funcionalidades.length, "funcionalidade", "funcionalidades")})</span>
                  </h4>
                  {lista.map((f) => (
                    <CartaoFuncionalidade
                      key={f.id}
                      f={f}
                      outras={abertas.filter((o) => o.id !== f.id)}
                      revisao={revisao}
                      evidencias={evidencias}
                      aoExplicar={explicar}
                      aoAbrirExplicacao={abrirExplicacao}
                      aoAbrirDocumento={abrirDocumento}
                    />
                  ))}
                </div>
              );
            })}
            {mapa.fechadas.length > 0 && (
              <details className="funcionalidades-fechadas">
                <summary>
                  {contar(mapa.fechadas.length, "descartada ou juntada", "descartadas ou juntadas")}
                </summary>
                {mapa.fechadas.map((f) => (
                  <CartaoFuncionalidade
                    key={f.id}
                    f={f}
                    outras={[]}
                    revisao={revisao}
                    evidencias={evidencias}
                  />
                ))}
              </details>
            )}
          </section>
        )}

        {mapa && (semTratamento.length > 0 || mapa.justificados.length > 0) && (
          <section
            aria-label="Pontos de entrada sem funcionalidade"
            className="pontos-sem-funcionalidade"
          >
            <h4>Pontos de entrada sem funcionalidade</h4>
            {semTratamento.length > 0 && (
              <>
                <p className="aviso">
                  {contar(
                    semTratamento.length,
                    "ponto de entrada ainda sem tratamento",
                    "pontos de entrada ainda sem tratamento",
                  )}
                  : uma sessão de mapeamento os atribui a funcionalidades ou justifica.
                </p>
                <ul aria-label="Sem tratamento">
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
                  <button type="button" className="btn" onClick={() => setTodosSemTratamento(true)}>
                    Mostrar mais {semTratamento.length - mostrados.length}
                  </button>
                )}
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
          </section>
        )}
          </>
        )}
      </div>
      {sessao && (
        <SessaoDeMapeamento
          projeto={projeto}
          api={api}
          apiTerminal={apiTerminal}
          criarTela={criarTela}
          aoFechar={() => setSessao(false)}
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
  apiTerminal,
  criarTela,
  navegar,
}: {
  projetos: ProjetoResumo[];
  projetoInicial?: string;
  /** Para testes. */
  api?: ApiMapa;
  apiAnalise?: ApiAnalise;
  /** Para testes: a revalidação do painel. */
  apiConhecimento?: ApiConhecimento;
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
  navegar: (t: Tela) => void;
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
          vigiar={vigiar}
          apiTerminal={apiTerminal}
          criarTela={criarTela}
          navegar={navegar}
        />
      )}
    </section>
  );
}
