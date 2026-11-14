import Markdown from "markdown-to-jsx";
import { createContext, Fragment, useContext, useState, type ReactNode } from "react";

/** Trecho do texto de uma afirmação: prosa ou código. */
export type Segmento = { tipo: "texto"; valor: string } | { tipo: "codigo"; valor: string };

/** Código maior que isto fica recolhido atrás de "ver código". */
export const LIMITE_CODIGO_INLINE = 60;

/** Abaixo disto, um trecho entre chaves ou colchetes sem crase continua prosa. */
const MINIMO_CODIGO_CRU = 30;

const ABRE: Record<string, string> = { "{": "}", "[": "]", "(": ")" };

/** Fim do bloco balanceado que começa em `inicio` (`{` ou `[`), ou -1. */
function fimBalanceado(texto: string, inicio: number): number {
  const pilha: string[] = [];
  let aspa: string | null = null;
  for (let i = inicio; i < texto.length; i++) {
    const c = texto[i];
    if (aspa) {
      if (c === "\\") i++;
      else if (c === aspa) aspa = null;
      continue;
    }
    if (c === '"' || c === "'") aspa = c;
    else if (ABRE[c]) pilha.push(ABRE[c]);
    else if (c === "}" || c === "]" || c === ")") {
      if (pilha.pop() !== c) return -1;
      if (pilha.length === 0) return i;
    }
  }
  return -1;
}

/** Separa a prosa de trechos de código cru (sem crase) por chaves e colchetes balanceados. */
function codigoCru(texto: string): Segmento[] {
  const saida: Segmento[] = [];
  let resto = 0;
  let i = 0;
  while (i < texto.length) {
    const c = texto[i];
    if (c !== "{" && c !== "[") {
      i++;
      continue;
    }
    const fim = fimBalanceado(texto, i);
    if (fim < 0) {
      i++;
      continue;
    }
    // Volta até o começo da palavra colada no bloco: `montar(e){`, `Jae=[`.
    let ini = i;
    while (ini > resto && !/\s/.test(texto[ini - 1])) ini--;
    // Segue o que vem colado depois (`}.map(...)`), sem levar a pontuação final.
    let depois = fim + 1;
    while (depois < texto.length && !/\s/.test(texto[depois])) depois++;
    while (depois > fim + 1 && /[.,;:!?]/.test(texto[depois - 1])) depois--;
    const trecho = texto.slice(ini, depois);
    // Link do markdown (`[texto](url)`) e trechos curtos continuam prosa.
    if (trecho.length < MINIMO_CODIGO_CRU || /^\[[^\]]*\]\(/.test(trecho)) {
      i = fim + 1;
      continue;
    }
    if (ini > resto) saida.push({ tipo: "texto", valor: texto.slice(resto, ini) });
    saida.push({ tipo: "codigo", valor: trecho });
    resto = depois;
    i = depois;
  }
  if (resto < texto.length) saida.push({ tipo: "texto", valor: texto.slice(resto) });
  return saida;
}

/**
 * Divide o texto em prosa e código: o que está entre crases e, fora delas, trechos
 * longos entre chaves ou colchetes (código ou JSON colado sem crase pelo agente).
 */
export function segmentar(texto: string): Segmento[] {
  const saida: Segmento[] = [];
  const partes = texto.split(/(`[^`\n]+`)/);
  for (const parte of partes) {
    if (!parte) continue;
    if (parte.length > 2 && parte.startsWith("`") && parte.endsWith("`"))
      saida.push({ tipo: "codigo", valor: parte.slice(1, -1) });
    else saida.push(...codigoCru(parte));
  }
  return saida;
}

/**
 * `_` no meio de uma palavra é parte do nome (`BNMP_EXTERNO_INTERACAO`), não ênfase;
 * nem o que abre um nome em maiúsculas (`_CONSULTA`):
 * escapado, o renderizador o mostra como está.
 */
function semEnfaseNaPalavra(texto: string): string {
  return texto.replace(
    // No meio da palavra, ou antes de um nome em maiúsculas (`_CONSULTA`, sufixo citado).
    /(?<=[\p{L}\p{N}])_(?=[\p{L}\p{N}_])|_(?=[A-Z0-9][A-Z0-9_]*(?![\p{L}\p{N}]))/gu,
    "\\_",
  );
}

/**
 * Markdown do texto com o código cru entre crases, para o renderizador tratá-lo como
 * código. Blocos cercados (```) ficam como estão.
 */
export function comCodigoMarcado(texto: string): string {
  return texto
    .split(/(^```[^\n]*\n[\s\S]*?^```[^\n]*$)/m)
    .map((parte) =>
      parte.startsWith("```")
        ? parte
        : segmentar(parte)
            .map((s) => (s.tipo === "codigo" ? `\`${s.valor}\`` : semEnfaseNaPalavra(s.valor)))
            .join(""),
    )
    .join("");
}

/** Dentro de um bloco de código, o `code` não se recolhe de novo. */
const EmBloco = createContext(false);

function Recolhido({ children }: { children: ReactNode }) {
  const [aberto, setAberto] = useState(false);
  return (
    <>
      <button
        type="button"
        className="ver-codigo"
        aria-expanded={aberto}
        onClick={() => setAberto((a) => !a)}
      >
        {aberto ? "ocultar código" : "ver código"}
      </button>
      {aberto && <pre className="codigo-afirmacao">{children}</pre>}
    </>
  );
}

function textoDe(no: ReactNode): string {
  if (typeof no === "string" || typeof no === "number") return String(no);
  if (Array.isArray(no)) return no.map(textoDe).join("");
  return "";
}

function Codigo({ children }: { children?: ReactNode }) {
  const emBloco = useContext(EmBloco);
  if (emBloco) return <code>{children}</code>;
  return textoDe(children).length > LIMITE_CODIGO_INLINE ? (
    <Recolhido>{children}</Recolhido>
  ) : (
    <code>{children}</code>
  );
}

/** Bloco cercado: o analista quase nunca lê código, então fica recolhido. */
function Bloco({ children }: { children?: ReactNode }) {
  return (
    <EmBloco.Provider value>
      <Recolhido>{children}</Recolhido>
    </EmBloco.Provider>
  );
}

/** Título no meio de uma afirmação vira só um destaque, sem quebrar a hierarquia. */
function Destaque({ children }: { children?: ReactNode }) {
  return <strong className="destaque-afirmacao">{children}</strong>;
}

/** Links não navegam dentro do app: viram texto. */
function SemLink({ children }: { children?: ReactNode }) {
  return <span className="link-externo">{children}</span>;
}

// Fora do componente: um objeto novo a cada render remontaria o conteúdo.
const OPCOES = {
  disableParsingRawHTML: true,
  wrapper: Fragment,
  overrides: {
    code: { component: Codigo },
    pre: { component: Bloco },
    h1: { component: Destaque },
    h2: { component: Destaque },
    h3: { component: Destaque },
    h4: { component: Destaque },
    h5: { component: Destaque },
    h6: { component: Destaque },
    a: { component: SemLink },
    img: { component: ({ alt }: { alt?: string }) => <span>{alt}</span> },
  },
} as const;

/**
 * Texto de uma afirmação (#130): Markdown simples (listas, negrito, código), sem HTML
 * cru; o código, entre crases ou colado sem elas, aparece formatado e, quando longo,
 * recolhido.
 */
export function TextoAfirmacao({ texto }: { texto: string }) {
  return (
    <span className="texto">
      <Markdown options={OPCOES}>{comCodigoMarcado(texto)}</Markdown>
    </span>
  );
}
