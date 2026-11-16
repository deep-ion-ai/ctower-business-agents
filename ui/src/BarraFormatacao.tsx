import type { RefObject } from "react";
import { BOTOES_FORMATO, type Formato } from "./formatacao";

/**
 * Barra de formatação do editor (#105): um botão por sintaxe Markdown, mais a Ajuda.
 * Quem usa decide o que `aoFormatar` faz (o editor aplica ao `textarea`).
 */
export function BarraFormatacao({
  aoFormatar,
  aoAjuda,
  botaoAjuda,
}: {
  aoFormatar: (formato: Formato) => void;
  aoAjuda: () => void;
  /** Ref do botão Ajuda: o modal devolve o foco a ele ao fechar. */
  botaoAjuda: RefObject<HTMLButtonElement>;
}) {
  return (
    <div className="barra-formatacao" role="toolbar" aria-label="Formatação" aria-orientation="horizontal">
      {BOTOES_FORMATO.map((b) => {
        const dica = b.atalho ? `${b.rotulo} (${b.atalho})` : b.rotulo;
        return (
          <button
            key={b.formato}
            type="button"
            className="btn btn-sm btn-formato"
            data-formato={b.formato}
            aria-label={b.rotulo}
            aria-keyshortcuts={b.atalho?.replace("Ctrl", "Control")}
            title={dica}
            // O clique não tira o foco (nem a seleção) do texto.
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => aoFormatar(b.formato)}
          >
            <span aria-hidden="true">{b.simbolo}</span>
          </button>
        );
      })}
      <button
        ref={botaoAjuda}
        type="button"
        className="btn btn-sm barra-formatacao-ajuda"
        aria-haspopup="dialog"
        onClick={aoAjuda}
      >
        Ajuda
      </button>
    </div>
  );
}
