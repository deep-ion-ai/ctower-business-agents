import { useCallback, useEffect, useState } from "react";
import { abrirLinkDoProjeto } from "./abrirLinkProjeto";
import { mensagemDe } from "./analise";
import type { ApiConhecimento, Encontrado } from "./conhecimento";
import { situacaoEmTexto, type Acao } from "./acaoIssue";
import type { Tela } from "./Navegacao";
import type { IssueDoProjeto } from "./issuesProjeto";
import { BarraAcoes, TextoRico, type Acao as AcaoBotao } from "./componentes";

export const ROTULO_ORIGEM = { github: "GitHub", gitlab: "GitLab", jira: "Jira" } as const;

/** Um link que não abriu: o endereço (para copiar) e o motivo que o app deu. */
export interface FalhaLink {
  url: string;
  motivo: string;
}

/**
 * Abre links do projeto e guarda a falha para o aviso ao lado do painel (#398): a
 * falha aparece em português, com o endereço para copiar, e não some em silêncio.
 */
export function useAbrirLink(projeto: string) {
  const [falha, setFalha] = useState<FalhaLink | null>(null);
  const abrir = useCallback(
    (link: string) => {
      setFalha(null);
      return Promise.resolve()
        .then(() => abrirLinkDoProjeto(projeto, link))
        .catch((e: unknown) => setFalha({ url: link, motivo: mensagemDe(e) }));
    },
    [projeto],
  );
  return { abrir, falha, fechar: () => setFalha(null) };
}

export function AvisoAbrirLink({ falha }: { falha: FalhaLink | null }) {
  if (!falha) return null;
  return (
    <p className="error-text" role="alert">
      Não foi possível abrir o link no navegador. {falha.motivo}{" "}
      <button
        type="button"
        className="link"
        onClick={() => {
          void navigator.clipboard?.writeText(falha.url).catch(() => undefined);
        }}
      >
        Copiar a URL
      </button>
    </p>
  );
}

/**
 * A descrição da issue em Markdown (#398), pelo {@link TextoRico}: os links do host
 * da issue abrem no navegador do sistema; os de outro host aparecem como texto, com
 * o endereço no title. Imagens não carregam: viram o link delas, com o texto
 * alternativo.
 */
export function DescricaoIssue({ issue, abrir }: { issue: IssueDoProjeto; abrir: (link: string) => void }) {
  if (!issue.descricao.trim()) return <p className="muted">A issue não tem descrição.</p>;
  return (
    <TextoRico
      texto={issue.descricao}
      hostPermitido={issue.url}
      aoAbrirLink={abrir}
      className="demandas-descricao"
    />
  );
}

/**
 * A ação principal da demanda, no topo (#404): "Analisar impacto" (ou o rótulo que a
 * situação der) e, quando houver, "Abrir em nova aba". O motivo do bloqueio fica
 * junto do botão. No topo, ela não some numa descrição longa.
 */
export function AcoesDaIssue({
  acao,
  analisando = false,
  aoAgir,
  secundarias,
}: {
  acao: Acao;
  analisando?: boolean;
  aoAgir: () => void;
  /** Ações a mais na mesma fileira (ex.: "Abrir em nova aba"). */
  secundarias?: AcaoBotao[];
}) {
  return (
    <div className="demandas-acoes">
      <BarraAcoes
        rotulo="Ações da demanda"
        alinhamento="fim"
        principal={{
          texto: analisando ? "Analisando…" : acao.rotulo,
          aoEscolher: aoAgir,
          desabilitado: analisando || !!acao.bloqueio,
        }}
        secundarias={secundarias}
      />
      {acao.bloqueio && <p className="muted demandas-bloqueio">{acao.bloqueio}</p>}
    </div>
  );
}

/** O que a issue tem na base: situação, demanda, análise e funcionalidades. */
export function NaBaseIssue({
  issue,
  projeto,
  navegar,
  aoAbrirAnalise,
}: {
  issue: IssueDoProjeto;
  projeto: string;
  navegar: (t: Tela) => void;
  aoAbrirAnalise: (id: string) => void;
}) {
  return (
    <div className="demandas-nabase">
      {issue.situacao === "sem-analise" ? (
        <p>
          <b>Ainda sem análise.</b> A análise de impacto verifica no código o que o solicitante
          afirmou e lista o que muda em regras, dados e telas.
        </p>
      ) : (
        <p>
          <b>{situacaoEmTexto(issue)}.</b>
        </p>
      )}
      {issue.demanda && (
        <p className="muted">
          Demanda{" "}
          <button
            type="button"
            className="link"
            onClick={() => navegar({ tipo: "demanda", projeto, demanda: issue.demanda! })}
          >
            {issue.demanda}
          </button>
          {issue.analise && issue.situacao !== "sem-analise" && (
            <>
              {" · análise "}
              <button type="button" className="link" onClick={() => aoAbrirAnalise(issue.analise!)}>
                {issue.analise}
              </button>
            </>
          )}
        </p>
      )}
      <p className="muted">
        {issue.funcionalidades.length > 0
          ? `Funcionalidades: ${issue.funcionalidades.join(", ")}.`
          : "Ainda sem funcionalidade ligada."}{" "}
        <button type="button" className="link" onClick={() => navegar({ tipo: "vincular", projeto })}>
          Vincular a funcionalidades
        </button>
      </p>
    </div>
  );
}

/** Uma linha de "Parecidas": a issue (com a análise, se tiver) ou a regra da base. */
export interface ItemParecido {
  chave: string;
  id: string;
  titulo: string;
  analise?: string | null;
}

export function ParecidasLista({
  itens,
  regras,
  navegar,
}: {
  itens: ItemParecido[];
  regras: Encontrado[];
  navegar: (t: Tela) => void;
}) {
  return (
    <>
      <h4>Parecidas</h4>
      {itens.length === 0 && regras.length === 0 ? (
        <p className="muted">Nada parecido na lista nem nas regras da base.</p>
      ) : (
        <ul className="linhas demandas-parecidas" aria-label="Itens parecidos">
          {itens.map((s) => (
            <li key={s.chave}>
              <span className="id">{s.id}</span>
              <span className="tit">{s.titulo}</span>
              {s.analise && <span className="badge">{s.analise}</span>}
            </li>
          ))}
          {regras.map((r) => (
            <li key={r.caminho}>
              <span className="id">{r.identificador ?? "regra"}</span>
              <button type="button" className="link" onClick={() => navegar({ tipo: "base", documento: r.caminho })}>
                {r.titulo}
              </button>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

/** Regras da base com palavras do título, para o analista ver se já existe. */
export function useRegrasParecidas(api: ApiConhecimento, projeto: string, titulo: string): Encontrado[] {
  const [regras, setRegras] = useState<Encontrado[]>([]);
  useEffect(() => {
    let ativo = true;
    setRegras([]);
    if (!api.buscar || !titulo.trim()) return;
    api
      .buscar(titulo, { projeto, tipos: ["regra"] })
      .then((b) => ativo && setRegras(b.resultados.slice(0, 3)))
      // Sem a busca, o painel só não lista as regras.
      .catch(() => undefined);
    return () => {
      ativo = false;
    };
  }, [api, projeto, titulo]);
  return regras;
}
