/**
 * Renderização dos diagramas Mermaid (#118), sem rede: a biblioteca `mermaid` (MIT)
 * vai empacotada no app e é carregada só quando aparece o primeiro diagrama
 * (import dinâmico, num pedaço separado do bundle).
 *
 * - {@link desenharMermaid}: SVG do código, para a prévia (documento ao vivo, revisão,
 *   documentos da base e editor de textos). Código inválido rejeita a promessa: quem
 *   chama mostra o código e um aviso.
 * - {@link pngDoMermaid}: PNG do código, para a exportação em Word (o `cba-docs` embute
 *   a figura). Desenhado no tema claro, com fundo branco e o dobro da resolução.
 *
 * Configuração: `securityLevel: "strict"` (o Mermaid sanitiza os rótulos e desliga
 * `click`), sem rótulos em HTML (`htmlLabels: false`: o SVG só tem texto SVG, o que
 * permite desenhá-lo num canvas sem "contaminar" o canvas no WebKit) e fonte do
 * sistema, que existe também quando o SVG é desenhado como imagem.
 */

type Mermaid = (typeof import("mermaid"))["default"];

export type TemaDiagrama = "claro" | "escuro";

const FONTE = "Arial, Helvetica, sans-serif";

let carregando: Promise<Mermaid> | null = null;

function carregar(): Promise<Mermaid> {
  carregando ??= import("mermaid").then((m) => m.default);
  return carregando;
}

/** Tema da tela: o claro é o `data-theme="light"` no `<html>` (theme.css). */
export function temaDaTela(): TemaDiagrama {
  return typeof document !== "undefined" &&
    document.documentElement.getAttribute("data-theme") === "light"
    ? "claro"
    : "escuro";
}

// `initialize` é global: as renderizações (prévia e exportação, em temas diferentes)
// passam uma de cada vez.
let fila: Promise<unknown> = Promise.resolve();
let contador = 0;

function naFila<T>(tarefa: () => Promise<T>): Promise<T> {
  const resultado = fila.then(tarefa, tarefa);
  fila = resultado.catch(() => undefined);
  return resultado;
}

/** SVG do diagrama. Rejeita com a mensagem do Mermaid quando o código é inválido. */
export function desenharMermaid(codigo: string, tema: TemaDiagrama = temaDaTela()): Promise<string> {
  return naFila(async () => {
    const mermaid = await carregar();
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: "strict",
      theme: tema === "claro" ? "default" : "dark",
      fontFamily: FONTE,
      htmlLabels: false,
      flowchart: { htmlLabels: false },
    });
    await mermaid.parse(codigo);
    contador += 1;
    const { svg } = await mermaid.render(`diagrama-mermaid-${contador}`, codigo);
    return svg;
  });
}

/** Figura em PNG, com o tamanho de exibição em pixels CSS. */
export interface FiguraPng {
  png: Uint8Array;
  largura: number;
  altura: number;
}

/** Fator de resolução do PNG em relação ao tamanho de exibição. */
const ESCALA = 2;

/** Tamanho do SVG: o `viewBox` (o Mermaid põe largura 100% e `max-width`). */
function tamanhoDoSvg(svg: Element): { largura: number; altura: number } {
  const caixa = (svg.getAttribute("viewBox") ?? "")
    .split(/[\s,]+/)
    .map(Number)
    .filter((n) => Number.isFinite(n));
  if (caixa.length === 4 && caixa[2] > 0 && caixa[3] > 0) {
    return { largura: caixa[2], altura: caixa[3] };
  }
  const largura = Number.parseFloat(svg.getAttribute("width") ?? "");
  const altura = Number.parseFloat(svg.getAttribute("height") ?? "");
  if (largura > 0 && altura > 0) return { largura, altura };
  throw new Error("o diagrama não tem tamanho");
}

function carregarImagem(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const imagem = new Image();
    imagem.onload = () => resolve(imagem);
    imagem.onerror = () => reject(new Error("o navegador não desenhou o diagrama"));
    imagem.src = url;
  });
}

/** PNG do diagrama para o Word. Rejeita se o código for inválido ou o desenho falhar. */
export async function pngDoMermaid(codigo: string): Promise<FiguraPng> {
  const svg = await desenharMermaid(codigo, "claro");
  const doc = new DOMParser().parseFromString(svg, "image/svg+xml");
  const raiz = doc.documentElement;
  if (raiz.nodeName.toLowerCase() !== "svg") throw new Error("o Mermaid não devolveu um SVG");
  const { largura, altura } = tamanhoDoSvg(raiz);
  raiz.setAttribute("width", String(largura));
  raiz.setAttribute("height", String(altura));
  raiz.removeAttribute("style");
  const texto = new XMLSerializer().serializeToString(raiz);
  const imagem = await carregarImagem(
    `data:image/svg+xml;charset=utf-8,${encodeURIComponent(texto)}`,
  );
  const canvas = document.createElement("canvas");
  canvas.width = Math.ceil(largura * ESCALA);
  canvas.height = Math.ceil(altura * ESCALA);
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("canvas indisponível");
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.scale(ESCALA, ESCALA);
  ctx.drawImage(imagem, 0, 0, largura, altura);
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
  if (!blob) throw new Error("o diagrama não virou imagem");
  return {
    png: new Uint8Array(await blob.arrayBuffer()),
    largura: Math.round(largura),
    altura: Math.round(altura),
  };
}

/** Elemento do diagrama gerado pelo app (`n1`…, `L_n1_n2_0`) que recebeu o clique. */
export type ElementoDoDiagrama =
  | { tipo: "no"; indice: number }
  | { tipo: "aresta"; de: number; para: number; ocorrencia: number };

/**
 * Elemento a partir do SVG do Mermaid (flowchart): o nó tem `id` `<render>-flowchart-n3-2`
 * (terceiro nó) e a aresta, nos traços e no rótulo, `data-id` `L_n1_n2_0` (de `n1` para
 * `n2`). Aceita também `n3`. Índices a partir de 0.
 */
export function elementoDoId(id: string | null | undefined): ElementoDoDiagrama | null {
  if (!id) return null;
  const no = /^n(\d+)$/.exec(id) ?? /(?:^|-)flowchart-n(\d+)-\d+$/.exec(id);
  if (no) return { tipo: "no", indice: Number(no[1]) - 1 };
  const aresta = /^L[_-]n(\d+)[_-]n(\d+)(?:[_-](\d+))?$/.exec(id);
  if (aresta) {
    return {
      tipo: "aresta",
      de: Number(aresta[1]) - 1,
      para: Number(aresta[2]) - 1,
      ocorrencia: Number(aresta[3] ?? 0),
    };
  }
  return null;
}
