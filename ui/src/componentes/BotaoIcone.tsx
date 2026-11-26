import type { ButtonHTMLAttributes, ReactNode } from "react";

/**
 * Botão só de ícone, para a ação repetida em cada item de lista (#406): o `rotulo`
 * é obrigatório e vira o nome acessível (`aria-label`) e a dica (`title`) ao passar
 * o mouse. `tom` destaca a ação principal do item ou a destrutiva. Os demais
 * atributos (`aria-expanded`, `aria-controls`...) passam para o `<button>`.
 */
export function BotaoIcone({
  rotulo,
  dica,
  onClick,
  disabled,
  tom,
  children,
  ...outros
}: {
  rotulo: string;
  /** Dica mais longa que o rótulo, quando ajuda. */
  dica?: string;
  onClick: () => void;
  disabled?: boolean;
  tom?: "principal" | "perigo";
  /** O ícone (ex.: `<IconeConfirmar />`), sempre `aria-hidden`. */
  children: ReactNode;
} & Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "aria-label" | "title" | "onClick" | "disabled" | "children" | "type" | "className"
>) {
  return (
    <button
      {...outros}
      type="button"
      className={tom ? `icone-acao icone-${tom}` : "icone-acao"}
      aria-label={rotulo}
      title={dica ?? rotulo}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
