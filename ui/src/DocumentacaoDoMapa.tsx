import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, EstadoVazio, Secao, Selo, TextoRico, type TomSelo } from "./componentes";
import {
  apiDocumentacaoTauri,
  avisarMudancaNaDocumentacao,
  elegibilidade,
  ESTADOS_DA_CELULA,
  estadoDaCelula,
  EVENTO_DOCUMENTACAO,
  filaDeRevisao,
  nomeCurto,
  pendencia,
  prerequisitos,
  resumoDoFora,
  resumoDoLote,
  ROTULO_ESTADO,
  ROTULO_FILTRO,
  rotuloDaContagem,
  rotuloDoDocumento,
  tituloDaRevisao,
  type ApiDocumentacao,
  type ColunaDeDocumentacao,
  type DocumentoDoTipo,
  type EstadoDaCelula,
  type EstadoDocumento,
  type ItemDoLote,
  type LinhaDeDocumentacao,
  type PropostaVista,
  type TabelaDeDocumentacao,
  type Trecho,
} from "./documentacaoMapa";
import { contar } from "./formatacao";
import { TerminalClaude } from "./TerminalClaude";
import type { ApiTerminal } from "./terminal";
import type { TelaTerminal } from "./telaXterm";

/** Intervalo da releitura da tabela enquanto uma sessão do lote está aberta. */
export const INTERVALO_DO_LOTE_MS = 3000;

/** Acima disso, as áreas abrem recolhidas (sem busca nem filtro). */
const LINHAS_PARA_RECOLHER = 30;

const TOM: Record<EstadoDocumento["estado"], TomSelo> = {
  em_dia: "concluido",
  proposta_pendente: "atencao",
  desatualizado: "erro",
  sem_documento: "neutro",
};

const ICONE: Record<EstadoDaCelula, string> = {
  em_dia: "✓",
  a_revisar: "✎",
  desatualizado: "!",
  pronta: "○",
  aguardando: "⧗",
};

/** Abre a revisão final de um rascunho (numa aba própria, na navegação). */
export interface PedidoDeRevisao {
  caminho: string;
  funcionalidade: string;
  titulo: string;
}

/** Detalhe da célula, embaixo do estado (modo cartão). */
function detalhe(d: DocumentoDoTipo): string | null {
  const e = d.estado;
  if (e.estado === "desatualizado") return `mudou: ${e.arquivos.join(", ")}`;
  if (e.estado === "proposta_pendente") return e.revisada ? "revisada, pronta para publicar" : "rascunho";
  return null;
}

/** As sessões do agente, uma por funcionalidade, em sequência. */
interface Sessoes {
  tipo: ColunaDeDocumentacao;
  fila: { funcionalidade: string; pedido: string }[];
  /** Índice da sessão aberta. */
  atual: number;
  feitas: string[];
  puladas: string[];
  /** As que o lote deixou de fora, com o motivo. */
  fora: Extract<ItemDoLote, { resultado: "pulada" }>[];
  /** "Parar depois desta": termina a sessão aberta e não abre as outras. */
  parar: boolean;
}

/** Busca sem caixa nem acento. */
const normalizar = (s: string) =>
  s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase();

// ---------------------------------------------------------------------------
// Peças compartilhadas com a aba da revisão

/** O texto da proposta, com as marcas do que mudou em relação à versão publicada. */
export function TrechosDaProposta({
  trechos,
  publicada,
}: {
  trechos: Trecho[];
  publicada: string | null;
}) {
  return (
    <div className="trechos-proposta" aria-label="Texto da proposta">
      {trechos
        .filter((t) => t.marca !== "removido" || publicada)
        .map((t, i) => (
          <div key={i} className={`trecho trecho-${t.marca}`} data-marca={t.marca}>
            {t.marca !== "mantido" && publicada && <span className="trecho-marca">{t.marca}</span>}
            {t.anterior && (
              <del className="trecho-anterior">
                <TextoRico texto={t.anterior} />
              </del>
            )}
            <TextoRico texto={t.texto} />
          </div>
        ))}
    </div>
  );
}

/** A proposta aberta para a revisão final, embutida (sem a navegação por abas). */
function RevisaoDaProposta({
  vista,
  ocupado,
  aoMarcar,
  aoPublicar,
  aoDescartar,
  aoFechar,
}: {
  vista: PropostaVista;
  ocupado: boolean;
  aoMarcar: () => void;
  aoPublicar: () => void;
  aoDescartar: () => void;
  aoFechar: () => void;
}) {
  const p = vista.proposta;
  const revisada = !!p.revisada;
  return (
    <Secao
      titulo={`${vista.nomeDoTipo} · ${vista.funcionalidade ?? ""} · proposta v${p.versao}`}
      nivel={3}
      rotulo="Revisão da proposta"
      className="revisao-proposta"
      acoes={
        <button type="button" className="btn" onClick={aoFechar}>
          Fechar
        </button>
      }
    >
      {revisada ? (
        <p className="muted">
          Revisada por {p.revisada!.por}. Uma edição ou "Gerar de novo" volta a proposta para
          rascunho.
        </p>
      ) : (
        <Alerta tom="atencao" titulo="Revisão final obrigatória" role="note">
          Confira o texto contra o código e as regras citadas antes de publicar.
          {vista.publicada ? ` A versão publicada é a ${vista.publicada}.` : " É a primeira versão."}
        </Alerta>
      )}
      <p>
        <strong>O que muda:</strong> {p.mudancas}
      </p>
      <TrechosDaProposta trechos={p.trechos} publicada={vista.publicada} />
      <div className="acoes-revisao-proposta">
        {!revisada && (
          <button type="button" className="btn" disabled={ocupado} onClick={aoMarcar}>
            Marcar como revisado
          </button>
        )}
        <button
          type="button"
          className="btn btn-primary"
          disabled={ocupado || !revisada}
          title={revisada ? undefined : "Marque como revisado antes de publicar"}
          onClick={aoPublicar}
        >
          Publicar a v{p.versao}
        </button>
        <button type="button" className="btn btn-link" disabled={ocupado} onClick={aoDescartar}>
          Descartar a proposta
        </button>
      </div>
    </Secao>
  );
}

// ---------------------------------------------------------------------------
// Modo cartão (página da funcionalidade)

interface PropsDaCelula {
  d: DocumentoDoTipo;
  linha: LinhaDeDocumentacao;
  coluna: ColunaDeDocumentacao;
  ocupado: boolean;
  aoAgir: (d: DocumentoDoTipo, linha: LinhaDeDocumentacao, coluna: ColunaDeDocumentacao) => void;
  /** Leva ao "Explicar" da funcionalidade, quando faltam regras; sem ele, só o aviso. */
  aoExplicar?: (funcionalidade: string) => void;
}

/** O que o cartão mostra de cada tipo: o documento, o estado e a ação (ou o que falta para ela). */
function ConteudoDaCelula({ d, linha, coluna, ocupado, aoAgir, aoExplicar }: PropsDaCelula) {
  const rotulo = rotuloDoDocumento(d);
  const extra = detalhe(d);
  const falta = d.acao === "Gerar versão inicial" ? pendencia(linha, coluna) : null;
  return (
    <>
      {rotulo && <span className="celula-documento-versao">{rotulo}</span>}
      <Selo tom={TOM[d.estado.estado]}>{ROTULO_ESTADO[d.estado.estado]}</Selo>
      {extra && <span className="muted celula-documento-detalhe">{extra}</span>}
      {falta ? (
        <>
          <span className="muted celula-documento-detalhe">
            {falta}: explique a funcionalidade e aprove a análise
          </span>
          {aoExplicar && (
            <button
              type="button"
              className="btn btn-sm"
              disabled={ocupado}
              aria-label={`Explicar ${linha.funcionalidade} para gerar ${coluna.nome}`}
              onClick={() => aoExplicar(linha.funcionalidade)}
            >
              Explicar
            </button>
          )}
        </>
      ) : (
        d.acao && (
          <button
            type="button"
            className="btn btn-sm"
            disabled={ocupado}
            aria-label={`${d.acao}: ${coluna.nome} de ${linha.funcionalidade}`}
            onClick={() => aoAgir(d, linha, coluna)}
          >
            {d.acao}
          </button>
        )
      )}
    </>
  );
}

// ---------------------------------------------------------------------------
// Modo tabela

/** Os pré-requisitos da versão inicial, abertos a partir da célula que aguarda. */
function PainelDePrerequisitos({
  linha,
  coluna,
  aoExplicar,
  aoAbrirFuncionalidade,
  aoFechar,
}: {
  linha: LinhaDeDocumentacao;
  coluna: ColunaDeDocumentacao;
  aoExplicar?: (funcionalidade: string) => void;
  aoAbrirFuncionalidade?: (funcionalidade: string) => void;
  aoFechar: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.focus();
  }, []);
  return (
    <div
      ref={ref}
      className="painel-prerequisitos"
      role="dialog"
      aria-label={`Pré-requisitos: ${coluna.nome} de ${linha.funcionalidade}`}
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === "Escape") aoFechar();
      }}
    >
      <div className="painel-prerequisitos-cabecalho">
        <strong>
          {coluna.nome} · {linha.funcionalidade}
        </strong>
        <button type="button" className="btn btn-link" aria-label="Fechar" onClick={aoFechar}>
          ×
        </button>
      </div>
      <p className="muted">Ainda não dá para gerar a versão inicial.</p>
      <ul className="lista-prerequisitos">
        {prerequisitos(linha, coluna).map((p) => (
          <li key={p.nome} data-ok={p.ok ? "sim" : "nao"}>
            <span className="marca-prerequisito" aria-hidden="true">
              {p.ok ? "✓" : "!"}
            </span>
            <span>
              <strong>{p.nome}</strong>
              <span className="muted"> {p.detalhe}</span>
            </span>
          </li>
        ))}
      </ul>
      <div className="acoes-prerequisitos">
        {aoExplicar && !linha.explicada && (
          <button
            type="button"
            className="btn btn-primary btn-sm"
            onClick={() => aoExplicar(linha.funcionalidade)}
          >
            Explicar a funcionalidade
          </button>
        )}
        {aoAbrirFuncionalidade && (
          <button
            type="button"
            className="btn btn-sm"
            onClick={() => aoAbrirFuncionalidade(linha.funcionalidade)}
          >
            Abrir a funcionalidade
          </button>
        )}
      </div>
    </div>
  );
}

/** Uma célula da tabela: o estado, com cor, ícone e texto; a ação aparece no foco. */
function CelulaDaTabela({
  d,
  linha,
  coluna,
  noLote,
  bloqueada,
  ocupado,
  aberta,
  aoAgir,
  aoAbrirPrerequisitos,
  painel,
}: {
  d: DocumentoDoTipo;
  linha: LinhaDeDocumentacao;
  coluna: ColunaDeDocumentacao;
  /** "gerando…" ou "na fila · 2º", quando a célula está no lote em andamento. */
  noLote: string | null;
  /** Gerar não está disponível agora (há um lote do agente em andamento). */
  bloqueada: boolean;
  ocupado: boolean;
  aberta: boolean;
  aoAgir: (d: DocumentoDoTipo, linha: LinhaDeDocumentacao, coluna: ColunaDeDocumentacao) => void;
  aoAbrirPrerequisitos: () => void;
  painel: React.ReactNode;
}) {
  const estado = estadoDaCelula(d, linha, coluna);
  const e = d.estado;
  const rotuloAcao = (acao: string) => `${acao}: ${coluna.nome} de ${linha.funcionalidade}`;
  let texto: string;
  switch (e.estado) {
    case "em_dia":
      texto = `v${e.versao} em dia`;
      break;
    case "proposta_pendente":
      texto = `${rotuloDoDocumento(d)} · ${e.revisada ? "revisado" : "a revisar"}`;
      break;
    case "desatualizado":
      texto = `v${e.versao} desatualizado`;
      break;
    case "sem_documento":
      texto = pendencia(linha, coluna) ?? "pronta para gerar";
  }
  if (noLote)
    return (
      <td className="celula-doc" data-estado="lote">
        <span className="estado-celula estado-lote" aria-live="polite">
          {noLote}
        </span>
      </td>
    );
  return (
    <td className="celula-doc" data-estado={estado}>
      {estado === "a_revisar" && d.acao ? (
        <button
          type="button"
          className="estado-celula estado-a_revisar"
          disabled={ocupado}
          aria-label={rotuloAcao("Revisar proposta")}
          onClick={() => aoAgir(d, linha, coluna)}
        >
          <span aria-hidden="true">{ICONE[estado]}</span> {texto}
        </button>
      ) : estado === "aguardando" ? (
        <button
          type="button"
          className="estado-celula estado-aguardando"
          aria-expanded={aberta}
          aria-label={`${texto}: pré-requisitos de ${coluna.nome} de ${linha.funcionalidade}`}
          onClick={aoAbrirPrerequisitos}
        >
          <span aria-hidden="true">{ICONE[estado]}</span> {texto}
        </button>
      ) : (
        <span
          className={`estado-celula estado-${estado}`}
          title={e.estado === "desatualizado" ? `mudou: ${e.arquivos.join(", ")}` : undefined}
        >
          <span aria-hidden="true">{ICONE[estado]}</span> {texto}
        </span>
      )}
      {(estado === "pronta" || estado === "desatualizado") && d.acao && (
        <button
          type="button"
          className="btn btn-sm acao-celula"
          disabled={ocupado || (estado === "pronta" && bloqueada)}
          title={bloqueada && estado === "pronta" ? "Há um lote do agente em andamento" : undefined}
          aria-label={rotuloAcao(d.acao)}
          onClick={() => aoAgir(d, linha, coluna)}
        >
          {estado === "pronta"
            ? coluna.geracao === "app"
              ? "Gerar na hora"
              : "Gerar"
            : "Propor atualização"}
        </button>
      )}
      {painel}
    </td>
  );
}

/** A gaveta do lote do agente: progresso por funcionalidade e o terminal da sessão. */
function GavetaDoLote({
  sessoes,
  nomes,
  recolhida,
  projeto,
  apiTerminal,
  criarTela,
  rascunhos,
  aoRecolher,
  aoParar,
  aoCancelar,
  aoFechar,
  aoIrParaRevisao,
}: {
  sessoes: Sessoes;
  nomes: Map<string, string>;
  recolhida: boolean;
  projeto: string;
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
  rascunhos: number;
  aoRecolher: () => void;
  aoParar: () => void;
  aoCancelar: () => void;
  aoFechar: () => void;
  aoIrParaRevisao?: () => void;
}) {
  const atual = sessoes.atual < sessoes.fila.length ? sessoes.fila[sessoes.atual] : null;
  const situacao = (f: string, i: number) =>
    sessoes.feitas.includes(f)
      ? "rascunho"
      : sessoes.puladas.includes(f)
        ? "não gerado"
        : i === sessoes.atual
          ? "gerando…"
          : "na fila";
  return (
    <aside className="gaveta-lote" aria-label="Lote em andamento" hidden={recolhida}>
      <div className="gaveta-lote-cabecalho">
        <strong>Lote · {sessoes.tipo.nome}</strong>
        <button type="button" className="btn btn-sm" onClick={aoRecolher}>
          Recolher
        </button>
      </div>
      <p className="muted">
        Uma sessão do Claude Code por funcionalidade, em sequência. Cada documento sai como
        rascunho.
      </p>
      <p role="status" aria-label="Progresso do lote">
        {atual
          ? `Sessão ${sessoes.atual + 1} de ${sessoes.fila.length}: ${atual.funcionalidade}`
          : `Lote concluído: ${contar(sessoes.feitas.length, "rascunho gerado", "rascunhos gerados")}`}
        {sessoes.puladas.length > 0 &&
          ` · ${contar(sessoes.puladas.length, "não gerada", "não geradas")}`}
        . Cada rascunho espera a revisão final.
      </p>
      <div
        className="barra-progresso"
        role="progressbar"
        aria-label="Sessões concluídas"
        aria-valuemin={0}
        aria-valuemax={sessoes.fila.length}
        aria-valuenow={sessoes.atual}
      >
        <div style={{ width: `${(100 * sessoes.atual) / sessoes.fila.length}%` }} />
      </div>
      <ol className="itens-lote" aria-label="Funcionalidades do lote">
        {sessoes.fila.map((f, i) => {
          const s = situacao(f.funcionalidade, i);
          return (
            <li key={f.funcionalidade} data-situacao={s}>
              <span className="painel-id">{f.funcionalidade}</span>{" "}
              <span>{nomes.get(f.funcionalidade) ?? ""}</span> <Selo>{s}</Selo>
            </li>
          );
        })}
      </ol>
      {sessoes.fora.length > 0 && (
        <p className="muted">
          Fora do lote:{" "}
          {sessoes.fora.map((p) => `${p.funcionalidade} (${p.motivo})`).join(" · ")}
        </p>
      )}
      {atual && (
        <div className="gaveta-lote-sessao">
          <p className="muted">
            Sessão ativa · {atual.funcionalidade}
            {sessoes.parar && " · o lote para depois desta"}
          </p>
          <div className="terminal-lote">
            <TerminalClaude
              key={atual.funcionalidade}
              projeto={projeto}
              pedido={{ promptInicial: atual.pedido }}
              api={apiTerminal}
              criarTela={criarTela}
            />
          </div>
        </div>
      )}
      <div className="gaveta-lote-acoes">
        {atual ? (
          <>
            <button type="button" className="btn" disabled={sessoes.parar} onClick={aoParar}>
              Parar depois desta
            </button>
            <button type="button" className="btn btn-link" onClick={aoCancelar}>
              Cancelar lote
            </button>
          </>
        ) : (
          <>
            {aoIrParaRevisao && rascunhos > 0 && (
              <button type="button" className="btn btn-primary" onClick={aoIrParaRevisao}>
                Ir para a revisão final
              </button>
            )}
            <button type="button" className="btn" onClick={aoFechar}>
              Fechar
            </button>
          </>
        )}
      </div>
    </aside>
  );
}

/**
 * Visão "Documentação" do mapa (#462, #499): uma linha por funcionalidade, agrupada
 * por área, e uma coluna por tipo vivo habilitado no projeto. A célula diz o estado
 * (em dia, a revisar, desatualizado, pronta para gerar ou o que falta); a ação
 * aparece no foco. A cobertura por tipo filtra a tabela; a seleção abre a barra
 * do lote, que conta as elegíveis por tipo. Os tipos montados pelo app saem na
 * hora; os redigidos pelo agente rodam uma sessão do Claude por funcionalidade,
 * em sequência, numa gaveta à direita. Todo documento gerado fica como rascunho
 * até a revisão final, que abre numa aba própria quando há `aoRevisar`.
 *
 * Com `funcionalidade`, mostra só os documentos dela, como cartão na página da
 * funcionalidade: sem a cobertura, o lote e a legenda.
 */
export function DocumentacaoDoMapa({
  projeto,
  api = apiDocumentacaoTauri,
  recarga = 0,
  apiTerminal,
  criarTela,
  aoAbrirFuncionalidade,
  funcionalidade,
  aoExplicar = aoAbrirFuncionalidade,
  aoRevisar,
  aoConfigurar,
}: {
  projeto: string;
  api?: ApiDocumentacao;
  /** Sobe quando o mapa muda: a tabela é relida. */
  recarga?: number;
  apiTerminal?: ApiTerminal;
  criarTela?: () => TelaTerminal;
  /** Abre o painel da funcionalidade (a atualização do que o agente redige vem da demanda). */
  aoAbrirFuncionalidade?: (funcionalidade: string) => void;
  /** Só esta funcionalidade, no cartão da página dela. */
  funcionalidade?: string;
  /** Leva ao "Explicar" de uma funcionalidade sem as regras que o tipo exige. */
  aoExplicar?: (funcionalidade: string) => void;
  /** Abre a revisão final do rascunho numa aba; sem ela, a revisão abre aqui. */
  aoRevisar?: (pedido: PedidoDeRevisao) => void;
  /** Leva a Configurações › Entregáveis. */
  aoConfigurar?: () => void;
}) {
  const [tabela, setTabela] = useState<TabelaDeDocumentacao | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [selecionadas, setSelecionadas] = useState<ReadonlySet<string>>(new Set());
  const [revisao, setRevisao] = useState<PropostaVista | null>(null);
  const [sessoes, setSessoes] = useState<Sessoes | null>(null);
  const [recolhida, setRecolhida] = useState(false);
  const [busca, setBusca] = useState("");
  const [area, setArea] = useState("");
  const [filtro, setFiltro] = useState<{ estado: EstadoDaCelula; tipo: string | null } | null>(
    null,
  );
  const [alternadas, setAlternadas] = useState<ReadonlySet<string>>(new Set());
  const [prereq, setPrereq] = useState<{ funcionalidade: string; tipo: string } | null>(null);
  const ativo = useRef(true);
  const painelDaProposta = useRef<HTMLDivElement>(null);

  const carregar = useCallback(() => {
    Promise.resolve()
      .then(() => api.tabela(projeto))
      .then((t) => {
        if (!ativo.current) return;
        setTabela(t);
        setErro(null);
      })
      .catch((e: unknown) => ativo.current && setErro(mensagemDe(e)));
  }, [api, projeto]);

  useEffect(() => {
    ativo.current = true;
    carregar();
    return () => {
      ativo.current = false;
    };
  }, [carregar, recarga]);

  // A revisão em outra aba (ou outro cartão) muda os documentos: relê.
  useEffect(() => {
    window.addEventListener(EVENTO_DOCUMENTACAO, carregar);
    return () => window.removeEventListener(EVENTO_DOCUMENTACAO, carregar);
  }, [carregar]);

  const colunas = useMemo(() => tabela?.colunas ?? [], [tabela]);

  // A revisão embutida abre acima da tabela: leva a vista até ela.
  const propostaAberta = revisao?.caminho ?? null;
  useEffect(() => {
    if (propostaAberta)
      painelDaProposta.current?.scrollIntoView?.({ behavior: "smooth", block: "start" });
  }, [propostaAberta]);

  // Lote do agente: relê a tabela enquanto a sessão está aberta e passa para a
  // próxima funcionalidade quando o rascunho da atual aparece.
  const emSessao = sessoes !== null && sessoes.atual < sessoes.fila.length;
  useEffect(() => {
    if (!emSessao) return;
    const id = setInterval(carregar, INTERVALO_DO_LOTE_MS);
    return () => clearInterval(id);
  }, [emSessao, carregar]);
  useEffect(() => {
    if (!sessoes || !tabela || sessoes.atual >= sessoes.fila.length) return;
    const atual = sessoes.fila[sessoes.atual];
    const col = tabela.colunas.findIndex((c) => c.tipo === sessoes.tipo.tipo);
    const linha = tabela.linhas.find((l) => l.funcionalidade === atual.funcionalidade);
    const d = col >= 0 ? linha?.documentos[col] : undefined;
    if (d && d.estado.estado !== "sem_documento") {
      const restantes = sessoes.fila.slice(sessoes.atual + 1).map((f) => f.funcionalidade);
      setSessoes({
        ...sessoes,
        atual: sessoes.parar ? sessoes.fila.length : sessoes.atual + 1,
        feitas: [...sessoes.feitas, atual.funcionalidade],
        puladas: sessoes.parar ? [...sessoes.puladas, ...restantes] : sessoes.puladas,
      });
      avisarMudancaNaDocumentacao();
    }
  }, [tabela, sessoes]);

  const executar = <T,>(p: () => Promise<T>): Promise<T | undefined> => {
    setOcupado(true);
    setFalha(null);
    setAviso(null);
    return p()
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        return undefined;
      })
      .finally(() => {
        setOcupado(false);
        carregar();
      });
  };

  const gerar = (coluna: ColunaDeDocumentacao, funcionalidades: string[]) =>
    executar(() => api.gerar(projeto, coluna.tipo, funcionalidades)).then((lote) => {
      if (!lote) return;
      const fila = lote.itens.flatMap((i) =>
        i.resultado === "sessao" ? [{ funcionalidade: i.funcionalidade, pedido: i.pedido }] : [],
      );
      const fora = lote.itens.filter(
        (i): i is Extract<ItemDoLote, { resultado: "pulada" }> => i.resultado === "pulada",
      );
      if (fila.length > 0) {
        setSessoes({ tipo: coluna, fila, atual: 0, feitas: [], puladas: [], fora, parar: false });
        setRecolhida(false);
      } else {
        setAviso(resumoDoLote(lote));
        avisarMudancaNaDocumentacao();
      }
      setSelecionadas(new Set());
    });

  const abrirProposta = (documento: string) =>
    executar(() => api.proposta(projeto, documento)).then((v) => v && setRevisao(v));

  const revisar = (caminho: string, funcionalidade: string, coluna: ColunaDeDocumentacao) => {
    if (aoRevisar)
      aoRevisar({ caminho, funcionalidade, titulo: tituloDaRevisao(funcionalidade, coluna) });
    else void abrirProposta(caminho);
  };

  const agir = (d: DocumentoDoTipo, linha: LinhaDeDocumentacao, coluna: ColunaDeDocumentacao) => {
    if (d.acao === "Gerar versão inicial") void gerar(coluna, [linha.funcionalidade]);
    else if (d.acao === "Revisar proposta" && d.caminho)
      revisar(d.caminho, linha.funcionalidade, coluna);
    else if (d.acao === "Propor atualização") {
      if (coluna.geracao === "app") {
        void executar(() =>
          api.proporAtualizacao(projeto, coluna.tipo, linha.funcionalidade),
        ).then((caminho) => {
          if (caminho) revisar(caminho, linha.funcionalidade, coluna);
        });
      } else {
        setAviso(
          `${coluna.nome} é redigido pelo agente: a atualização vem da demanda que mudou a funcionalidade. Abra a demanda no painel de ${linha.funcionalidade}.`,
        );
        aoAbrirFuncionalidade?.(linha.funcionalidade);
      }
    }
  };

  const decidir = (acao: "marcar" | "publicar" | "descartar") => {
    if (!revisao) return;
    const doc = revisao.caminho;
    if (acao === "marcar")
      void executar(() => api.marcarRevisada(projeto, doc)).then(
        (p) => p && setRevisao({ ...revisao, proposta: p }),
      );
    else if (acao === "publicar")
      void executar(() => api.publicar(projeto, doc)).then((ok) => {
        if (ok === undefined) return;
        setAviso(`${revisao.nomeDoTipo} v${revisao.proposta.versao} publicado.`);
        setRevisao(null);
        avisarMudancaNaDocumentacao();
      });
    else
      void executar(() => api.descartar(projeto, doc)).then(() => {
        setAviso("Proposta descartada.");
        setRevisao(null);
        avisarMudancaNaDocumentacao();
      });
  };

  if (erro)
    return (
      <Alerta tom="erro" titulo="Não foi possível ler a documentação do mapa.">
        {erro}
      </Alerta>
    );
  if (!tabela) return <p className="muted">Carregando…</p>;
  if (tabela.colunas.length === 0)
    return (
      <EstadoVazio>
        Nenhum documento vivo habilitado no projeto. Habilite em Configurações › Entregáveis um
        tipo que pertence à funcionalidade (caso de uso, catálogo de regras, especificação
        suplementar, manual).
      </EstadoVazio>
    );
  if (tabela.linhas.length === 0)
    return <p className="muted">O mapa ainda não tem funcionalidades para documentar.</p>;

  const avisos = (
    <>
      {falha && <Alerta tom="erro" titulo={falha} />}
      {aviso && (
        <p className="muted" role="status">
          {aviso}
        </p>
      )}
    </>
  );
  const painelDaRevisao = revisao && (
    <div ref={painelDaProposta}>
      <RevisaoDaProposta
        vista={revisao}
        ocupado={ocupado}
        aoMarcar={() => decidir("marcar")}
        aoPublicar={() => decidir("publicar")}
        aoDescartar={() => decidir("descartar")}
        aoFechar={() => setRevisao(null)}
      />
    </div>
  );
  const nomes = new Map(tabela.linhas.map((l) => [l.funcionalidade, l.nome]));
  const gaveta = sessoes && (
    <GavetaDoLote
      sessoes={sessoes}
      nomes={nomes}
      recolhida={recolhida}
      projeto={projeto}
      apiTerminal={apiTerminal}
      criarTela={criarTela}
      rascunhos={filaDeRevisao(tabela).length}
      aoRecolher={() => setRecolhida(true)}
      aoParar={() => setSessoes({ ...sessoes, parar: true })}
      aoCancelar={() => setSessoes(null)}
      aoFechar={() => setSessoes(null)}
      aoIrParaRevisao={
        aoRevisar
          ? () => {
              const primeiro = filaDeRevisao(tabela)[0];
              if (!primeiro) return;
              const c = colunas.find((x) => x.tipo === primeiro.tipo)!;
              revisar(primeiro.caminho, primeiro.funcionalidade, c);
            }
          : undefined
      }
    />
  );

  if (funcionalidade !== undefined) {
    const l = tabela.linhas.find((x) => x.funcionalidade === funcionalidade);
    if (!l) return <p className="muted">A funcionalidade não está aberta no mapa.</p>;
    return (
      <div className="documentacao-funcionalidade">
        <ul
          className="documentos-da-funcionalidade"
          aria-label={`Documentos vivos de ${l.funcionalidade}`}
        >
          {colunas.map((c, i) => (
            <li key={c.tipo} className="documento-da-funcionalidade" aria-label={c.nome}>
              <strong>{c.nome}</strong>
              <ConteudoDaCelula
                d={l.documentos[i]}
                linha={l}
                coluna={c}
                ocupado={ocupado || emSessao}
                aoAgir={agir}
                aoExplicar={aoExplicar}
              />
            </li>
          ))}
        </ul>
        {avisos}
        {gaveta}
        {painelDaRevisao}
      </div>
    );
  }

  // --- Tabela ---------------------------------------------------------------

  const estados = (l: LinhaDeDocumentacao) =>
    colunas.map((c, i) => estadoDaCelula(l.documentos[i], l, c));
  const termo = normalizar(busca.trim());
  const naBuscaENaArea = tabela.linhas.filter(
    (l) =>
      (!area || l.area === area) &&
      (!termo ||
        normalizar(l.nome).includes(termo) ||
        normalizar(l.funcionalidade).includes(termo)),
  );
  const noEstado = (l: LinhaDeDocumentacao, f: NonNullable<typeof filtro>) => {
    const es = estados(l);
    if (f.tipo === null) return es.includes(f.estado);
    const i = colunas.findIndex((c) => c.tipo === f.tipo);
    return es[i] === f.estado;
  };
  const visiveis = filtro ? naBuscaENaArea.filter((l) => noEstado(l, filtro)) : naBuscaENaArea;
  const contagemDoFiltro = (estado: EstadoDaCelula) =>
    naBuscaENaArea.filter((l) => estados(l).includes(estado)).length;

  const areas: { area: string; nome: string; linhas: LinhaDeDocumentacao[] }[] = [];
  for (const l of tabela.linhas) {
    const a = areas.find((x) => x.area === l.area);
    if (a) a.linhas.push(l);
    else areas.push({ area: l.area, nome: l.areaNome, linhas: [l] });
  }
  const recolherPorPadrao = tabela.linhas.length > LINHAS_PARA_RECOLHER && !termo && !filtro && !area;
  const aberta = (a: string) => (recolherPorPadrao ? alternadas.has(a) : !alternadas.has(a));
  const alternarArea = (a: string) =>
    setAlternadas((atual) => {
      const novo = new Set(atual);
      if (novo.has(a)) novo.delete(a);
      else novo.add(a);
      return novo;
    });
  const alternar = (ids: string[], marcar: boolean) =>
    setSelecionadas((atual) => {
      const novo = new Set(atual);
      for (const id of ids) {
        if (marcar) novo.add(id);
        else novo.delete(id);
      }
      return novo;
    });

  const fila = filaDeRevisao(tabela);
  const linhasSelecionadas = tabela.linhas.filter((l) => selecionadas.has(l.funcionalidade));
  const areasDaSelecao = new Set(linhasSelecionadas.map((l) => l.areaNome));
  const noLote = (f: string, tipo: string): string | null => {
    if (!sessoes || sessoes.tipo.tipo !== tipo) return null;
    const i = sessoes.fila.findIndex((x) => x.funcionalidade === f);
    if (i < 0 || sessoes.feitas.includes(f) || sessoes.puladas.includes(f)) return null;
    if (i === sessoes.atual) return "gerando…";
    if (i > sessoes.atual) return `na fila · ${i - sessoes.atual + 1}º`;
    return null;
  };
  const progressoDoLote = sessoes && `${Math.min(sessoes.atual + 1, sessoes.fila.length)}/${sessoes.fila.length}`;

  return (
    <div className="documentacao-mapa">
      <div className="documentacao-cabecalho">
        <p className="muted">
          Metodologia <strong>{tabela.metodologia === "rup" ? "RUP" : tabela.metodologia === "agil" ? "Ágil" : "Personalizado"}</strong>{" "}
          · {contar(colunas.length, "tipo de documento vivo", "tipos de documento vivo")} nas colunas ·
          todo documento sai como rascunho e passa pela revisão final
        </p>
        <div className="documentacao-cabecalho-acoes">
          {tabela.emBreve.map((t) => (
            <span key={t.tipo} className="selo-em-breve">
              <Selo tom="neutro">{t.nome} · geração em breve</Selo>
              {aoConfigurar && (
                <button type="button" className="btn btn-link" onClick={aoConfigurar}>
                  Configurações
                </button>
              )}
            </span>
          ))}
          {sessoes && recolhida && (
            <button type="button" className="btn btn-sm" onClick={() => setRecolhida(false)}>
              Lote {progressoDoLote}
            </button>
          )}
          <button
            type="button"
            className="btn btn-revisao-final"
            disabled={fila.length === 0}
            onClick={() => {
              setFiltro({ estado: "a_revisar", tipo: null });
              const primeiro = fila[0];
              if (aoRevisar && primeiro) {
                const c = colunas.find((x) => x.tipo === primeiro.tipo)!;
                revisar(primeiro.caminho, primeiro.funcionalidade, c);
              }
            }}
          >
            Revisão final · {contar(fila.length, "rascunho", "rascunhos")}
          </button>
        </div>
      </div>

      <section aria-label="Cobertura por tipo" className="cobertura-por-tipo">
        {tabela.cobertura.map((c, i) => {
          const coluna = colunas[i];
          const contagens = ESTADOS_DA_CELULA.map((estado) => ({
            estado,
            n: tabela.linhas.filter((l) => estadoDaCelula(l.documentos[i], l, coluna) === estado)
              .length,
          }));
          return (
            <div key={c.tipo} className="cartao-cobertura-tipo" aria-label={`Cobertura de ${c.nome}`}>
              <span className="muted">
                {c.nome} ·{" "}
                {coluna.geracao === "app" ? "montado pelo app · na hora" : "agente · 1 sessão por funcionalidade"}
              </span>
              <strong>
                {c.comDocumento} de {c.funcionalidades}
              </strong>{" "}
              <span className="muted">com documento</span>
              <div className="barra-empilhada" aria-hidden="true">
                {contagens.map(
                  (x) =>
                    x.n > 0 && (
                      <div
                        key={x.estado}
                        className={`faixa-${x.estado}`}
                        style={{ width: `${(100 * x.n) / c.funcionalidades}%` }}
                      />
                    ),
                )}
              </div>
              <div className="contagens-cobertura">
                {contagens.map(
                  (x) =>
                    x.n > 0 && (
                      <button
                        key={x.estado}
                        type="button"
                        className={`btn btn-link contagem-${x.estado}`}
                        aria-pressed={filtro?.estado === x.estado && filtro.tipo === c.tipo}
                        onClick={() => setFiltro({ estado: x.estado, tipo: c.tipo })}
                      >
                        {rotuloDaContagem(x.estado, x.n)}
                      </button>
                    ),
                )}
              </div>
            </div>
          );
        })}
      </section>

      <div className="filtros-documentacao" role="search" aria-label="Filtrar funcionalidades">
        <input
          type="search"
          placeholder="Buscar funcionalidade"
          aria-label="Buscar funcionalidade"
          value={busca}
          onChange={(e) => setBusca(e.target.value)}
        />
        <select aria-label="Área" value={area} onChange={(e) => setArea(e.target.value)}>
          <option value="">Área: todas ({areas.length})</option>
          {areas.map((a) => (
            <option key={a.area} value={a.area}>
              {a.nome}
            </option>
          ))}
        </select>
        <div className="abas-estado" role="group" aria-label="Estado">
          <button
            type="button"
            className="btn btn-sm"
            aria-pressed={filtro === null}
            onClick={() => setFiltro(null)}
          >
            Todas <span className="contagem">{naBuscaENaArea.length}</span>
          </button>
          {(["a_revisar", "desatualizado", "pronta", "aguardando"] as const).map((estado) => (
            <button
              key={estado}
              type="button"
              className="btn btn-sm"
              aria-pressed={filtro?.estado === estado && filtro.tipo === null}
              onClick={() => setFiltro({ estado, tipo: null })}
            >
              {ROTULO_FILTRO[estado]} <span className="contagem">{contagemDoFiltro(estado)}</span>
            </button>
          ))}
        </div>
        {filtro?.tipo && (
          <span className="muted">
            {ROTULO_FILTRO[filtro.estado]} em {colunas.find((c) => c.tipo === filtro.tipo)?.nome}{" "}
            <button type="button" className="btn btn-link" onClick={() => setFiltro(null)}>
              limpar
            </button>
          </span>
        )}
      </div>

      {avisos}
      {painelDaRevisao}

      <div className="tabela-documentacao-rolagem">
        <table className="tabela-documentacao" aria-label="Documentação por funcionalidade">
          <thead>
            <tr>
              <th scope="col" className="col-selecao">
                <span className="sr-only">Selecionar</span>
              </th>
              <th scope="col">Funcionalidade</th>
              {colunas.map((c) => (
                <th key={c.tipo} scope="col">
                  {c.nome}
                  <span className="muted cabecalho-geracao">
                    {c.geracao === "app" ? "app · na hora" : "agente"}
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {areas.map((a) => {
              const linhas = a.linhas.filter((l) => visiveis.includes(l));
              if (linhas.length === 0) return null;
              const ids = a.linhas.map((l) => l.funcionalidade);
              const todas = ids.every((f) => selecionadas.has(f));
              const expandida = aberta(a.area);
              return (
                <Fragment key={a.area}>
                  <tr className="grupo-area" aria-label={`Área ${a.nome}`}>
                    <td className="col-selecao">
                      <input
                        type="checkbox"
                        aria-label={`Selecionar a área ${a.nome}`}
                        checked={todas}
                        onChange={() => alternar(ids, !todas)}
                      />
                    </td>
                    <th scope="rowgroup">
                      <button
                        type="button"
                        className="btn btn-link grupo-area-alternar"
                        aria-expanded={expandida}
                        onClick={() => alternarArea(a.area)}
                      >
                        {expandida ? "▾" : "▸"} {a.nome}
                      </button>{" "}
                      <span className="muted">{a.linhas.length}</span>
                    </th>
                    {colunas.map((c, i) => (
                      <td key={c.tipo} className="muted cobertura-area">
                        {a.linhas.filter((l) => l.documentos[i].estado.estado !== "sem_documento").length}
                        /{a.linhas.length}
                      </td>
                    ))}
                  </tr>
                  {expandida &&
                    linhas.map((l) => (
                      <tr key={l.funcionalidade} aria-label={`${l.funcionalidade}: ${l.nome}`}>
                        <td className="col-selecao">
                          <input
                            type="checkbox"
                            aria-label={`Selecionar ${l.funcionalidade}`}
                            checked={selecionadas.has(l.funcionalidade)}
                            onChange={() =>
                              alternar([l.funcionalidade], !selecionadas.has(l.funcionalidade))
                            }
                          />
                        </td>
                        <th scope="row">
                          {aoAbrirFuncionalidade ? (
                            <button
                              type="button"
                              className="btn btn-link linha-nome"
                              onClick={() => aoAbrirFuncionalidade(l.funcionalidade)}
                            >
                              {l.nome}
                            </button>
                          ) : (
                            <strong>{l.nome}</strong>
                          )}{" "}
                          <span className="painel-id muted">{l.funcionalidade}</span>
                        </th>
                        {colunas.map((c, i) => {
                          const aberto =
                            prereq?.funcionalidade === l.funcionalidade && prereq.tipo === c.tipo;
                          return (
                            <CelulaDaTabela
                              key={c.tipo}
                              d={l.documentos[i]}
                              linha={l}
                              coluna={c}
                              noLote={noLote(l.funcionalidade, c.tipo)}
                              bloqueada={emSessao && c.geracao === "agente"}
                              ocupado={ocupado}
                              aberta={aberto}
                              aoAgir={agir}
                              aoAbrirPrerequisitos={() =>
                                setPrereq(aberto ? null : { funcionalidade: l.funcionalidade, tipo: c.tipo })
                              }
                              painel={
                                aberto && (
                                  <PainelDePrerequisitos
                                    linha={l}
                                    coluna={c}
                                    aoExplicar={aoExplicar}
                                    aoAbrirFuncionalidade={aoAbrirFuncionalidade}
                                    aoFechar={() => setPrereq(null)}
                                  />
                                )
                              }
                            />
                          );
                        })}
                      </tr>
                    ))}
                </Fragment>
              );
            })}
            {visiveis.length === 0 && (
              <tr>
                <td colSpan={colunas.length + 2} className="muted">
                  Nenhuma funcionalidade neste filtro.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <section aria-label="Legenda" className="legenda-documentacao muted">
        <span>
          <span className="estado-celula estado-em_dia">{ICONE.em_dia} em dia</span> a versão
          publicada confere com o código
        </span>
        <span>
          <span className="estado-celula estado-a_revisar">{ICONE.a_revisar} a revisar</span> há um
          rascunho à espera da revisão final
        </span>
        <span>
          <span className="estado-celula estado-desatualizado">{ICONE.desatualizado} desatualizado</span>{" "}
          o código citado mudou desde a versão publicada
        </span>
        <span>
          <span className="estado-celula estado-pronta">{ICONE.pronta} pronta para gerar</span> sem
          documento, com o que o tipo precisa
        </span>
        <span>
          <span className="estado-celula estado-aguardando">{ICONE.aguardando} aguardando</span> falta
          um pré-requisito: clique para ver qual
        </span>
      </section>

      {selecionadas.size > 0 && (
        <div className="barra-lote" role="group" aria-label="Geração em lote">
          <span>
            <strong>{contar(selecionadas.size, "funcionalidade selecionada", "funcionalidades selecionadas")}</strong>
            {areasDaSelecao.size === 1 && <span className="muted"> · {[...areasDaSelecao][0]}</span>}
          </span>
          {colunas.map((c, i) => {
            const e = elegibilidade(linhasSelecionadas, c, i);
            const n = e.elegiveis.length;
            const ocupadaPeloLote = emSessao && c.geracao === "agente";
            const detalheDoTipo =
              c.geracao === "app" ? "montado na hora" : contar(n, "sessão", "sessões") + " em sequência";
            const fora = resumoDoFora(e);
            return (
              <button
                key={c.tipo}
                type="button"
                className="btn botao-lote"
                disabled={ocupado || ocupadaPeloLote || n === 0}
                title={ocupadaPeloLote ? "Há um lote do agente em andamento" : undefined}
                aria-label={`Gerar ${nomeCurto(c)} · ${n} de ${linhasSelecionadas.length}`}
                onClick={() => void gerar(c, e.elegiveis)}
              >
                <strong>
                  Gerar {nomeCurto(c)} · {n} de {linhasSelecionadas.length}
                </strong>
                <span className="muted">
                  {n > 0 ? detalheDoTipo : "nenhuma elegível"}
                  {fora && ` · ${fora}`}
                </span>
              </button>
            );
          })}
          <button type="button" className="btn btn-link" onClick={() => setSelecionadas(new Set())}>
            Limpar
          </button>
        </div>
      )}

      {gaveta}
    </div>
  );
}
