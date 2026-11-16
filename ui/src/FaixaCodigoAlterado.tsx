import { useMemo, useState } from "react";
import type { Afirmacao, Evidencia } from "./analise";
import type { OrigemEvidencia } from "./DocumentoAoVivo";

/** Arquivo de código cuja mudança afeta afirmações (evidências marcadas a revalidar). */
export interface CodigoAlterado {
  arquivo: string;
  afirmacoes: number[];
  /** Primeira evidência do arquivo, para "Ver a mudança". */
  evidencia: Evidencia;
  origem: OrigemEvidencia;
}

/** Agrupa por arquivo as evidências a revalidar e as afirmações que as usam. */
export function codigoAlterado(afirmacoes: Afirmacao[]): CodigoAlterado[] {
  const por = new Map<string, CodigoAlterado>();
  for (const a of afirmacoes) {
    for (const e of a.evidencias) {
      if (e.estado !== "a-revalidar") continue;
      const atual = por.get(e.arquivo);
      if (!atual) {
        por.set(e.arquivo, {
          arquivo: e.arquivo,
          afirmacoes: [a.numero],
          evidencia: e,
          origem: `afirmacao-${a.numero}`,
        });
      } else if (!atual.afirmacoes.includes(a.numero)) atual.afirmacoes.push(a.numero);
    }
  }
  return [...por.values()];
}

/**
 * Faixa no topo do documento (#186): o código citado como evidência mudou. Mostra o
 * arquivo, as afirmações que o usam, "Ver a mudança" (abre o trecho), "Pedir
 * revalidação ao agente" e dispensar. Dispensar vale até o conjunto de arquivos mudar.
 */
export function FaixaCodigoAlterado({
  afirmacoes,
  aoVer,
  aoPedir,
  aoIrPara,
}: {
  afirmacoes: Afirmacao[];
  aoVer: (e: Evidencia, origem: OrigemEvidencia) => void;
  /** Envia o pedido ao agente; devolve `true` se foi enviado. */
  aoPedir: (alterados: CodigoAlterado[]) => boolean;
  aoIrPara?: (numero: number) => void;
}) {
  const alterados = useMemo(() => codigoAlterado(afirmacoes), [afirmacoes]);
  const chave = alterados.map((c) => `${c.arquivo}:${c.afirmacoes.join(",")}`).join("|");
  const [dispensada, setDispensada] = useState<string | null>(null);
  const [pedida, setPedida] = useState<string | null>(null);

  if (alterados.length === 0 || dispensada === chave) return null;

  return (
    <section className="faixa-codigo" role="region" aria-label="Código alterado" data-tone="attention">
      <div className="faixa-codigo-texto">
        <strong>O código citado na análise mudou.</strong>
        <ul>
          {alterados.map((c) => (
            <li key={c.arquivo}>
              <code>{c.arquivo}</code>
              {" · usado por "}
              {c.afirmacoes.map((n, i) => (
                <span key={n}>
                  {i > 0 && ", "}
                  {aoIrPara ? (
                    <button
                      className="btn btn-link"
                      type="button"
                      aria-label={`Ir para a afirmação ${n}`}
                      onClick={() => aoIrPara(n)}
                    >
                      afirmação {n}
                    </button>
                  ) : (
                    `afirmação ${n}`
                  )}
                </span>
              ))}{" "}
              <button
                className="btn"
                type="button"
                aria-label={`Ver a mudança em ${c.arquivo}`}
                onClick={() => aoVer(c.evidencia, c.origem)}
              >
                Ver a mudança
              </button>
            </li>
          ))}
        </ul>
        {pedida === chave && (
          <p className="muted" role="status">
            Pedido de revalidação enviado ao agente.
          </p>
        )}
      </div>
      <div className="faixa-codigo-acoes">
        <button
          className="btn"
          type="button"
          onClick={() => aoPedir(alterados) && setPedida(chave)}
        >
          Pedir revalidação ao agente
        </button>
        <button className="btn" type="button" onClick={() => setDispensada(chave)}>
          Dispensar
        </button>
      </div>
    </section>
  );
}
