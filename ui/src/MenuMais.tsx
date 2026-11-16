import { useEffect, useRef, useState, type KeyboardEvent } from "react";

export interface ItemMenuMais {
  texto: string;
  aoEscolher: () => void;
  desabilitado?: boolean;
}

/**
 * Botão “⋯” com um menu de ações secundárias: `aria-haspopup`/`aria-expanded` no botão,
 * `role="menu"` na lista. Setas andam pelos itens, `Esc` e o clique fora fecham, e o
 * foco volta ao botão.
 */
export function MenuMais({ rotulo, itens }: { rotulo: string; itens: ItemMenuMais[] }) {
  const [aberto, setAberto] = useState(false);
  const raiz = useRef<HTMLDivElement>(null);
  const botao = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!aberto) return;
    raiz.current?.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')?.focus();
    const fora = (e: MouseEvent) => {
      if (raiz.current && !raiz.current.contains(e.target as Node)) setAberto(false);
    };
    document.addEventListener("mousedown", fora);
    return () => document.removeEventListener("mousedown", fora);
  }, [aberto]);

  const fechar = () => {
    setAberto(false);
    botao.current?.focus();
  };

  const teclas = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Escape" && aberto) {
      e.stopPropagation();
      fechar();
      return;
    }
    if (!aberto || (e.key !== "ArrowDown" && e.key !== "ArrowUp")) return;
    const lista = Array.from(
      raiz.current?.querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled)') ?? [],
    );
    if (lista.length === 0) return;
    e.preventDefault();
    const i = lista.findIndex((x) => x === document.activeElement);
    const passo = e.key === "ArrowDown" ? 1 : -1;
    lista[(i + passo + lista.length) % lista.length].focus();
  };

  return (
    <div className="menu-mais" ref={raiz} onKeyDown={teclas}>
      <button
        ref={botao}
        type="button"
        className="btn btn-sm menu-mais-botao"
        aria-label={rotulo}
        aria-haspopup="menu"
        aria-expanded={aberto}
        onClick={() => setAberto((a) => !a)}
      >
        <span aria-hidden="true">⋯</span>
      </button>
      {aberto && (
        <ul className="menu-mais-lista" role="menu" aria-label={rotulo}>
          {itens.map((i) => (
            <li key={i.texto} role="none">
              <button
                type="button"
                role="menuitem"
                disabled={i.desabilitado}
                onClick={() => {
                  fechar();
                  i.aoEscolher();
                }}
              >
                {i.texto}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
