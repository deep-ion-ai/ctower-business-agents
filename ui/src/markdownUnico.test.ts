/// <reference types="vite/client" />
// Markdown só pelo TextoRico (#410): nenhum outro arquivo importa o renderizador de
// Markdown. Fora do componente, só os tipos e as opções que ele mesmo usa. O nome do
// pacote é montado, para este arquivo não aparecer na busca pelo import.

const fontes = import.meta.glob<string>(["./**/*.ts", "./**/*.tsx", "!./markdownUnico.test.ts"], {
  query: "?raw",
  import: "default",
  eager: true,
});

/** O pacote do renderizador de Markdown. */
const PACOTE = ["markdown", "to", "jsx"].join("-");

/** Quem pode importar o pacote: o TextoRico e o que ele usa. */
const PERMITIDOS = new Set([
  "./componentes/TextoRico.tsx",
  "./componentes/regraLinks.tsx",
  "./markdownOpcoes.ts",
  "./notasDeRodape.ts",
]);

const IMPORTA = new RegExp(`(?:from\\s+|import\\s*\\(\\s*|import\\s+|require\\s*\\(\\s*)["']${PACOTE}["']`);

describe("Markdown só pelo TextoRico (#410)", () => {
  it("lê as fontes do app", () => {
    expect(fontes["./componentes/TextoRico.tsx"]).toMatch(IMPORTA);
  });

  it("nenhum outro arquivo importa o pacote de Markdown", () => {
    const fora = Object.entries(fontes)
      .filter(([caminho, texto]) => !PERMITIDOS.has(caminho) && IMPORTA.test(texto))
      .map(([caminho]) => caminho);
    expect(fora).toEqual([]);
  });
});
