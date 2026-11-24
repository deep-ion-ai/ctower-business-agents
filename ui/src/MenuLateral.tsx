import type { KeyboardEvent, ReactNode } from "react";
import {
  GRUPOS_DE_SECOES,
  ITEM_CONFIGURACOES,
  type Contagem,
  type ItemDoMenu,
  type Secao,
} from "./secoesProjeto";
import { Icon } from "./shell/icons";

/**
 * Menu lateral (#319): ícone, rótulo sempre à vista e contagem, em dois grupos
 * (Projeto e Conhecimento), com Configurações no rodapé. O item da seção à vista
 * leva `aria-current="page"`; as setas para cima e para baixo, Home e End andam
 * pelos itens.
 */
export function MenuLateral({
  atual,
  contagens,
  aoEscolher,
  rodape,
  aoNovaAnalise,
}: {
  /** Seção à vista; `null` quando uma aba de objeto está na frente. */
  atual: Secao | null;
  contagens: Partial<Record<Secao, Contagem>>;
  aoEscolher: (s: Secao) => void;
  /** Ações extras no pé (ex.: "Refazer o primeiro uso"). */
  rodape?: ReactNode;
  /** "Nova análise", ação global: abre a escolha do tipo na seção Análises. */
  aoNovaAnalise?: () => void;
}) {
  const teclas = (e: KeyboardEvent<HTMLElement>) => {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return;
    const itens = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>("button[data-item-menu]"));
    const i = itens.findIndex((b) => b === document.activeElement);
    if (i < 0) return;
    e.preventDefault();
    const n = itens.length;
    const proximo =
      e.key === "Home" ? 0 : e.key === "End" ? n - 1 : (i + (e.key === "ArrowDown" ? 1 : -1) + n) % n;
    itens[proximo].focus();
  };

  const item = (it: ItemDoMenu) => {
    const c = contagens[it.id];
    return (
      <button
        key={it.id}
        type="button"
        className="menu-item"
        data-item-menu=""
        aria-current={atual === it.id ? "page" : undefined}
        onClick={() => aoEscolher(it.id)}
      >
        <Icon name={it.icone} size={18} />
        <span className="menu-item-rotulo">{it.rotulo}</span>
        {c && c.n > 0 && (
          <span className="menu-item-contagem" data-tone={c.atencao ? "attention" : undefined}>
            {c.n}
          </span>
        )}
      </button>
    );
  };

  return (
    <nav className="menu-lateral" aria-label="Seções do projeto" onKeyDown={teclas}>
      {aoNovaAnalise && (
        <button type="button" className="btn btn-primary menu-nova" onClick={aoNovaAnalise}>
          Nova análise
        </button>
      )}
      {GRUPOS_DE_SECOES.map((g) => (
        <div key={g.titulo} className="menu-grupo" role="group" aria-label={g.titulo}>
          <span className="menu-grupo-titulo" aria-hidden="true">
            {g.titulo}
          </span>
          {g.itens.map(item)}
        </div>
      ))}
      <div className="menu-fim">
        {item(ITEM_CONFIGURACOES)}
        {rodape}
      </div>
    </nav>
  );
}
