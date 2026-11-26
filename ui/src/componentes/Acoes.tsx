import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import "./componentes.css";

/** Uma ação de botão ou de menu. */
export interface Acao {
  texto: string;
  aoEscolher: () => void;
  desabilitado?: boolean;
  /** Ação destrutiva (excluir, descartar): tom de perigo. */
  perigo?: boolean;
  /** Dica ao passar o mouse, quando o texto não basta. */
  dica?: string;
}

/** Mais que isto de ações secundárias do mesmo nível viram um menu (prancheta Padrões). */
export const MAXIMO_SECUNDARIAS = 3;

const ITENS = '[role="menuitem"]:not(:disabled)';

/**
 * Botão com "▾" que abre um menu de ações do mesmo nível ("Exportar ▾", "Nova análise
 * ▾"). Acessível: `aria-haspopup`/`aria-expanded` no botão, `role="menu"` e
 * `role="menuitem"` na lista. Ao abrir, o foco vai ao primeiro item; setas, `Home` e
 * `End` andam pelos itens; `Esc` fecha e devolve o foco ao botão; `Tab` e o clique
 * fora fecham.
 */
export function MenuAcoes({
  rotulo,
  itens,
  principal,
  desabilitado,
  icone,
}: {
  /** Texto do botão (e nome do menu). */
  rotulo: string;
  itens: Acao[];
  /** O botão é a ação principal da tela. */
  principal?: boolean;
  desabilitado?: boolean;
  /**
   * Botão só de ícone ("⋯"): o ícone fica no lugar do texto e da seta, e o `rotulo`
   * vira o nome acessível e a dica.
   */
  icone?: ReactNode;
}) {
  const [aberto, setAberto] = useState<false | "primeiro" | "ultimo">(false);
  const raiz = useRef<HTMLDivElement>(null);
  const botao = useRef<HTMLButtonElement>(null);
  const idLista = useId();

  useEffect(() => {
    if (!aberto) return;
    const lista = raiz.current?.querySelectorAll<HTMLElement>(ITENS);
    if (lista && lista.length > 0) (aberto === "ultimo" ? lista[lista.length - 1] : lista[0]).focus();
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

  const teclasBotao = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (aberto || desabilitado) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setAberto(e.key === "ArrowDown" ? "primeiro" : "ultimo");
    }
  };

  const teclasMenu = (e: KeyboardEvent<HTMLUListElement>) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      fechar();
      return;
    }
    if (e.key === "Tab") {
      // O foco volta ao botão antes de o Tab andar: segue para o próximo da tela.
      fechar();
      return;
    }
    const lista = Array.from(raiz.current?.querySelectorAll<HTMLElement>(ITENS) ?? []);
    if (lista.length === 0) return;
    const i = lista.findIndex((x) => x === document.activeElement);
    const destino =
      e.key === "ArrowDown"
        ? (i + 1) % lista.length
        : e.key === "ArrowUp"
          ? (i - 1 + lista.length) % lista.length
          : e.key === "Home"
            ? 0
            : e.key === "End"
              ? lista.length - 1
              : null;
    if (destino === null) return;
    e.preventDefault();
    lista[destino].focus();
  };

  return (
    <div className="menu-acoes" ref={raiz}>
      <button
        ref={botao}
        type="button"
        className={`${principal ? "btn btn-primary" : "btn"}${icone ? " menu-acoes-icone" : ""}`}
        aria-label={icone ? rotulo : undefined}
        title={icone ? rotulo : undefined}
        aria-haspopup="menu"
        aria-expanded={!!aberto}
        aria-controls={aberto ? idLista : undefined}
        disabled={desabilitado}
        onClick={() => setAberto((a) => (a ? false : "primeiro"))}
        onKeyDown={teclasBotao}
      >
        {icone ?? (
          <>
            {rotulo}
            <span aria-hidden="true" className="menu-acoes-seta">
              ▾
            </span>
          </>
        )}
      </button>
      {aberto && (
        <ul id={idLista} className="menu-acoes-lista" role="menu" aria-label={rotulo} onKeyDown={teclasMenu}>
          {itens.map((item) => (
            <li key={item.texto} role="none">
              <button
                type="button"
                role="menuitem"
                tabIndex={-1}
                disabled={item.desabilitado}
                title={item.dica}
                data-perigo={item.perigo || undefined}
                onClick={() => {
                  fechar();
                  item.aoEscolher();
                }}
              >
                {item.texto}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function classeBotao(acao: Acao, principal: boolean): string {
  if (acao.perigo) return "btn btn-danger";
  return principal ? "btn btn-primary" : "btn";
}

function BotaoAcao({ acao, principal = false }: { acao: Acao; principal?: boolean }) {
  return (
    <button
      type="button"
      className={classeBotao(acao, principal)}
      disabled={acao.desabilitado}
      title={acao.dica}
      onClick={acao.aoEscolher}
    >
      {acao.texto}
    </button>
  );
}

/**
 * Fileira de ações de uma tela ou seção: uma ação principal (no máximo uma) e as
 * secundárias. Com mais de {@link MAXIMO_SECUNDARIAS} secundárias, elas viram um
 * {@link MenuAcoes} ("Mais ações ▾"). `children` entra no fim da fileira (um
 * `MenuAcoes` próprio, um `BotaoIcone`).
 */
export function BarraAcoes({
  principal,
  secundarias = [],
  rotulo = "Ações",
  rotuloMenu = "Mais ações",
  alinhamento = "inicio",
  children,
}: {
  principal?: Acao;
  secundarias?: Acao[];
  /** Nome acessível do grupo. */
  rotulo?: string;
  /** Texto do menu quando as secundárias não cabem na fileira. */
  rotuloMenu?: string;
  /** "fim": encosta à direita (cabeçalho da tela). */
  alinhamento?: "inicio" | "fim";
  children?: ReactNode;
}) {
  const noMenu = secundarias.length > MAXIMO_SECUNDARIAS;
  return (
    <div className="barra-acoes" role="group" aria-label={rotulo} data-alinhamento={alinhamento}>
      {principal && <BotaoAcao acao={principal} principal />}
      {noMenu ? (
        <MenuAcoes rotulo={rotuloMenu} itens={secundarias} />
      ) : (
        secundarias.map((a) => <BotaoAcao key={a.texto} acao={a} />)
      )}
      {children}
    </div>
  );
}
