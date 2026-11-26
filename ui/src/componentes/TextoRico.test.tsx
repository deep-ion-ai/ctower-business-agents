import { fireEvent, render, screen } from "@testing-library/react";
import { estimarLinhas, TextoRico } from "./TextoRico";
import { mesmoHost } from "./regraLinks";

describe("TextoRico (#406)", () => {
  it("renderiza o Markdown, sem marcadores crus nem HTML cru", () => {
    const { container } = render(
      <TextoRico texto={"Há **dois caminhos**:\n\n- **Acordo**: mantém\n- outro `código`\n\n<b>cru</b>"} />,
    );
    expect(screen.getByText("dois caminhos").tagName).toBe("STRONG");
    expect(container.querySelectorAll("li")).toHaveLength(2);
    expect(screen.getByText("código").tagName).toBe("CODE");
    expect(container).not.toHaveTextContent("**");
    expect(container.querySelector("b")).toBeNull();
    expect(container).toHaveTextContent("<b>cru</b>");
    expect(container.firstElementChild).toHaveClass("markdown", "texto-rico");
  });

  it("link do host permitido abre por aoAbrirLink; de outro host vira texto com o endereço", () => {
    const abrir = vi.fn();
    render(
      <TextoRico
        texto="Ver [a wiki](https://github.com/o/r/wiki) e [um site](https://exemplo.com/x)."
        hostPermitido="https://github.com/o/r/issues/1"
        aoAbrirLink={abrir}
      />,
    );
    fireEvent.click(screen.getByRole("link", { name: "a wiki" }));
    expect(abrir).toHaveBeenCalledWith("https://github.com/o/r/wiki");
    expect(screen.queryByRole("link", { name: "um site" })).not.toBeInTheDocument();
    expect(screen.getByText("um site")).toHaveAttribute("title", "https://exemplo.com/x");
  });

  it("sem host permitido, nenhum link externo abre", () => {
    render(<TextoRico texto="[a wiki](https://github.com/o/r/wiki)" />);
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(screen.getByText("a wiki")).toHaveClass("link-externo");
  });

  it("imagem não carrega: vira o link dela, com o texto alternativo", () => {
    const abrir = vi.fn();
    const { container } = render(
      <TextoRico
        texto={"![print da tela](https://github.com/o/r/a.png)\n\n![fora](https://exemplo.com/b.png)"}
        hostPermitido="github.com"
        aoAbrirLink={abrir}
      />,
    );
    expect(container.querySelector("img")).toBeNull();
    fireEvent.click(screen.getByRole("link", { name: "print da tela" }));
    expect(abrir).toHaveBeenCalledWith("https://github.com/o/r/a.png");
    expect(screen.getByText("fora")).toHaveAttribute("title", "https://exemplo.com/b.png");
  });

  it("overrides próprios trocam a regra padrão de links", () => {
    const Link = ({ children }: { children?: React.ReactNode }) => <em data-testid="proprio">{children}</em>;
    render(<TextoRico texto="[doc](outro.md)" overrides={{ a: { component: Link } }} />);
    expect(screen.getByTestId("proprio")).toHaveTextContent("doc");
  });

  it("mostra todas as notas de rodapé, mesmo coladas", () => {
    const { container } = render(<TextoRico texto={"Texto.[^1][^2]\n\n[^1]: um\n[^2]: dois"} />);
    expect(container).not.toHaveTextContent("[^");
    expect(screen.getByRole("link", { name: "1" })).toHaveAttribute("href", "#1");
    expect(screen.getByRole("link", { name: "2" })).toHaveAttribute("href", "#2");
  });

  it("com linhas, recolhe o texto longo com 'ver mais' e expande com aria-expanded", () => {
    const longo = Array.from({ length: 8 }, (_, i) => `Linha ${i + 1} do texto.`).join("\n\n");
    const { container } = render(<TextoRico texto={longo} linhas={4} />);
    const caixa = container.querySelector(".texto-rico")!;
    expect(caixa).toHaveClass("texto-rico-recolhido");
    expect(caixa).toHaveStyle({ "--texto-rico-linhas": "4" });
    // O texto inteiro fica no DOM (sem "…"): só a altura é recolhida.
    expect(container).toHaveTextContent("Linha 8 do texto.");
    expect(container).not.toHaveTextContent("…");
    const botao = screen.getByRole("button", { name: "ver mais" });
    expect(botao).toHaveAttribute("aria-expanded", "false");
    expect(botao).toHaveAttribute("aria-controls", caixa.id);
    fireEvent.click(botao);
    expect(screen.getByRole("button", { name: "ver menos" })).toHaveAttribute("aria-expanded", "true");
    expect(caixa).not.toHaveClass("texto-rico-recolhido");
  });

  it("texto curto não ganha 'ver mais'", () => {
    render(<TextoRico texto="Uma linha só." linhas={4} />);
    expect(screen.queryByRole("button", { name: "ver mais" })).not.toBeInTheDocument();
  });

  it("inline: sem parágrafo, dentro de um span", () => {
    const { container } = render(<TextoRico texto="um **título**" inline />);
    expect(container.firstElementChild!.tagName).toBe("SPAN");
    expect(container.querySelector("p")).toBeNull();
    expect(screen.getByText("título").tagName).toBe("STRONG");
  });

  it("estima as linhas pelas quebras e pelo comprimento", () => {
    expect(estimarLinhas("a\n\nb")).toBe(2);
    expect(estimarLinhas("x".repeat(200))).toBe(3);
  });

  it("mesmoHost aceita endereço ou host e recusa o que não é URL absoluta", () => {
    expect(mesmoHost("https://GitHub.com/a", "https://github.com/o/r")).toBe(true);
    expect(mesmoHost("https://github.com/a", "github.com")).toBe(true);
    expect(mesmoHost("https://exemplo.com/a", "github.com")).toBe(false);
    expect(mesmoHost("/relativo", "github.com")).toBe(false);
  });
});
