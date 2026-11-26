import Markdown, { type MarkdownToJSX } from "markdown-to-jsx";
import {
  Fragment,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent,
  type ReactNode,
} from "react";
import { OPCOES_MARKDOWN_BASE } from "../markdownOpcoes";
import { separarNotasSeguidas } from "../notasDeRodape";
import { overridesDeLinks } from "./regraLinks";
import "./componentes.css";

/** Caracteres por linha usados para estimar a altura quando a tela não mede (testes). */
const CARACTERES_POR_LINHA = 90;

/** Quantas linhas o texto ocupa, contando a quebra das linhas longas. */
export function estimarLinhas(texto: string): number {
  return texto
    .split("\n")
    .filter((l) => l.trim())
    .reduce((total, l) => total + Math.max(1, Math.ceil(l.length / CARACTERES_POR_LINHA)), 0);
}

/** A âncora interna (nota de rodapé) leva ao alvo dentro do texto, sem mexer no endereço. */
function irParaAncora(e: MouseEvent<HTMLElement>) {
  const href = (e.target as HTMLElement).closest("a")?.getAttribute("href");
  if (!href?.startsWith("#") || href.length < 2) return;
  const alvo = Array.from(e.currentTarget.querySelectorAll("[id]")).find((el) => el.id === href.slice(1));
  e.preventDefault();
  alvo?.scrollIntoView?.({ block: "center" });
}

/**
 * Renderizador único de Markdown do app (#406): todo texto do agente ou do
 * rastreador passa por aqui. Usa `markdown-to-jsx` com {@link OPCOES_MARKDOWN_BASE}
 * (sem HTML cru, sem `javascript:`, Mermaid como diagrama), mostra todas as notas de
 * rodapé (mesmo coladas, `[^1][^2]`) e aplica a regra única de links e imagens
 * ({@link overridesDeLinks}): só o host permitido abre, imagens não carregam.
 *
 * Com `linhas`, o texto longo mostra só essas linhas e um "ver mais" (com
 * `aria-expanded`), no lugar de cortar com "…".
 */
export function TextoRico({
  texto,
  linhas,
  hostPermitido,
  aoAbrirLink,
  overrides,
  inline = false,
  className,
}: {
  /** O Markdown. */
  texto: string;
  /** Recolhe o texto em N linhas, com "ver mais"/"ver menos". */
  linhas?: number;
  /** Endereço (ou host) cujos links abrem no navegador; os de outro host viram texto. */
  hostPermitido?: string;
  /** Abre um link do host permitido (ex.: `useAbrirLink(projeto).abrir`). */
  aoAbrirLink?: (link: string) => void;
  /** Overrides do `markdown-to-jsx` próprios do uso; `a` e `img` aqui trocam a regra padrão. */
  overrides?: MarkdownToJSX.Overrides;
  /** Texto curto numa linha (título, rótulo): sem parágrafos, dentro de um `<span>`. */
  inline?: boolean;
  /** Classe extra do contêiner (além de `markdown texto-rico`). */
  className?: string;
}) {
  const id = useId();
  const caixa = useRef<HTMLDivElement>(null);
  const [aberto, setAberto] = useState(false);
  const [transborda, setTransborda] = useState<boolean | null>(null);
  const recolhivel = !inline && linhas !== undefined && linhas > 0;

  const opcoes = useMemo<Partial<MarkdownToJSX.Options>>(
    () => ({
      ...OPCOES_MARKDOWN_BASE,
      ...(inline ? { forceBlock: false, forceInline: true } : { wrapper: Fragment }),
      overrides: { ...overridesDeLinks(hostPermitido, aoAbrirLink), ...overrides },
    }),
    [inline, hostPermitido, aoAbrirLink, overrides],
  );

  // Mede se o texto recolhido passa das N linhas; sem medida (teste, tela oculta), estima.
  useLayoutEffect(() => {
    const el = caixa.current;
    if (!recolhivel || aberto || !el) return;
    const medir = () => {
      if (el.clientHeight > 0) setTransborda(el.scrollHeight > el.clientHeight + 1);
    };
    medir();
    if (typeof ResizeObserver === "undefined") return;
    const observador = new ResizeObserver(medir);
    observador.observe(el);
    return () => observador.disconnect();
  }, [recolhivel, aberto, texto, linhas]);

  const classes = ["markdown", "texto-rico", className].filter(Boolean).join(" ");
  const conteudo = <Markdown options={opcoes}>{separarNotasSeguidas(texto)}</Markdown>;

  if (inline) {
    return (
      <span className={classes} onClick={irParaAncora}>
        {conteudo}
      </span>
    );
  }

  const comBotao = recolhivel && (aberto || (transborda ?? estimarLinhas(texto) > linhas!));
  const recolhido = recolhivel && !aberto;
  const estilo = recolhido ? ({ "--texto-rico-linhas": linhas } as CSSProperties) : undefined;
  const corpo: ReactNode = (
    <div
      ref={caixa}
      id={id}
      className={recolhido ? `${classes} texto-rico-recolhido` : classes}
      style={estilo}
      onClick={irParaAncora}
    >
      {conteudo}
    </div>
  );
  if (!recolhivel) return corpo;
  return (
    <div className="texto-rico-recolhivel">
      {corpo}
      {comBotao && (
        <button
          type="button"
          className="link texto-rico-alternar"
          aria-expanded={aberto}
          aria-controls={id}
          onClick={() => setAberto((a) => !a)}
        >
          {aberto ? "ver menos" : "ver mais"}
        </button>
      )}
    </div>
  );
}
