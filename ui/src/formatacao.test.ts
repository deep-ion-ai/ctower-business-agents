import { formatar, formatoDoAtalho, type Formato } from "./formatacao";

/** Aplica a edição como o editor faria e devolve o texto e a seleção resultantes. */
function aplicar(formato: Formato, valor: string, ini: number, fim = ini) {
  const e = formatar(formato, valor, ini, fim);
  const texto = valor.slice(0, e.inicio) + e.texto + valor.slice(e.fim);
  return { texto, sel: texto.slice(e.selInicio, e.selFim) };
}

describe("formatar", () => {
  it("negrito, itálico e código: envolvem a seleção", () => {
    expect(aplicar("negrito", "uma palavra", 4, 11)).toEqual({
      texto: "uma **palavra**",
      sel: "palavra",
    });
    expect(aplicar("italico", "uma palavra", 4, 11).texto).toBe("uma *palavra*");
    expect(aplicar("codigo", "uma palavra", 4, 11).texto).toBe("uma `palavra`");
  });

  it("negrito, itálico e código: sem seleção inserem o modelo selecionado", () => {
    expect(aplicar("negrito", "ab", 1)).toEqual({
      texto: "a**texto em negrito**b",
      sel: "texto em negrito",
    });
    expect(aplicar("italico", "", 0)).toEqual({ texto: "*texto em itálico*", sel: "texto em itálico" });
    expect(aplicar("codigo", "", 0)).toEqual({ texto: "`código`", sel: "código" });
  });

  it("negrito desfaz quando a seleção já está marcada", () => {
    expect(aplicar("negrito", "uma **palavra**", 4, 15)).toEqual({ texto: "uma palavra", sel: "palavra" });
    expect(aplicar("negrito", "uma **palavra**", 6, 13).texto).toBe("uma palavra");
  });

  it("itálico dentro de negrito não é confundido com ele", () => {
    expect(aplicar("italico", "**palavra**", 2, 9).texto).toBe("***palavra***");
  });

  it("título: liga, desliga e acrescenta ao texto da linha", () => {
    expect(aplicar("titulo", "Intro", 2)).toEqual({ texto: "## Intro", sel: "Intro" });
    expect(aplicar("titulo", "## Intro", 3).texto).toBe("Intro");
    expect(aplicar("titulo", "# Intro", 3).texto).toBe("## Intro");
    expect(aplicar("titulo", "", 0)).toEqual({ texto: "## Título", sel: "Título" });
    expect(aplicar("titulo", "a\n\nb", 4).texto).toBe("a\n\n## b");
  });

  it("lista: prefixa cada linha tocada pela seleção e desliga", () => {
    expect(aplicar("lista", "um\ndois\ntres", 1, 6).texto).toBe("- um\n- dois\ntres");
    expect(aplicar("lista", "- um\n- dois", 0, 11).texto).toBe("um\ndois");
    expect(aplicar("lista", "", 0)).toEqual({ texto: "- item", sel: "item" });
    expect(aplicar("lista", "x\n\ny", 0, 4).texto).toBe("- x\n\n- y");
  });

  it("lista numerada: numera e troca o marcador de lista", () => {
    expect(aplicar("listaNumerada", "um\ndois", 0, 7).texto).toBe("1. um\n2. dois");
    expect(aplicar("listaNumerada", "1. um\n2. dois", 0, 13).texto).toBe("um\ndois");
    expect(aplicar("listaNumerada", "", 0)).toEqual({ texto: "1. item", sel: "item" });
  });

  it("citação: prefixa as linhas e desliga", () => {
    expect(aplicar("citacao", "a\nb", 0, 3).texto).toBe("> a\n> b");
    expect(aplicar("citacao", "> a\n> b", 0, 7).texto).toBe("a\nb");
    expect(aplicar("citacao", "", 0)).toEqual({ texto: "> citação", sel: "citação" });
  });

  it("seleção que termina depois da quebra não toca a linha seguinte", () => {
    expect(aplicar("lista", "a\nb", 0, 2).texto).toBe("- a\nb");
  });

  it("link: seleção vira o texto e seleciona o endereço", () => {
    expect(aplicar("link", "veja aqui", 5, 9)).toEqual({
      texto: "veja [aqui](https://)",
      sel: "https://",
    });
    expect(aplicar("link", "", 0)).toEqual({
      texto: "[texto do link](https://)",
      sel: "texto do link",
    });
    expect(aplicar("link", "https://a.com", 0, 13)).toEqual({
      texto: "[texto do link](https://a.com)",
      sel: "texto do link",
    });
  });

  it("bloco de código: em linhas próprias", () => {
    expect(aplicar("blocoCodigo", "", 0)).toEqual({ texto: "```\ncódigo\n```", sel: "código" });
    expect(aplicar("blocoCodigo", "a b c", 2, 3).texto).toBe("a \n```\nb\n```\n c");
    expect(aplicar("blocoCodigo", "x\ny\n", 2, 3).texto).toBe("x\n```\ny\n```\n");
  });

  it("tabela: insere o modelo; a seleção vira o título da primeira coluna", () => {
    expect(aplicar("tabela", "", 0)).toEqual({
      texto: "| Coluna 1 | Coluna 2 |\n| --- | --- |\n| valor | valor |",
      sel: "Coluna 1",
    });
    expect(aplicar("tabela", "texto", 5).texto.startsWith("texto\n| Coluna 1")).toBe(true);
    expect(aplicar("tabela", "Nome", 0, 4)).toMatchObject({ sel: "Nome" });
  });
});

describe("formatoDoAtalho", () => {
  const base = { ctrlKey: false, metaKey: false, shiftKey: false, altKey: false };
  it("reconhece Ctrl e Cmd com B, I e K", () => {
    expect(formatoDoAtalho({ ...base, ctrlKey: true, key: "b" })).toBe("negrito");
    expect(formatoDoAtalho({ ...base, metaKey: true, key: "I" })).toBe("italico");
    expect(formatoDoAtalho({ ...base, ctrlKey: true, key: "k" })).toBe("link");
  });
  it("ignora sem modificador, com Shift/Alt ou outra tecla", () => {
    expect(formatoDoAtalho({ ...base, key: "b" })).toBeNull();
    expect(formatoDoAtalho({ ...base, ctrlKey: true, shiftKey: true, key: "b" })).toBeNull();
    expect(formatoDoAtalho({ ...base, ctrlKey: true, altKey: true, key: "b" })).toBeNull();
    expect(formatoDoAtalho({ ...base, ctrlKey: true, key: "s" })).toBeNull();
  });
});
