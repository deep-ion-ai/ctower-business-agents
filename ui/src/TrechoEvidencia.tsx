import { useEffect, useRef } from "react";
import type { Trecho } from "./analise";

export type CargaTrecho =
  | { tipo: "carregando"; chave: string }
  | { tipo: "pronto"; chave: string; trecho: Trecho }
  | { tipo: "erro"; chave: string; mensagem: string };

/** Trecho do código de uma evidência, com as linhas citadas destacadas. */
export function TrechoEvidencia({ carga, aoFechar }: { carga: CargaTrecho; aoFechar: () => void }) {
  const destaque = useRef<HTMLTableRowElement>(null);

  useEffect(() => {
    // O jsdom não implementa a rolagem.
    destaque.current?.scrollIntoView?.({ block: "center" });
  }, [carga]);

  return (
    <section className="trecho-evidencia" aria-label="Trecho da evidência">
      <header>
        <h3>{carga.chave}</h3>
        <button className="btn" type="button" onClick={aoFechar}>
          Fechar
        </button>
      </header>
      {carga.tipo === "carregando" && <p className="muted">Abrindo o trecho…</p>}
      {carga.tipo === "erro" && (
        <p className="error-text" role="alert">
          {carga.mensagem}
        </p>
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
