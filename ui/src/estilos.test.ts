/// <reference types="vite/client" />
// Identidade visual (#405, épico #411): as cores vêm só do theme.css e os tamanhos
// de fonte só da escala --fs-*. Este teste lê as folhas de estilo e os componentes
// como texto e falha apontando arquivo:linha de cada valor solto.

const css = import.meta.glob<string>("./**/*.css", {
  query: "?raw",
  import: "default",
  eager: true,
});
const tsx = import.meta.glob<string>(["./**/*.tsx", "!./**/*.test.tsx", "!./test/**"], {
  query: "?raw",
  import: "default",
  eager: true,
});

/** Troca os comentários por espaços, mantendo as quebras de linha (e a numeração). */
function semComentarios(texto: string, jsx: boolean): string {
  const branco = (m: string) => m.replace(/[^\n]/g, " ");
  let t = texto.replace(/\/\*[\s\S]*?\*\//g, branco);
  // Em TS/TSX, também `// …` (não o de "https://").
  if (jsx) t = t.replace(/(^|[^:"'`\\])\/\/[^\n]*/g, (m, antes: string) => antes + branco(m.slice(antes.length)));
  return t;
}

/**
 * Cor literal: hexadecimal (#rgb, #rgba, #rrggbb, #rrggbbaa), rgb()/rgba() ou
 * hsl()/hsla(). Um "#" só com 3 ou 4 dígitos é referência a issue (#142), não cor.
 */
const HEX = /#([0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})\b/g;
const FUNCAO_COR = /\b(?:rgba?|hsla?)\(/;

function coresSoltas(texto: string): string[] {
  const achadas: string[] = [];
  for (const m of texto.matchAll(HEX)) {
    const digitos = m[1];
    if (digitos.length <= 4 && /^\d+$/.test(digitos)) continue;
    achadas.push(m[0]);
  }
  const f = texto.match(FUNCAO_COR);
  if (f) achadas.push(f[0]);
  return achadas;
}

/**
 * Uma linha com o comentário `estilos:ignorar` fica de fora: use só para texto que
 * parece cor sem ser estilo (o exemplo "#1f6feb" de um placeholder, por exemplo).
 */

/** `font-size` aceito: um token da escala ou `inherit`. Nada de px, em ou rem soltos. */
const TAMANHO_OK = /^(var\(--fs-(title|section|reading|base|support|small)\)|inherit)$/;

function violacoes(arquivos: Record<string, string>, ehCss: boolean): string[] {
  const erros: string[] = [];
  for (const [caminho, bruto] of Object.entries(arquivos)) {
    const nome = caminho.replace(/^\.\//, "src/");
    const brutas = bruto.split("\n");
    const linhas = semComentarios(bruto, !ehCss).split("\n");
    const ehTema = nome === "src/theme.css";
    linhas.forEach((linha, i) => {
      // Texto que só parece cor (ex.: um exemplo num placeholder): marcado na linha.
      if (brutas[i].includes("estilos:ignorar")) return;
      const onde = `${nome}:${i + 1}`;
      // O theme.css define as cores (linhas "--token: valor;"), e só ele.
      const defineToken = ehTema && /^\s*--[\w-]+\s*:/.test(linha);
      if (!defineToken) {
        for (const cor of coresSoltas(linha)) erros.push(`${onde}: cor solta ${cor}`);
      }
      if (ehCss) {
        for (const m of linha.matchAll(/font-size\s*:\s*([^;}]+)/g)) {
          const valor = m[1].trim();
          if (!TAMANHO_OK.test(valor)) erros.push(`${onde}: font-size fora da escala (${valor})`);
        }
        for (const m of linha.matchAll(/(?:^|[\s;{])font\s*:\s*([^;}]+)/g)) {
          const valor = m[1].trim();
          if (valor !== "inherit" && !/var\(--fs-[a-z]+\)/.test(valor))
            erros.push(`${onde}: font sem tamanho da escala (${valor})`);
        }
      } else {
        for (const m of linha.matchAll(/fontSize\s*:\s*([^,}]+)/g)) {
          const valor = m[1].trim().replace(/^["'`]|["'`]$/g, "");
          if (!TAMANHO_OK.test(valor)) erros.push(`${onde}: fontSize fora da escala (${valor})`);
        }
      }
    });
  }
  return erros;
}

describe("identidade visual: só tokens (#405)", () => {
  it("acha as folhas de estilo e os componentes", () => {
    expect(Object.keys(css)).toContain("./theme.css");
    expect(Object.keys(css)).toContain("./telas.css");
    expect(Object.keys(css).length).toBeGreaterThan(5);
    expect(Object.keys(tsx).length).toBeGreaterThan(50);
    // O ?raw trouxe o texto, não um módulo vazio.
    expect(css["./theme.css"]).toContain("--fs-title");
  });

  it("nenhuma cor literal nem tamanho de fonte fora da escala nos .css", () => {
    expect(violacoes(css, true)).toEqual([]);
  });

  it("nenhuma cor literal nem fontSize fora da escala nos .tsx", () => {
    expect(violacoes(tsx, false)).toEqual([]);
  });

  it("a checagem aponta arquivo:linha do que está fora", () => {
    const folha = [
      "/* #fff no comentário não conta (#142) */",
      ".a {",
      "  color: #fff;",
      "  font-size: 13px;",
      "  background: rgba(0, 0, 0, 0.2);",
      "  font: 600 12px var(--sans);",
      "}",
      ".b { font-size: var(--fs-base); font: inherit; }",
    ].join("\n");
    expect(violacoes({ "./x.css": folha }, true)).toEqual([
      "src/x.css:3: cor solta #fff",
      "src/x.css:4: font-size fora da escala (13px)",
      "src/x.css:5: cor solta rgba(",
      "src/x.css:6: font sem tamanho da escala (600 12px var(--sans))",
    ]);
    const componente = [
      "// #abcdef no comentário não conta",
      'const s = { color: "#1f5fbf", fontSize: 12 };',
      'const t = "issue #142, https://exemplo/#abc";',
      'const u = { fontSize: "var(--fs-small)" };',
      'const v = "Ex.: #1f6feb"; // estilos:ignorar',
    ].join("\n");
    expect(violacoes({ "./Y.tsx": componente }, false)).toEqual([
      "src/Y.tsx:2: cor solta #1f5fbf",
      "src/Y.tsx:2: fontSize fora da escala (12)",
      "src/Y.tsx:3: cor solta #abc",
    ]);
  });
});
