import { useCallback, useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, Selo } from "./componentes";
import { TrechosDaProposta, type PedidoDeRevisao } from "./DocumentacaoDoMapa";
import {
  apiDocumentacaoTauri,
  avisarMudancaNaDocumentacao,
  EVENTO_DOCUMENTACAO,
  filaDeRevisao,
  tituloDaRevisao,
  type ApiDocumentacao,
  type PropostaVista,
  type RascunhoNaFila,
  type TabelaDeDocumentacao,
} from "./documentacaoMapa";
import { contar } from "./formatacao";

/** Como o rascunho terminou nesta aba. */
type Desfecho = { tipo: "publicado"; versao: string } | { tipo: "descartado" };

/**
 * Revisão final de um documento vivo numa aba própria (#499): o texto do
 * rascunho com as marcas do que muda, os passos (ler → marcar como revisado →
 * publicar) e, ao lado, a fila dos outros rascunhos do projeto. "Publicar" só
 * habilita depois de marcar como revisado; "Publicar e abrir o próximo" segue a
 * fila sem voltar à tabela.
 */
export function RevisaoDocumentoVivo({
  projeto,
  documento,
  api = apiDocumentacaoTauri,
  aoAbrir,
}: {
  projeto: string;
  /** Caminho do documento na base, relativo ao projeto. */
  documento: string;
  api?: ApiDocumentacao;
  /** Abre a revisão de outro rascunho (outra aba). */
  aoAbrir?: (pedido: PedidoDeRevisao) => void;
}) {
  const [vista, setVista] = useState<PropostaVista | null>(null);
  const [tabela, setTabela] = useState<TabelaDeDocumentacao | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [desfecho, setDesfecho] = useState<Desfecho | null>(null);

  const carregarFila = useCallback(() => {
    Promise.resolve()
      .then(() => api.tabela(projeto))
      .then(setTabela)
      .catch(() => undefined);
  }, [api, projeto]);

  useEffect(() => {
    let vivo = true;
    Promise.resolve()
      .then(() => api.proposta(projeto, documento))
      .then((v) => vivo && setVista(v))
      .catch((e: unknown) => vivo && setErro(mensagemDe(e)));
    carregarFila();
    return () => {
      vivo = false;
    };
  }, [api, projeto, documento, carregarFila]);

  useEffect(() => {
    window.addEventListener(EVENTO_DOCUMENTACAO, carregarFila);
    return () => window.removeEventListener(EVENTO_DOCUMENTACAO, carregarFila);
  }, [carregarFila]);

  const fila: RascunhoNaFila[] = tabela ? filaDeRevisao(tabela) : [];
  const outros = fila.filter((r) => r.caminho !== documento);
  const linha = tabela?.linhas.find((l) => l.funcionalidade === vista?.funcionalidade);
  const proximo = outros[0];

  const abrir = (r: RascunhoNaFila) =>
    aoAbrir?.({
      caminho: r.caminho,
      funcionalidade: r.funcionalidade,
      titulo: tituloDaRevisao(r.funcionalidade, { tipo: r.tipo, nome: r.nomeDoTipo }),
    });

  const executar = <T,>(p: () => Promise<T>): Promise<T | undefined> => {
    setOcupado(true);
    setFalha(null);
    return p()
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        return undefined;
      })
      .finally(() => setOcupado(false));
  };

  const marcar = () =>
    void executar(() => api.marcarRevisada(projeto, documento)).then((p) => {
      if (!p || !vista) return;
      setVista({ ...vista, proposta: p });
      avisarMudancaNaDocumentacao();
    });

  const publicar = (eAbrirOProximo: boolean) =>
    void executar(() => api.publicar(projeto, documento).then(() => true)).then((ok) => {
      if (!ok || !vista) return;
      setDesfecho({ tipo: "publicado", versao: vista.proposta.versao });
      avisarMudancaNaDocumentacao();
      if (eAbrirOProximo && proximo) abrir(proximo);
    });

  const descartar = () =>
    void executar(() => api.descartar(projeto, documento).then(() => true)).then((ok) => {
      if (!ok) return;
      setDesfecho({ tipo: "descartado" });
      avisarMudancaNaDocumentacao();
    });

  if (erro)
    return (
      <div className="page">
        <Alerta tom="erro" titulo="Não foi possível abrir o rascunho.">
          {erro}
        </Alerta>
      </div>
    );
  if (!vista) return <p className="muted page">Carregando…</p>;

  const p = vista.proposta;
  const revisada = !!p.revisada;
  const publicado = desfecho?.tipo === "publicado";
  const passos = [
    { nome: "Ler o rascunho", feito: true },
    { nome: "Marcar como revisado", feito: revisada },
    { nome: `Publicar v${p.versao}`, feito: publicado },
  ];
  const porTipo = new Map<string, RascunhoNaFila[]>();
  for (const r of fila) porTipo.set(r.nomeDoTipo, [...(porTipo.get(r.nomeDoTipo) ?? []), r]);

  return (
    <div className="revisao-documento page">
      <aside className="fila-revisao" aria-label="A revisar">
        <h2>
          A revisar <span className="contagem">{fila.length}</span>
        </h2>
        {[...porTipo].map(([tipo, itens]) => (
          <section key={tipo} aria-label={tipo}>
            <h3 className="muted">{tipo}</h3>
            <ul>
              {itens.map((r) => (
                <li key={r.caminho} aria-current={r.caminho === documento ? "page" : undefined}>
                  <button
                    type="button"
                    className="btn btn-link item-fila"
                    disabled={r.caminho === documento || !aoAbrir}
                    onClick={() => abrir(r)}
                  >
                    <span>{r.nome}</span>
                    <span className="muted">
                      {r.funcionalidade} · {r.rotulo}
                      {r.revisada && " · revisado"}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        ))}
        {fila.length === 0 && <p className="muted">Nenhum rascunho à espera.</p>}
      </aside>

      <section className="revisao-principal" aria-label="Revisão final">
        <p className="muted">
          {vista.nomeDoTipo}
          {linha && ` · ${linha.areaNome}`}
        </p>
        <h1>{linha?.nome ?? vista.funcionalidade ?? vista.caminho}</h1>
        <p className="muted">
          <span className="painel-id">{vista.funcionalidade}</span> ·{" "}
          {vista.publicada ? `proposta v${vista.publicada} → v${p.versao}` : `rascunho v${p.versao}`} ·
          versão publicada: {vista.publicada ? `v${vista.publicada}` : "nenhuma"}
        </p>

        <ol className="passos-revisao" aria-label="Passos da revisão final">
          {passos.map((s, i) => (
            <li key={s.nome} data-feito={s.feito ? "sim" : "nao"}>
              <span className="marca-passo" aria-hidden="true">
                {s.feito ? "✓" : i + 1}
              </span>{" "}
              {s.nome}
            </li>
          ))}
        </ol>

        {desfecho ? (
          <div className="desfecho-revisao" role="status">
            <Selo tom={desfecho.tipo === "publicado" ? "concluido" : "neutro"}>
              {desfecho.tipo === "publicado" ? "publicado" : "descartado"}
            </Selo>{" "}
            <strong>
              {desfecho.tipo === "publicado"
                ? `${vista.nomeDoTipo} v${desfecho.versao} publicado.`
                : "Rascunho descartado."}
            </strong>{" "}
            {proximo
              ? `Há mais ${contar(outros.length, "rascunho", "rascunhos")} na fila.`
              : "Não há mais rascunhos na fila."}
            {proximo && aoAbrir && (
              <>
                {" "}
                <button type="button" className="btn btn-link" onClick={() => abrir(proximo)}>
                  Abrir o próximo
                </button>
              </>
            )}
          </div>
        ) : revisada ? (
          <p className="muted">
            Revisado por {p.revisada!.por}. Uma edição ou "Gerar de novo" volta o documento para
            rascunho.
          </p>
        ) : (
          <Alerta tom="atencao" titulo="Revisão final obrigatória" role="note">
            Confira o texto contra o código e as regras citadas antes de publicar.
          </Alerta>
        )}

        <p>
          <strong>O que muda:</strong> {p.mudancas}
        </p>
        <p className="legenda-trechos muted">
          <Selo tom="concluido">novo</Selo> trecho criado nesta versão · <code>RN-…</code> regra de
          negócio citada
        </p>
        <TrechosDaProposta trechos={p.trechos} publicada={vista.publicada} />

        {falha && <Alerta tom="erro" titulo={falha} />}

        {!desfecho && (
          <div className="acoes-revisao-documento">
            <button type="button" className="btn btn-link" disabled={ocupado} onClick={descartar}>
              Descartar rascunho
            </button>
            <span className="espaco" />
            {!revisada && (
              <span className="muted">Publicar só depois de marcar como revisado</span>
            )}
            {!revisada && (
              <button type="button" className="btn" disabled={ocupado} onClick={marcar}>
                Marcar como revisado
              </button>
            )}
            <button
              type="button"
              className="btn btn-primary"
              disabled={ocupado || !revisada}
              title={revisada ? undefined : "Marque como revisado antes de publicar"}
              onClick={() => publicar(false)}
            >
              Publicar v{p.versao}
            </button>
            {proximo && aoAbrir && (
              <button
                type="button"
                className="btn"
                disabled={ocupado || !revisada}
                onClick={() => publicar(true)}
              >
                Publicar e abrir o próximo
              </button>
            )}
          </div>
        )}
      </section>
    </div>
  );
}
