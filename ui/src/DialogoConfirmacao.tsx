import type { ReactNode } from "react";

/**
 * Caixa de confirmação modal sobre a tela. `Esc` cancela. O foco inicial vai para o
 * botão marcado com `autoFocus` (a opção que não perde nada).
 */
export function DialogoConfirmacao({
  titulo,
  aoCancelar,
  acoes,
  children,
}: {
  titulo: string;
  aoCancelar: () => void;
  /** Os botões, o mais seguro com `autoFocus`. */
  acoes: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="dialogo-fundo">
      <div
        className="dialogo"
        role="dialog"
        aria-modal="true"
        aria-label={titulo}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.stopPropagation();
            aoCancelar();
          }
        }}
      >
        <h3>{titulo}</h3>
        {children}
        <div className="dialogo-acoes">{acoes}</div>
      </div>
    </div>
  );
}
