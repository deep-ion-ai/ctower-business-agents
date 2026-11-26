import { useEffect, useRef, type ReactNode } from "react";
import { TituloSecao } from "./componentes";

const FOCAVEIS =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Diálogo modal da aprovação (#186): foco preso (Tab e Shift+Tab giram dentro dele),
 * Esc fecha e o foco volta a quem abriu. Fecha também pelo clique no fundo.
 */
export function AprovacaoDialogo({
  titulo,
  aoFechar,
  children,
}: {
  titulo: string;
  aoFechar: () => void;
  children: ReactNode;
}) {
  const caixa = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const anterior = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    caixa.current?.focus();
    return () => anterior?.focus();
  }, []);

  const tecla = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      aoFechar();
      return;
    }
    if (e.key !== "Tab" || !caixa.current) return;
    const itens = Array.from(caixa.current.querySelectorAll<HTMLElement>(FOCAVEIS));
    if (itens.length === 0) {
      e.preventDefault();
      return;
    }
    const primeiro = itens[0];
    const ultimo = itens[itens.length - 1];
    const ativo = document.activeElement;
    if (e.shiftKey && (ativo === primeiro || ativo === caixa.current)) {
      e.preventDefault();
      ultimo.focus();
    } else if (!e.shiftKey && ativo === ultimo) {
      e.preventDefault();
      primeiro.focus();
    }
  };

  return (
    <div
      className="dialogo-fundo"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) aoFechar();
      }}
    >
      <div
        ref={caixa}
        className="dialogo dialogo-aprovacao"
        role="dialog"
        aria-modal="true"
        aria-label={titulo}
        tabIndex={-1}
        onKeyDown={tecla}
      >
        <TituloSecao nivel={3}>{titulo}</TituloSecao>
        {children}
        <div className="dialogo-acoes">
          <button className="btn" type="button" onClick={aoFechar}>
            Fechar
          </button>
        </div>
      </div>
    </div>
  );
}
