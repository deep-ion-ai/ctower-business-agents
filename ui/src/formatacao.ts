/**
 * Lógica pura da barra de formatação do editor: dado o texto e a seleção, calcula a
 * edição que aplica a sintaxe Markdown. Não toca no DOM; quem aplica (com `insertText`,
 * para o desfazer funcionar) é `aplicarFormatoNoCampo`, em `inserirNoCursor.ts`.
 */

export type Formato =
  | "titulo"
  | "negrito"
  | "italico"
  | "lista"
  | "listaNumerada"
  | "link"
  | "codigo"
  | "blocoCodigo"
  | "citacao"
  | "tabela";

/** Troca `[inicio, fim)` por `texto` e deixa `[selInicio, selFim)` selecionado (no texto novo). */
export interface Edicao {
  inicio: number;
  fim: number;
  texto: string;
  selInicio: number;
  selFim: number;
}

export const MODELO = {
  negrito: "texto em negrito",
  italico: "texto em itálico",
  codigo: "código",
  blocoCodigo: "código",
  titulo: "Título",
  lista: "item",
  listaNumerada: "item",
  citacao: "citação",
  link: "texto do link",
  url: "https://",
  tabela: "Coluna 1",
} as const;

/** Formato do atalho de teclado (Ctrl/Cmd + B, I ou K, sem Shift/Alt), ou `null`. */
export function formatoDoAtalho(e: {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}): Formato | null {
  if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey) return null;
  switch (e.key.toLowerCase()) {
    case "b":
      return "negrito";
    case "i":
      return "italico";
    case "k":
      return "link";
    default:
      return null;
  }
}

function envolver(valor: string, ini: number, fim: number, marca: string, modelo: string): Edicao {
  const n = marca.length;
  if (ini === fim) {
    return {
      inicio: ini,
      fim,
      texto: `${marca}${modelo}${marca}`,
      selInicio: ini + n,
      selFim: ini + n + modelo.length,
    };
  }
  const sel = valor.slice(ini, fim);
  // Já envolvida (a seleção inclui as marcas): tira.
  if (sel.length > 2 * n && sel.startsWith(marca) && sel.endsWith(marca)) {
    const dentro = sel.slice(n, sel.length - n);
    return { inicio: ini, fim, texto: dentro, selInicio: ini, selFim: ini + dentro.length };
  }
  // Marcas logo em volta da seleção: tira. Para o itálico (`*`) não confunde com `**`.
  const em = (a: number, b: number) => valor.slice(a, b) === marca;
  const colada = marca === "*" && (valor[ini - 2] === "*" || valor[fim + 1] === "*");
  if (ini >= n && em(ini - n, ini) && em(fim, fim + n) && !colada) {
    return {
      inicio: ini - n,
      fim: fim + n,
      texto: sel,
      selInicio: ini - n,
      selFim: ini - n + sel.length,
    };
  }
  return {
    inicio: ini,
    fim,
    texto: `${marca}${sel}${marca}`,
    selInicio: ini + n,
    selFim: ini + n + sel.length,
  };
}

/** Estende a seleção para as linhas inteiras que ela toca. */
function linhasDa(valor: string, ini: number, fim: number) {
  const inicio = valor.lastIndexOf("\n", ini - 1) + 1;
  // Seleção que termina logo depois de uma quebra não toca a linha seguinte.
  const ate = fim > ini && valor[fim - 1] === "\n" ? fim - 1 : fim;
  const f = valor.indexOf("\n", ate);
  const fimLinha = f === -1 ? valor.length : f;
  return { inicio, fim: fimLinha, linhas: valor.slice(inicio, fimLinha).split("\n") };
}

function prefixoDeLinha(
  valor: string,
  ini: number,
  fim: number,
  modelo: string,
  prefixo: (i: number) => string,
  temPrefixo: (linha: string) => RegExpExecArray | null,
): Edicao {
  const { inicio, fim: fimLinha, linhas } = linhasDa(valor, ini, fim);
  if (linhas.length === 1 && linhas[0].trim() === "") {
    const p = prefixo(0);
    return {
      inicio,
      fim: fimLinha,
      texto: p + modelo,
      selInicio: inicio + p.length,
      selFim: inicio + p.length + modelo.length,
    };
  }
  const todas = linhas.filter((l) => l.trim() !== "").every((l) => temPrefixo(l));
  let n = 0;
  const novas = linhas.map((l) => {
    if (l.trim() === "") return l;
    const m = temPrefixo(l);
    if (todas) return l.slice(m![0].length);
    // Troca um prefixo do mesmo tipo (lista numerada) em vez de empilhar.
    return prefixo(n++) + (m ? l.slice(m[0].length) : l);
  });
  const texto = novas.join("\n");
  return { inicio, fim: fimLinha, texto, selInicio: inicio, selFim: inicio + texto.length };
}

const TEM_TITULO = (l: string) => /^#{1,6} /.exec(l);
const TEM_LISTA = (l: string) => /^[-*+] /.exec(l);
const TEM_NUMERO = (l: string) => /^\d+[.)] /.exec(l);
const TEM_CITACAO = (l: string) => /^> ?/.exec(l);

function titulo(valor: string, ini: number, fim: number): Edicao {
  const { inicio, fim: fimLinha, linhas } = linhasDa(valor, ini, fim);
  if (linhas.length === 1 && linhas[0].trim() === "") {
    return {
      inicio,
      fim: fimLinha,
      texto: `## ${MODELO.titulo}`,
      selInicio: inicio + 3,
      selFim: inicio + 3 + MODELO.titulo.length,
    };
  }
  // Uma só linha: `## ` liga e desliga.
  const unica = linhas.length === 1 ? linhas[0] : null;
  if (unica !== null && unica.startsWith("## ")) {
    const t = unica.slice(3);
    return { inicio, fim: fimLinha, texto: t, selInicio: inicio, selFim: inicio + t.length };
  }
  const novas = linhas.map((l) => {
    if (l.trim() === "") return l;
    const m = TEM_TITULO(l);
    return `## ${m ? l.slice(m[0].length) : l}`;
  });
  const texto = novas.join("\n");
  return {
    inicio,
    fim: fimLinha,
    texto,
    selInicio: unica !== null ? inicio + 3 : inicio,
    selFim: inicio + texto.length,
  };
}

/** Quebra de linha antes do bloco, se o cursor não está no começo de uma linha. */
function quebraAntes(valor: string, ini: number): string {
  return ini === 0 || valor[ini - 1] === "\n" ? "" : "\n";
}

function blocoDeCodigo(valor: string, ini: number, fim: number): Edicao {
  const antes = quebraAntes(valor, ini);
  const depois = fim >= valor.length || valor[fim] === "\n" ? "" : "\n";
  const miolo = ini === fim ? MODELO.blocoCodigo : valor.slice(ini, fim);
  const abre = `${antes}\`\`\`\n`;
  return {
    inicio: ini,
    fim,
    texto: `${abre}${miolo}\n\`\`\`${depois}`,
    selInicio: ini + abre.length,
    selFim: ini + abre.length + miolo.length,
  };
}

function link(valor: string, ini: number, fim: number): Edicao {
  const sel = valor.slice(ini, fim);
  if (sel === "" || /^(https?:\/\/|www\.)\S+$/i.test(sel)) {
    // Sem seleção, ou a seleção já é o endereço: falta o texto do link.
    return {
      inicio: ini,
      fim,
      texto: `[${MODELO.link}](${sel === "" ? MODELO.url : sel})`,
      selInicio: ini + 1,
      selFim: ini + 1 + MODELO.link.length,
    };
  }
  const url = ini + sel.length + 3; // depois de `[sel](`
  return {
    inicio: ini,
    fim,
    texto: `[${sel}](${MODELO.url})`,
    selInicio: url,
    selFim: url + MODELO.url.length,
  };
}

function tabela(valor: string, ini: number, fim: number): Edicao {
  const antes = quebraAntes(valor, ini);
  // A seleção, se houver, vira o título da primeira coluna: nada se perde.
  const primeira = ini === fim ? MODELO.tabela : valor.slice(ini, fim).replace(/\s*\n\s*/g, " ");
  const texto = `${antes}| ${primeira} | Coluna 2 |\n| --- | --- |\n| valor | valor |`;
  const comeco = ini + antes.length + 2;
  return { inicio: ini, fim, texto, selInicio: comeco, selFim: comeco + primeira.length };
}

/** A edição que aplica `formato` ao texto com a seleção `[ini, fim)`. */
export function formatar(formato: Formato, valor: string, ini: number, fim: number): Edicao {
  switch (formato) {
    case "negrito":
      return envolver(valor, ini, fim, "**", MODELO.negrito);
    case "italico":
      return envolver(valor, ini, fim, "*", MODELO.italico);
    case "codigo":
      return envolver(valor, ini, fim, "`", MODELO.codigo);
    case "titulo":
      return titulo(valor, ini, fim);
    case "lista":
      return prefixoDeLinha(valor, ini, fim, MODELO.lista, () => "- ", TEM_LISTA);
    case "listaNumerada":
      return prefixoDeLinha(valor, ini, fim, MODELO.listaNumerada, (i) => `${i + 1}. `, TEM_NUMERO);
    case "citacao":
      return prefixoDeLinha(valor, ini, fim, MODELO.citacao, () => "> ", TEM_CITACAO);
    case "blocoCodigo":
      return blocoDeCodigo(valor, ini, fim);
    case "link":
      return link(valor, ini, fim);
    case "tabela":
      return tabela(valor, ini, fim);
  }
}

export interface BotaoFormato {
  formato: Formato;
  rotulo: string;
  /** Texto curto do botão. */
  simbolo: string;
  atalho?: string;
}

export const BOTOES_FORMATO: readonly BotaoFormato[] = [
  { formato: "titulo", rotulo: "Título", simbolo: "T" },
  { formato: "negrito", rotulo: "Negrito", simbolo: "N", atalho: "Ctrl+B" },
  { formato: "italico", rotulo: "Itálico", simbolo: "I", atalho: "Ctrl+I" },
  { formato: "lista", rotulo: "Lista", simbolo: "•" },
  { formato: "listaNumerada", rotulo: "Lista numerada", simbolo: "1." },
  { formato: "link", rotulo: "Link", simbolo: "↗", atalho: "Ctrl+K" },
  { formato: "codigo", rotulo: "Código", simbolo: "</>" },
  { formato: "blocoCodigo", rotulo: "Bloco de código", simbolo: "{ }" },
  { formato: "citacao", rotulo: "Citação", simbolo: "”" },
  { formato: "tabela", rotulo: "Tabela", simbolo: "▦" },
];
