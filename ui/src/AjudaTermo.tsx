import { useId, useState } from "react";

/**
 * “?” ao lado de um termo do app, com uma frase de explicação (#325). Abre ao clicar ou
 * ao focar, e fecha com Esc; a frase também fica em `aria-label` para leitor de tela.
 */
export function AjudaTermo({ termo, children }: { termo: string; children: string }) {
  const id = useId();
  const [aberta, setAberta] = useState(false);
  return (
    <span className="ajuda-termo">
      <button
        type="button"
        className="ajuda-termo-botao"
        aria-label={`O que é ${termo}?`}
        aria-expanded={aberta}
        aria-controls={id}
        title={children}
        onClick={() => setAberta((a) => !a)}
        onKeyDown={(e) => {
          if (e.key === "Escape") setAberta(false);
        }}
      >
        ?
      </button>
      {aberta && (
        <span id={id} role="note" className="ajuda-termo-texto">
          {children}
        </span>
      )}
    </span>
  );
}
