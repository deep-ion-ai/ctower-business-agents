import { Icon } from "./shell/icons";

export interface ItemSumario {
  /** Id do elemento da seção no documento. */
  id: string;
  titulo: string;
  /** Quantas pendências a seção tem. */
  pendencias: number;
  /** Seção sem nada a decidir (nenhum item): não leva ✓ nem número. */
  vazia?: boolean;
}

/** Leva à seção do documento: rola e foca o título. */
export function irParaSecao(id: string) {
  const el = document.getElementById(id);
  el?.scrollIntoView?.({ block: "start", behavior: "smooth" });
  el?.focus({ preventScroll: true });
}

/**
 * Sumário (#184): as seções do documento, com o número de pendências de cada uma e
 * ✓ quando a seção está resolvida; clicar rola até a seção.
 */
export function SumarioAnalise({ itens }: { itens: ItemSumario[] }) {
  if (itens.length === 0) return null;
  return (
    <nav className="sumario-analise" aria-label="Sumário da análise">
      <ol>
        {itens.map((i) => {
          const resolvida = !i.vazia && i.pendencias === 0;
          const situacao = i.vazia
            ? ""
            : resolvida
              ? ": resolvida"
              : `: ${i.pendencias} ${i.pendencias === 1 ? "pendência" : "pendências"}`;
          return (
            <li key={i.id}>
              <button
                type="button"
                className="sumario-item"
                aria-label={`${i.titulo}${situacao}`}
                data-resolvida={resolvida || undefined}
                onClick={() => irParaSecao(i.id)}
              >
                <span className="sumario-titulo">{i.titulo}</span>
                {resolvida && (
                  <span className="sumario-ok" aria-hidden="true">
                    <Icon name="check" size={14} />
                  </span>
                )}
                {!i.vazia && !resolvida && (
                  <span className="sumario-n" aria-hidden="true">
                    {i.pendencias}
                  </span>
                )}
              </button>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
