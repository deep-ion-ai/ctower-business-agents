import { useEffect, useRef } from "react";
import type { LinkForja, Trecho } from "./analise";
import { Alerta } from "./componentes";

export type CargaTrecho =
  | { tipo: "carregando"; chave: string }
  | { tipo: "pronto"; chave: string; trecho: Trecho }
  | { tipo: "erro"; chave: string; mensagem: string };

const FORJA: Record<LinkForja["forja"], string> = { github: "GitHub", gitlab: "GitLab" };

interface Props {
  carga: CargaTrecho;
  aoFechar: () => void;
  /** Posição entre as evidências da mesma afirmação (`indice` a partir de 0). */
  posicao?: { indice: number; total: number };
  aoAnterior?: () => void;
  aoProxima?: () => void;
  /** Link na forja do repositório; sem ele (nenhum repositório cadastrado), sem o botão. */
  link?: LinkForja | null;
  aoAbrirLink?: (url: string) => void;
}

/**
 * Conteúdo da aba Código (#185): o caminho do arquivo, o trecho com as linhas citadas
 * em destaque, a evidência anterior e a próxima da mesma afirmação e o link para a
 * forja.
 */
export function TrechoEvidencia({
  carga,
  aoFechar,
  posicao,
  aoAnterior,
  aoProxima,
  link,
  aoAbrirLink,
}: Props) {
  const destaque = useRef<HTMLTableRowElement>(null);

  useEffect(() => {
    // O jsdom não implementa a rolagem.
    destaque.current?.scrollIntoView?.({ block: "center" });
  }, [carga]);

  const [arquivo, linhas] = separar(carga.chave);
  const varias = posicao !== undefined && posicao.total > 1;
  return (
    <section className="trecho-evidencia" aria-label="Trecho da evidência">
      <header>
        <h3 className="trecho-caminho">
          <code>{arquivo}</code>
          {linhas && <span className="muted"> · linhas {linhas}</span>}
        </h3>
        <button className="btn btn-sm" type="button" onClick={aoFechar}>
          Fechar
        </button>
      </header>
      {(varias || link) && (
        <div className="trecho-barra">
          {varias && (
            <div className="trecho-navegacao" role="group" aria-label="Evidências desta afirmação">
              <button
                className="btn btn-sm"
                type="button"
                disabled={posicao.indice === 0}
                onClick={aoAnterior}
              >
                ← Anterior
              </button>
              <span className="muted" aria-live="polite">
                {posicao.indice + 1} de {posicao.total}
              </span>
              <button
                className="btn btn-sm"
                type="button"
                disabled={posicao.indice >= posicao.total - 1}
                onClick={aoProxima}
              >
                Próxima →
              </button>
            </div>
          )}
          {link && (
            <button
              className="btn btn-sm"
              type="button"
              onClick={() => aoAbrirLink?.(link.url)}
              title={link.url}
            >
              Abrir no {FORJA[link.forja]} ↗
            </button>
          )}
        </div>
      )}
      {carga.tipo === "carregando" && <p className="muted">Abrindo o trecho…</p>}
      {carga.tipo === "erro" && (
        <Alerta tom="erro" titulo={carga.mensagem} />
      )}
      {carga.tipo === "pronto" && (
        <div className="trecho-codigo">
          <table>
            <tbody>
              {carga.trecho.linhas.map((texto, i) => {
                const numero = carga.trecho.primeira + i;
                const citada = numero >= carga.trecho.inicio && numero <= carga.trecho.fim;
                return (
                  <tr
                    key={numero}
                    ref={numero === carga.trecho.inicio ? destaque : undefined}
                    className="linha"
                    data-citada={citada || undefined}
                    data-linha={numero}
                    aria-current={numero === carga.trecho.inicio ? "location" : undefined}
                  >
                    <td className="numero">{numero}</td>
                    <td className="codigo">
                      <code>{texto}</code>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

/** `arquivo:inicio-fim` → o arquivo e as linhas (o caminho pode ter `:`). */
function separar(chave: string): [string, string] {
  const i = chave.lastIndexOf(":");
  return i < 0 ? [chave, ""] : [chave.slice(0, i), chave.slice(i + 1)];
}
