import { render, screen } from "@testing-library/react";
import { CorpoMarkdown } from "./DocumentoBase";
import { separarNotasSeguidas } from "./notasDeRodape";

describe("notas de rodapé (#220)", () => {
  it("separa notas coladas ou só com espaço, sem mexer em código", () => {
    expect(separarNotasSeguidas("a.[^1][^2][^3]")).toBe("a.[^1]​[^2]​[^3]");
    expect(separarNotasSeguidas("a.[^1] [^2]")).toBe("a.[^1]​ [^2]");
    expect(separarNotasSeguidas("a[^1] e b[^2]")).toBe("a[^1] e b[^2]");
    expect(separarNotasSeguidas("`x[^1][^2]`")).toBe("`x[^1][^2]`");
    expect(separarNotasSeguidas("```\n[^1][^2]\n```")).toBe("```\n[^1][^2]\n```");
    expect(separarNotasSeguidas("[^1]: texto")).toBe("[^1]: texto");
  });

  it("renderiza todas as notas do enunciado como links, sem texto cru", () => {
    const md = [
      "Sem multa na carência.[^1][^2][^3]",
      "",
      "[^1]: `a.sql:1-2`",
      "[^2]: `b.sql:3`",
      "[^3]: `c.sql:4`",
    ].join("\n");
    const { container } = render(<CorpoMarkdown markdown={md} caminho="x/regras/R.md" />);
    expect(container).not.toHaveTextContent("[^");
    for (const n of ["1", "2", "3"]) {
      expect(screen.getByRole("link", { name: n })).toHaveAttribute("href", `#${n}`);
    }
    expect(container.querySelector("footer")).toHaveTextContent("a.sql:1-2");
    expect(container.querySelector("footer")).toHaveTextContent("c.sql:4");
  });
});
