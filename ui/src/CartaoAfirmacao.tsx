import type { ReactNode } from "react";
import type { Afirmacao, Confianca, Evidencia } from "./analise";
import { AcoesAfirmacao, type ComandoAfirmacao, type Revisao } from "./AcoesRevisao";
import { TextoAfirmacao } from "./TextoAfirmacao";

export const ROTULO_CONFIANCA: Record<Confianca, string> = {
  confirmado: "confirmado",
  inferido: "inferido",
  a_confirmar: "a confirmar",
};

/** Cor do selo de confiança: confirmado (verde), inferido (azul), a confirmar (âmbar). */
const TOM_CONFIANCA: Record<Confianca, "done" | "running" | "attention"> = {
  confirmado: "done",
  inferido: "running",
  a_confirmar: "attention",
};

/** Resultado da decisão do analista, no selo do cartão. */
const RESULTADO: Partial<Record<Afirmacao["estado"], { texto: string; tom: string }>> = {
  confirmada: { texto: "confirmada por você", tom: "done" },
  incorreta: { texto: "marcada como incorreta", tom: "error" },
  aprofundar: { texto: "aprofundamento pedido", tom: "running" },
  "a-revalidar": { texto: "a revalidar: o código mudou", tom: "attention" },
};

export function chaveDaEvidencia(e: Evidencia): string {
  return `${e.arquivo}:${e.linhas}`;
}

interface Props {
  afirmacao: Afirmacao;
  /** Ações de revisão; sem elas, só leitura. */
  revisao?: Revisao;
  /** É a afirmação em foco dos atalhos de teclado. */
  atual?: boolean;
  comando?: ComandoAfirmacao | null;
  /** Evidência aberta na aba Código (destacada entre os chips). */
  evidenciaAberta?: string | null;
  aoAbrirEvidencia: (e: Evidencia) => void;
  aoFocar?: () => void;
  /** Trecho aberto da evidência, quando mostrado dentro do cartão. */
  trecho?: ReactNode;
}

/**
 * Cartão de uma afirmação (#184): número, selo de confiança, texto, evidências como
 * chips, ações com rótulo e, depois da decisão, o selo do resultado com o comentário.
 */
export function CartaoAfirmacao({
  afirmacao: a,
  revisao,
  atual,
  comando,
  evidenciaAberta,
  aoAbrirEvidencia,
  aoFocar,
  trecho,
}: Props) {
  const resultado = RESULTADO[a.estado];
  const semDecisao = !!revisao && a.estado === "registrada";
  const comentario =
    a.estado === "incorreta" ? a.comentario : a.estado === "aprofundar" ? a.pedido : undefined;
  return (
    <li
      className="afirmacao cartao"
      data-estado={a.estado}
      data-pendente={semDecisao || undefined}
      data-atual={atual || undefined}
      aria-current={atual ? "true" : undefined}
      data-testid={`afirmacao-${a.numero}`}
      tabIndex={-1}
      onFocus={aoFocar}
    >
      <div className="cartao-cabeca">
        <span className="cartao-numero" title={`Afirmação ${a.numero}`}>
          {a.numero}
        </span>
        <span
          className="badge selo-confianca"
          data-tone={TOM_CONFIANCA[a.confianca]}
          data-confianca={a.confianca}
        >
          {ROTULO_CONFIANCA[a.confianca]}
        </span>
        {semDecisao && (
          <span className="badge" data-tone="attention">
            sem decisão
          </span>
        )}
        {resultado && (
          <span className="badge selo-resultado" data-tone={resultado.tom}>
            {resultado.texto}
          </span>
        )}
        {a.texto_original && (
          <span className="muted editada" title={`Texto do agente: ${a.texto_original}`}>
            (texto editado pelo analista)
          </span>
        )}
      </div>
      <TextoAfirmacao texto={a.texto} />
      {comentario && <p className="comentario muted">{comentario}</p>}
      {a.evidencias.length > 0 && (
        <ul className="evidencias" aria-label="Evidências">
          {a.evidencias.map((e) => {
            const chave = chaveDaEvidencia(e);
            return (
              <li key={chave}>
                <button
                  type="button"
                  className="btn btn-sm evidencia chip-evidencia"
                  aria-pressed={evidenciaAberta === chave}
                  title={e.trecho}
                  onClick={() => aoAbrirEvidencia(e)}
                >
                  {chave}
                </button>
                {e.estado === "a-revalidar" && <span className="muted"> a revalidar</span>}
              </li>
            );
          })}
        </ul>
      )}
      {revisao && <AcoesAfirmacao afirmacao={a} revisao={revisao} comando={comando} />}
      {trecho}
    </li>
  );
}
