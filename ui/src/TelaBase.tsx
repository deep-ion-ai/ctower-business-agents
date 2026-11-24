import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { mensagemDe } from "./analise";
import {
  agruparPorArea,
  agruparPorTipo,
  carregarTudo,
  nomeDaArea,
  ROTULO_ABA,
  semAcento,
  SITUACOES,
  tituloLimpo,
  type AbaBase,
  type Grupo,
  type ResultadoBase,
} from "./baseLista";
import { ConferenciaEvidencias } from "./ConferenciaEvidencias";
import {
  partesDoTrecho,
  rotuloOrigem,
  rotuloTipo,
  ROTULO_STATUS_REGRA,
  type ApiConhecimento,
  type DocumentoAberto,
  type Encontrado,
  type StatusRegra,
  type VisaoProjeto,
} from "./conhecimento";
import { DocumentoBase } from "./DocumentoBase";
import { EditorDocumento } from "./EditorDocumento";
import { ErroTecnico } from "./erroTecnico";
import { contar } from "./formatacao";
import type { Tela } from "./Navegacao";
import type { ProjetoResumo } from "./terminal";

const ABAS: AbaBase[] = ["regras", "glossario", "documentos"];

/** Situação de uma regra, com o tom que o âmbar reforça só em "a revalidar". */
function SeloSituacao({ status }: { status: string | null }) {
  if (!status) return null;
  const rotulo = ROTULO_STATUS_REGRA[status as StatusRegra] ?? status;
  return (
    <span
      className="badge"
      data-tone={
        status === "a-revalidar" ? "attention" : status === "confirmada" ? "done" : undefined
      }
    >
      {rotulo}
    </span>
  );
}

/**
 * Base de conhecimento (#328, §9.4 da revisão de UX): um lugar só para as regras, o
 * glossário e os documentos, em abas com contagem e uma busca para as três. Regras e
 * glossário agrupam por área (com o nome legível) e filtram por área e situação,
 * com "A revalidar" sempre à vista. O documento abre ao lado da lista, com o texto
 * primeiro; "Abrir em aba" dá a página inteira. A conferência das evidências fica no
 * cabeçalho. Quando a base muda fora do app, a lista e o documento são relidos.
 *
 * Com `documentoInicial` (a aba de um documento), mostra só o documento, na largura
 * toda, com os links entre documentos navegáveis.
 */
export function TelaBase({
  api,
  projetos,
  documentoInicial,
  projetoInicial = "",
  navegar,
  aoAbrirDocumento,
  aoPendencia,
}: {
  api: ApiConhecimento;
  projetos: ProjetoResumo[];
  /** O projeto escolhido na barra de título (#319). */
  projetoInicial?: string;
  documentoInicial?: string;
  navegar: (t: Tela) => void;
  /** Avisa o shell do documento aberto, para listá-lo no painel lateral (#85). */
  aoAbrirDocumento?: (caminho: string) => void;
  /** Avisa o shell de haver (ou não) alterações sem salvar no editor do documento (#220). */
  aoPendencia?: (pendente: boolean) => void;
}) {
  const soDocumento = documentoInicial !== undefined;
  const projeto = projetoInicial || (soDocumento ? "" : (projetos[0]?.id ?? ""));

  const [texto, setTexto] = useState("");
  /** A busca em vigor (só muda ao enviar); vazia lista tudo. */
  const [consulta, setConsulta] = useState("");
  const [aba, setAba] = useState<AbaBase>("regras");
  const [area, setArea] = useState("");
  const [situacao, setSituacao] = useState<StatusRegra | "">("");
  const [resultado, setResultado] = useState<ResultadoBase | null>(null);
  const [visao, setVisao] = useState<VisaoProjeto | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [historico, setHistorico] = useState<string[]>(documentoInicial ? [documentoInicial] : []);
  const [documento, setDocumento] = useState<DocumentoAberto | null>(null);
  const [falhaDocumento, setFalhaDocumento] = useState<string | null>(null);
  const [rodada, setRodada] = useState(0);
  const [editando, setEditando] = useState(false);
  const aberto = historico[historico.length - 1];
  const topo = useRef<HTMLDivElement>(null);

  // A lista toda (as três abas de uma vez) e a visão do projeto, com os nomes das áreas.
  useEffect(() => {
    if (soDocumento) return;
    let ativo = true;
    carregarTudo(api, consulta, projeto)
      .then((r) => {
        if (!ativo) return;
        setResultado(r);
        setFalha(null);
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    if (projeto) {
      api
        .visaoProjeto(projeto)
        .then((v) => ativo && setVisao(v))
        .catch(() => undefined);
    }
    return () => {
      ativo = false;
    };
  }, [api, consulta, projeto, rodada, soDocumento]);

  // Outro documento (ou nenhum): sai do modo de edição.
  useEffect(() => setEditando(false), [aberto]);

  // Documento aberto.
  useEffect(() => {
    if (!aberto) {
      setDocumento(null);
      return;
    }
    let ativo = true;
    api
      .abrirDocumento(aberto)
      .then((d) => {
        if (!ativo) return;
        setDocumento(d);
        setFalhaDocumento(null);
        aoAbrirDocumento?.(aberto);
        topo.current?.scrollIntoView?.({ block: "start" });
      })
      .catch((e: unknown) => ativo && setFalhaDocumento(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, aberto, rodada]);

  // Edições fora do app: relê.
  useEffect(() => {
    let parar: (() => void) | null = null;
    let ativo = true;
    api
      .vigiarBase(() => setRodada((r) => r + 1))
      .then((p) => (ativo ? (parar = p) : p()))
      .catch(() => undefined);
    return () => {
      ativo = false;
      parar?.();
    };
  }, [api]);

  const areas = visao?.areas ?? [];
  const nome = useCallback((id: string) => nomeDaArea(id, areas), [areas]);

  const buscar = (ev: FormEvent) => {
    ev.preventDefault();
    setConsulta(texto.trim());
  };

  const abrir = useCallback((caminho: string) => setHistorico((h) => [...h, caminho]), []);
  const voltar = () => setHistorico((h) => h.slice(0, -1));
  const recarregar = useCallback(() => setRodada((r) => r + 1), []);
  /** Depois de gravar: a prévia mostra o texto novo e a lista refaz a busca no índice. */
  const aoSalvo = (d: DocumentoAberto) => {
    setDocumento(d);
    setRodada((r) => r + 1);
  };

  const abrirAnalise = (s: string, id: string) =>
    navegar({ tipo: "analise", projeto: s, aberta: { id, abertura: { tipo: "reabrir" } } });

  const painelDocumento = (
    <div className="documento-aberto" ref={topo}>
      <div className="acoes" hidden={editando}>
        {historico.length > 1 && (
          <button className="btn" type="button" onClick={voltar}>
            Voltar
          </button>
        )}
        {!soDocumento && (
          <button className="btn" type="button" onClick={() => setHistorico([])}>
            Fechar o documento
          </button>
        )}
      </div>
      {falhaDocumento && (
        <p className="error-text" role="alert">
          Não foi possível abrir {aberto}: {falhaDocumento}
        </p>
      )}
      {documento && !falhaDocumento && editando && (
        <EditorDocumento
          key={documento.caminho}
          documento={documento}
          salvar={(markdown, versao) => api.salvarDocumento(documento.caminho, markdown, versao)}
          reler={() => api.abrirDocumento(documento.caminho)}
          aoSalvo={aoSalvo}
          aoSair={() => {
            setEditando(false);
            // O que está no arquivo agora (o salvo, ou o de fora do app).
            setRodada((r) => r + 1);
          }}
          aoPendencia={aoPendencia}
        />
      )}
      {documento && !falhaDocumento && !editando && (
        <DocumentoBase
          aoEditar={documento.somenteLeitura ? undefined : () => setEditando(true)}
          aoAbrirEmAba={
            soDocumento ? undefined : () => navegar({ tipo: "base", documento: documento.caminho })
          }
          documento={documento}
          aoAbrirDocumento={abrir}
          aoAbrirAnalise={abrirAnalise}
          nomeDaArea={nome}
        />
      )}
    </div>
  );

  if (soDocumento) {
    return (
      <section className="tela-base tela-base-documento page" aria-label="Documento da base">
        {painelDocumento}
      </section>
    );
  }

  // Filtros: a área vale para regras e glossário; a situação, só para as regras.
  const noArea = (r: Encontrado) => !area || r.area === area;
  const regrasBase = (resultado?.regras ?? []).filter(noArea);
  const aRevalidar = regrasBase.filter((r) => r.status === "a-revalidar").length;
  const regras = regrasBase.filter((r) => !situacao || r.status === situacao);
  const termos = (resultado?.termos ?? []).filter(noArea);
  const documentos = resultado?.documentos ?? [];
  const contagens: Record<AbaBase, number> = {
    regras: regras.length,
    glossario: termos.length,
    documentos: documentos.length,
  };
  const idsDeArea = [
    ...new Set(
      [...(resultado?.regras ?? []), ...(resultado?.termos ?? [])]
        .map((r) => r.area)
        .filter((a): a is string => !!a),
    ),
  ]
    .map((id) => ({ id, nome: nome(id) }))
    .sort((a, b) => semAcento(a.nome).localeCompare(semAcento(b.nome)));

  const gruposDaAba: Grupo[] =
    aba === "regras"
      ? agruparPorArea(regras, areas, consulta !== "")
      : aba === "glossario"
        ? agruparPorArea(termos, areas, consulta !== "")
        : agruparPorTipo(documentos, rotuloTipo);
  const naAba = contagens[aba];
  const totalGeral =
    (resultado?.regras.length ?? 0) + (resultado?.termos.length ?? 0) + documentos.length;
  const filtrando = !!area || !!situacao;

  return (
    <section className="tela-base page" aria-label="Base de conhecimento">
      <div className="cab-secao">
        <h2>Base de conhecimento</h2>
      </div>
      {projeto && <ConferenciaEvidencias api={api} projeto={projeto} aoMudar={recarregar} />}

      {visao && visao.problemas.length > 0 && (
        <section className="aviso-documentos" aria-label="Documentos com problema" role="alert">
          <h3>
            {contar(visao.problemas.length, "documento com problema", "documentos com problema")}
          </h3>
          <ul>
            {visao.problemas.map((p) => (
              <li key={p.caminho}>
                <span className="mono" title={p.caminho}>
                  {p.caminho.split("/").pop()}
                </span>
                : <ErroTecnico mensagem={p.erro} />{" "}
                <button
                  type="button"
                  className="btn"
                  onClick={() => navegar({ tipo: "base", documento: p.caminho })}
                >
                  Abrir para corrigir
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      <form className="busca-base" role="search" onSubmit={buscar} aria-label="Busca na base">
        <input
          type="search"
          aria-label="Buscar"
          placeholder="Ex.: multa atraso, carência, RN-COB-001"
          value={texto}
          onChange={(e) => setTexto(e.target.value)}
        />
        <button type="submit" className="btn">
          Buscar
        </button>
      </form>

      <div className="barra-filtros">
        <div role="tablist" aria-label="O que mostrar" className="abas-base">
          {ABAS.map((a) => (
            <button
              key={a}
              id={`aba-base-${a}`}
              type="button"
              role="tab"
              className="btn"
              aria-selected={aba === a}
              aria-controls="painel-base"
              onClick={() => setAba(a)}
            >
              {ROTULO_ABA[a]} <span className="contagem">{contagens[a]}</span>
            </button>
          ))}
        </div>
        <button
          type="button"
          className="btn"
          aria-pressed={situacao === "a-revalidar"}
          title="Regras cujo código citado mudou desde a aprovação"
          onClick={() => {
            setSituacao(situacao === "a-revalidar" ? "" : "a-revalidar");
            setAba("regras");
          }}
        >
          A revalidar <span className="contagem">{aRevalidar}</span>
        </button>
        <label>
          <span className="sr-only">Área</span>
          <select value={area} onChange={(e) => setArea(e.target.value)} aria-label="Área">
            <option value="">Todas as áreas</option>
            {idsDeArea.map((a) => (
              <option key={a.id} value={a.id}>
                {a.nome}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span className="sr-only">Situação</span>
          <select
            value={situacao}
            onChange={(e) => setSituacao(e.target.value as StatusRegra | "")}
            aria-label="Situação"
            title="A situação vale para as regras"
          >
            <option value="">Todas as situações</option>
            {SITUACOES.map((s) => (
              <option key={s.valor} value={s.valor}>
                {s.rotulo}
              </option>
            ))}
          </select>
        </label>
        {filtrando && (
          <button
            type="button"
            className="link"
            onClick={() => {
              setArea("");
              setSituacao("");
            }}
          >
            Limpar filtros
          </button>
        )}
      </div>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}

      <div className="base-colunas" data-editando={editando || undefined}>
        <section aria-label="Resultados" className="resultados-base">
          <div role="tabpanel" id="painel-base" aria-labelledby={`aba-base-${aba}`}>
            <p className="muted" role="status">
              {!resultado
                ? "Carregando…"
                : consulta === ""
                  ? totalGeral === 0
                    ? "A base ainda não tem documentos."
                    : `${contar(naAba, "item", "itens")} em ${ROTULO_ABA[aba]}.`
                  : naAba === 0
                    ? `Nada encontrado em ${ROTULO_ABA[aba]} para "${consulta}".`
                    : `${contar(naAba, "resultado", "resultados")} em ${ROTULO_ABA[aba]} para "${consulta}"${
                        resultado.qualquerPalavra
                          ? " (nenhum tinha todas as palavras; estes têm ao menos uma)"
                          : ""
                      }.`}
            </p>
            {gruposDaAba.map((g) => (
              <div key={g.chave || "sem-area"} className="grupo-base">
                <h3>
                  {g.titulo} <span className="muted">· {g.itens.length}</span>
                </h3>
                <ul>
                  {g.itens.map((r) => (
                    <li
                      key={r.caminho}
                      aria-current={r.caminho === aberto ? "true" : undefined}
                      data-status={r.status ?? undefined}
                    >
                      {r.tipo === "regra" && r.identificador && (
                        <span className="id mono">{r.identificador}</span>
                      )}
                      <button type="button" className="link" onClick={() => abrir(r.caminho)}>
                        {tituloLimpo(r.titulo)}
                      </button>
                      {r.tipo === "regra" && <SeloSituacao status={r.status} />}
                      {aba === "documentos" && r.identificador && (
                        <span className="muted mono">{r.identificador}</span>
                      )}
                      {aba === "documentos" && r.status && r.tipo !== "regra" && (
                        <span className="muted">{r.status}</span>
                      )}
                      {r.externo && (
                        <span className="badge" title={r.externo.caminho}>
                          {rotuloOrigem(r.externo)}
                        </span>
                      )}
                      {r.trecho && (
                        <p className="trecho-busca">
                          {partesDoTrecho(r.trecho).map((p, i) =>
                            p.destaque ? (
                              <mark key={i}>{p.texto}</mark>
                            ) : (
                              <span key={i}>{p.texto}</span>
                            ),
                          )}
                        </p>
                      )}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
            {resultado && naAba === 0 && filtrando && consulta === "" && totalGeral > 0 && (
              <p className="muted">Nenhum item com estes filtros.</p>
            )}
            {resultado && consulta !== "" && (
              <div className="vazio-base">
                <b>Não achou o que procurava?</b>
                <span>Pergunte ao agente; a resposta aprovada entra na base.</span>
                <button
                  type="button"
                  className="btn"
                  onClick={() =>
                    navegar({ tipo: "analise", projeto: projeto || undefined, pergunta: consulta })
                  }
                >
                  Perguntar: "{consulta}"
                </button>
              </div>
            )}
          </div>
        </section>

        {aberto && painelDocumento}
      </div>
    </section>
  );
}
