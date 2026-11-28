import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, EstadoVazio, Secao, Selo, TextoRico, type TomSelo } from "./componentes";
import {
  alertasDaCobertura,
  apiDocumentacaoTauri,
  faltamRegras,
  resumoDaCobertura,
  resumoDoLote,
  ROTULO_ESTADO,
  rotuloDoDocumento,
  type ApiDocumentacao,
  type ColunaDeDocumentacao,
  type DocumentoDoTipo,
  type EstadoDocumento,
  type LinhaDeDocumentacao,
  type PropostaVista,
  type TabelaDeDocumentacao,
} from "./documentacaoMapa";
import { contar } from "./formatacao";
import { TerminalClaude } from "./TerminalClaude";
import type { ApiTerminal } from "./terminal";
import type { TelaTerminal } from "./telaXterm";

/** Intervalo da releitura da tabela enquanto uma sessão do lote está aberta. */
export const INTERVALO_DO_LOTE_MS = 3000;

const TOM: Record<EstadoDocumento["estado"], TomSelo> = {
  em_dia: "concluido",
  proposta_pendente: "atencao",
  desatualizado: "erro",
  sem_documento: "neutro",
};

/** Detalhe da célula, embaixo do estado. */
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
}

interface PropsDaCelula {
  d: DocumentoDoTipo;
  linha: LinhaDeDocumentacao;
  coluna: ColunaDeDocumentacao;
  ocupado: boolean;
  aoAgir: (d: DocumentoDoTipo, linha: LinhaDeDocumentacao, coluna: ColunaDeDocumentacao) => void;
  /** Leva ao "Explicar" da funcionalidade, quando faltam regras; sem ele, só o aviso. */
  aoExplicar?: (funcionalidade: string) => void;
}

/** O que a célula mostra: o documento, o estado e a ação (ou o que falta para ela). */
function ConteudoDaCelula({ d, linha, coluna, ocupado, aoAgir, aoExplicar }: PropsDaCelula) {
  const rotulo = rotuloDoDocumento(d);
  const extra = detalhe(d);
  const semRegras = faltamRegras(d, linha, coluna);
  return (
    <>
      {rotulo && <span className="celula-documento-versao">{rotulo}</span>}
      <Selo tom={TOM[d.estado.estado]}>{ROTULO_ESTADO[d.estado.estado]}</Selo>
      {extra && <span className="muted celula-documento-detalhe">{extra}</span>}
      {semRegras ? (
        <>
          <span className="muted celula-documento-detalhe">
            precisa de regras: explique a funcionalidade e aprove a análise
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

function Celula(props: PropsDaCelula) {
  return (
    <td className="celula-documento">
      <ConteudoDaCelula {...props} />
    </td>
  );
}

/** A proposta aberta para a revisão final: o texto, marcar como revisada, publicar e descartar. */
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
      <div className="trechos-proposta" aria-label="Texto da proposta">
        {p.trechos
          .filter((t) => t.marca !== "removido" || vista.publicada)
          .map((t, i) => (
            <div key={i} className={`trecho trecho-${t.marca}`} data-marca={t.marca}>
              {t.marca !== "mantido" && vista.publicada && (
                <span className="trecho-marca">{t.marca}</span>
              )}
              {t.anterior && (
                <del className="trecho-anterior">
                  <TextoRico texto={t.anterior} />
                </del>
              )}
              <TextoRico texto={t.texto} />
            </div>
          ))}
      </div>
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

/**
 * Visão "Documentação" do mapa (#462): uma linha por funcionalidade e uma coluna
 * por tipo vivo habilitado no projeto, com o documento (e a versão), o estado e a
 * ação de cada célula; a cobertura por tipo no topo; e a geração da versão
 * inicial em lote. Os tipos montados pelo app saem na hora; os redigidos pelo
 * agente rodam uma sessão do Claude por funcionalidade, em sequência. Todo
 * documento gerado fica como rascunho até a revisão final.
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
}) {
  const [tabela, setTabela] = useState<TabelaDeDocumentacao | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [selecionadas, setSelecionadas] = useState<ReadonlySet<string>>(new Set());
  const [tipo, setTipo] = useState("");
  const [revisao, setRevisao] = useState<PropostaVista | null>(null);
  const [sessoes, setSessoes] = useState<Sessoes | null>(null);
  const ativo = useRef(true);
  const painelDoLote = useRef<HTMLDivElement>(null);
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

  const colunas = useMemo(() => tabela?.colunas ?? [], [tabela]);

  // O lote abre acima da tabela: leva a vista até ele, que pode estar fora da
  // tela quando a geração começou numa linha mais abaixo.
  const loteAberto = sessoes !== null;
  useEffect(() => {
    if (loteAberto) painelDoLote.current?.scrollIntoView?.({ behavior: "smooth", block: "start" });
  }, [loteAberto]);
  // A revisão da proposta também abre acima da tabela.
  const propostaAberta = revisao?.caminho ?? null;
  useEffect(() => {
    if (propostaAberta)
      painelDaProposta.current?.scrollIntoView?.({ behavior: "smooth", block: "start" });
  }, [propostaAberta]);
  const tipoEscolhido = colunas.find((c) => c.tipo === tipo) ?? colunas[0] ?? null;

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
      setSessoes({
        ...sessoes,
        atual: sessoes.atual + 1,
        feitas: [...sessoes.feitas, atual.funcionalidade],
      });
    }
  }, [tabela, sessoes]);

  const alternar = (id: string) =>
    setSelecionadas((atual) => {
      const novo = new Set(atual);
      if (novo.has(id)) novo.delete(id);
      else novo.add(id);
      return novo;
    });

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
      if (fila.length > 0) {
        setSessoes({ tipo: coluna, fila, atual: 0, feitas: [], puladas: [] });
        const puladas = lote.itens.filter((i) => i.resultado === "pulada");
        if (puladas.length > 0) setAviso(resumoDoLote({ ...lote, itens: puladas }));
      } else {
        setAviso(resumoDoLote(lote));
      }
      setSelecionadas(new Set());
    });

  const abrirProposta = (documento: string) =>
    executar(() => api.proposta(projeto, documento)).then((v) => v && setRevisao(v));

  const agir = (d: DocumentoDoTipo, linha: LinhaDeDocumentacao, coluna: ColunaDeDocumentacao) => {
    if (d.acao === "Gerar versão inicial") void gerar(coluna, [linha.funcionalidade]);
    else if (d.acao === "Revisar proposta" && d.caminho) void abrirProposta(d.caminho);
    else if (d.acao === "Propor atualização") {
      if (coluna.geracao === "app") {
        void executar(() =>
          api.proporAtualizacao(projeto, coluna.tipo, linha.funcionalidade),
        ).then((caminho) => {
          if (caminho) void abrirProposta(caminho);
        });
      } else {
        setAviso(
          `${coluna.nome} é redigido pelo agente: a atualização vem da demanda que mudou a funcionalidade. Abra a demanda no painel de ${linha.funcionalidade}.`,
        );
        aoAbrirFuncionalidade?.(linha.funcionalidade);
      }
    }
  };

  const revisar = (acao: "marcar" | "publicar" | "descartar") => {
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
      });
    else
      void executar(() => api.descartar(projeto, doc)).then(() => {
        setAviso("Proposta descartada.");
        setRevisao(null);
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

  const atual = sessoes && sessoes.atual < sessoes.fila.length ? sessoes.fila[sessoes.atual] : null;
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
  const painelDasSessoes = sessoes && (
    <div ref={painelDoLote}>
      <Secao
        titulo={`Versão inicial: ${sessoes.tipo.nome}`}
        nivel={3}
        rotulo="Sessões do lote"
        className="sessoes-lote"
        acoes={
          <button type="button" className="btn" onClick={() => setSessoes(null)}>
            {atual ? "Encerrar o lote" : "Fechar"}
          </button>
        }
      >
        <p role="status" aria-label="Progresso do lote">
          {atual
            ? `Sessão ${sessoes.atual + 1} de ${sessoes.fila.length}: ${atual.funcionalidade}`
            : `Lote concluído: ${contar(sessoes.feitas.length, "rascunho gerado", "rascunhos gerados")}`}
          {sessoes.puladas.length > 0 && ` · ${contar(sessoes.puladas.length, "pulada", "puladas")}`}
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
        {atual && (
          <>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() =>
                setSessoes({
                  ...sessoes,
                  atual: sessoes.atual + 1,
                  puladas: [...sessoes.puladas, atual.funcionalidade],
                })
              }
            >
              Pular {atual.funcionalidade}
            </button>
            <div className="terminal-lote">
              <TerminalClaude
                key={atual.funcionalidade}
                projeto={projeto}
                pedido={{ promptInicial: atual.pedido }}
                api={apiTerminal}
                criarTela={criarTela}
              />
            </div>
          </>
        )}
      </Secao>
    </div>
  );
  const painelDaRevisao = revisao && (
    <div ref={painelDaProposta}>
      <RevisaoDaProposta
        vista={revisao}
        ocupado={ocupado}
        aoMarcar={() => revisar("marcar")}
        aoPublicar={() => revisar("publicar")}
        aoDescartar={() => revisar("descartar")}
        aoFechar={() => setRevisao(null)}
      />
    </div>
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
        {painelDasSessoes}
        {painelDaRevisao}
      </div>
    );
  }

  const todas = tabela.linhas.map((l) => l.funcionalidade);
  const todasMarcadas = todas.every((f) => selecionadas.has(f));

  return (
    <div className="documentacao-mapa">
      <section aria-label="Cobertura por tipo" className="cobertura-por-tipo">
        {tabela.cobertura.map((c) => {
          const alertas = alertasDaCobertura(c);
          const pct = c.funcionalidades > 0 ? Math.round((100 * c.comDocumento) / c.funcionalidades) : 0;
          return (
            <div key={c.tipo} className="cartao-cobertura-tipo" aria-label={`Cobertura de ${c.nome}`}>
              <span className="muted">{c.nome}</span>
              <strong>{resumoDaCobertura(c)}</strong>
              <div
                className="barra-progresso"
                role="progressbar"
                aria-label={`${c.nome} com documento`}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={pct}
              >
                <div style={{ width: `${pct}%` }} />
              </div>
              {alertas && <span className="cobertura-tipo-alerta">{alertas}</span>}
            </div>
          );
        })}
      </section>

      <div className="barra-lote" role="group" aria-label="Geração em lote">
        <span>
          <strong>{selecionadas.size}</strong>{" "}
          {selecionadas.size === 1 ? "funcionalidade selecionada" : "funcionalidades selecionadas"}
        </span>
        <label>
          Gerar{" "}
          <select
            value={tipoEscolhido?.tipo ?? ""}
            onChange={(e) => setTipo(e.target.value)}
            aria-label="Tipo a gerar"
          >
            {colunas.map((c) => (
              <option key={c.tipo} value={c.tipo}>
                {c.nome}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          className="btn btn-primary"
          disabled={ocupado || emSessao || selecionadas.size === 0 || !tipoEscolhido}
          onClick={() => tipoEscolhido && void gerar(tipoEscolhido, [...selecionadas])}
        >
          Gerar a versão inicial
        </button>
        {tipoEscolhido && (
          <span className="muted">
            {tipoEscolhido.geracao === "app"
              ? "Montado pelo app na hora, sem IA."
              : "Redigido pelo agente: uma sessão por funcionalidade."}
          </span>
        )}
      </div>

      {avisos}
      {painelDasSessoes}
      {painelDaRevisao}

      <div className="tabela-documentacao-rolagem">
        <table className="tabela-documentacao" aria-label="Documentação por funcionalidade">
          <thead>
            <tr>
              <th scope="col">
                <input
                  type="checkbox"
                  aria-label="Selecionar todas"
                  checked={todasMarcadas}
                  onChange={() => setSelecionadas(todasMarcadas ? new Set() : new Set(todas))}
                />
              </th>
              <th scope="col">Funcionalidade</th>
              {colunas.map((c) => (
                <th key={c.tipo} scope="col">
                  {c.nome}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {tabela.linhas.map((l) => (
              <tr key={l.funcionalidade} aria-label={`${l.funcionalidade}: ${l.nome}`}>
                <td>
                  <input
                    type="checkbox"
                    aria-label={`Selecionar ${l.funcionalidade}`}
                    checked={selecionadas.has(l.funcionalidade)}
                    onChange={() => alternar(l.funcionalidade)}
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
                  )}
                  <span className="muted">
                    {" "}
                    {l.areaNome} · <span className="painel-id">{l.funcionalidade}</span>
                  </span>
                </th>
                {colunas.map((c, i) => (
                  <Celula
                    key={c.tipo}
                    d={l.documentos[i]}
                    linha={l}
                    coluna={c}
                    ocupado={ocupado || emSessao}
                    aoAgir={agir}
                    aoExplicar={aoExplicar}
                  />
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <section aria-label="Legenda" className="legenda-documentacao muted">
        <span>
          <Selo tom="concluido">em dia</Selo> a versão publicada confere com o código
        </span>
        <span>
          <Selo tom="atencao">proposta pendente</Selo> há um rascunho à espera da revisão final
        </span>
        <span>
          <Selo tom="erro">desatualizado</Selo> o código citado mudou desde a versão publicada
        </span>
        <span>
          <Selo tom="neutro">sem documento</Selo> ainda não gerado
        </span>
      </section>
    </div>
  );
}
