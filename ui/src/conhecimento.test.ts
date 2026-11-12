import { partesDoTrecho, resolverLink } from "./conhecimento";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

describe("resolverLink", () => {
  const regra = "faturamento-exemplo/regras/RN-COB-001.md";

  it("resolve links relativos ao documento e à raiz da base", () => {
    expect(resolverLink(regra, "../analises/A-0001.md")).toBe(
      "faturamento-exemplo/analises/A-0001.md",
    );
    expect(resolverLink(regra, "./RN-COB-002.md")).toBe("faturamento-exemplo/regras/RN-COB-002.md");
    expect(resolverLink(regra, "RN-COB-002.md#evidencias")).toBe(
      "faturamento-exemplo/regras/RN-COB-002.md",
    );
    expect(resolverLink(regra, "/outro/sistema.md")).toBe("outro/sistema.md");
    expect(resolverLink(regra, "../glossario/car%C3%AAncia.md")).toBe(
      "faturamento-exemplo/glossario/carência.md",
    );
  });

  it("recusa o que não é documento da base", () => {
    expect(resolverLink(regra, "https://exemplo.invalido/a.md")).toBeNull();
    expect(resolverLink(regra, "javascript:alert(1)")).toBeNull();
    expect(resolverLink(regra, "file:///etc/passwd.md")).toBeNull();
    expect(resolverLink(regra, "//servidor/a.md")).toBeNull();
    expect(resolverLink(regra, "../../../fora.md")).toBeNull();
    expect(resolverLink(regra, "../codigo.sql")).toBeNull();
    expect(resolverLink(regra, "#nota-1")).toBeNull();
    expect(resolverLink(regra, "%E0%A4%A.md")).toBeNull();
  });
});

describe("partesDoTrecho", () => {
  it("separa as palavras destacadas", () => {
    expect(partesDoTrecho("a **multa** é de **2%**")).toEqual([
      { texto: "a ", destaque: false },
      { texto: "multa", destaque: true },
      { texto: " é de ", destaque: false },
      { texto: "2%", destaque: true },
    ]);
    expect(partesDoTrecho("")).toEqual([]);
  });
});
