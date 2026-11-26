/// <reference types="vite/client" />
// As classes dos componentes compartilhados (#406) são só deles: nenhuma outra folha
// de estilo as usa como seletor e nenhuma outra marcação as põe no className. Sem
// isso, um `.cartao` antigo vaza para o Cartao (e vice-versa) sem ninguém perceber.

const css = import.meta.glob<string>("../**/*.css", { query: "?raw", import: "default", eager: true });
const tsx = import.meta.glob<string>(["../**/*.tsx", "!../**/*.test.tsx", "!../test/**"], {
  query: "?raw",
  import: "default",
  eager: true,
});

/** Caminho a partir de `src/`, venha a chave como `./x` (esta pasta) ou `../x`. */
const nomeDe = (chave: string) =>
  chave.startsWith("./") ? `src/componentes/${chave.slice(2)}` : `src/${chave.replace(/^\.\.\//, "")}`;
const deFora = (chave: string) => !nomeDe(chave).startsWith("src/componentes/");

/**
 * Ajustes de contexto intencionais, que estilizam o componente onde ele é usado.
 * Só entra aqui o seletor que mira o componente compartilhado de propósito.
 */
const PERMITIDOS = new Set([
  // #403: o TituloSecao no documento ao vivo ganha espaço abaixo.
  ".documento-ao-vivo .titulo-secao",
  ".documento-ao-vivo .titulo-secao > h3",
  // #410: a lista de itens no documento ao vivo não herda o recuo das listas do texto.
  ".documento-ao-vivo .lista-itens",
]);

const semComentarios = (t: string) => t.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "));

/** Os seletores de uma folha, com a linha onde começam. */
function seletores(texto: string): { seletor: string; linha: number }[] {
  const limpo = semComentarios(texto);
  const achados: { seletor: string; linha: number }[] = [];
  for (const m of limpo.matchAll(/([^{};]+)\{/g)) {
    const inicio = m.index + m[1].length - m[1].trimStart().length;
    const linha = limpo.slice(0, inicio).split("\n").length;
    for (const s of m[1].split(",")) {
      const seletor = s.trim().replace(/\s+/g, " ");
      if (seletor && !seletor.startsWith("@")) achados.push({ seletor, linha });
    }
  }
  return achados;
}

const classesDe = (seletor: string) => [...seletor.matchAll(/\.(-?[_a-zA-Z][\w-]*)/g)].map((m) => m[1]);

const CLASSES = new Set(
  Object.entries(css)
    .filter(([chave]) => !deFora(chave) && chave.endsWith("componentes.css"))
    .flatMap(([, texto]) => seletores(texto).flatMap((s) => classesDe(s.seletor))),
);

describe("classes dos componentes compartilhados (#406)", () => {
  it("lê as classes de componentes.css", () => {
    expect(CLASSES.has("bloco-secao")).toBe(true);
    expect(CLASSES.has("bloco-cartao")).toBe(true);
    expect(CLASSES.has("selo")).toBe(true);
  });

  it("nenhuma outra folha de estilo usa essas classes como seletor", () => {
    const erros: string[] = [];
    for (const [chave, texto] of Object.entries(css)) {
      if (!deFora(chave)) continue;
      for (const { seletor, linha } of seletores(texto)) {
        if (PERMITIDOS.has(seletor)) continue;
        for (const c of classesDe(seletor)) {
          if (CLASSES.has(c)) erros.push(`${nomeDe(chave)}:${linha} .${c} em "${seletor}"`);
        }
      }
    }
    expect(erros).toEqual([]);
  });

  it("nenhuma outra marcação põe essas classes no className", () => {
    const erros: string[] = [];
    for (const [chave, texto] of Object.entries(tsx)) {
      if (!deFora(chave)) continue;
      texto.split("\n").forEach((linha, i) => {
        for (const m of linha.matchAll(/className=(.*)/g)) {
          for (const lit of m[1].matchAll(/"([^"]*)"|'([^']*)'|`([^`]*)`/g)) {
            const valor = (lit[1] ?? lit[2] ?? lit[3]).replace(/\$\{[^}]*\}/g, " ");
            for (const c of valor.split(/\s+/)) {
              if (CLASSES.has(c)) erros.push(`${nomeDe(chave)}:${i + 1} className "${c}"`);
            }
          }
        }
      });
    }
    expect(erros).toEqual([]);
  });
});
