import { fireEvent, render, screen } from "@testing-library/react";
import { comCodigoMarcado, segmentar, TextoAfirmacao } from "./TextoAfirmacao";

const TEXTO =
  'Trecho lido no pacote: montarGruposTipos(e){...return Jae.filter(i=>!i.interno||!this.agenteExterno)...}, com Jae=[{label:"Alvarás",interno:!0},{label:"Certidões"}]. (confirmada)';

describe("TextoAfirmacao", () => {
  it("separa o código entre crases e o código cru da prosa", () => {
    expect(segmentar("usa `calcularMulta()` no fim")).toEqual([
      { tipo: "texto", valor: "usa " },
      { tipo: "codigo", valor: "calcularMulta()" },
      { tipo: "texto", valor: " no fim" },
    ]);
    const s = segmentar(TEXTO);
    expect(s.filter((x) => x.tipo === "codigo").map((x) => x.valor)).toEqual([
      "montarGruposTipos(e){...return Jae.filter(i=>!i.interno||!this.agenteExterno)...}",
      'Jae=[{label:"Alvarás",interno:!0},{label:"Certidões"}]',
    ]);
    expect(s.map((x) => x.valor).join("")).not.toContain("``");
  });

  it("deixa a prosa com colchetes curtos como está", () => {
    expect(segmentar("ver [nota] e {x}")).toEqual([{ tipo: "texto", valor: "ver [nota] e {x}" }]);
  });

  it("recolhe o código longo atrás de 'ver código'", () => {
    render(<TextoAfirmacao texto={TEXTO} />);
    expect(screen.queryByText(/agenteExterno/)).not.toBeInTheDocument();
    const [ver] = screen.getAllByRole("button", { name: "ver código" });
    fireEvent.click(ver);
    expect(ver).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText(/agenteExterno/)).toBeVisible();
    expect(screen.getByText(/\(confirmada\)/)).toBeInTheDocument();
  });

  it("mostra o Markdown simples: lista, negrito e quebra em parágrafos", () => {
    const { container } = render(
      <TextoAfirmacao
        texto={"O BNMP tem **12 perfis**:\n\n- Administrador BNMP 2.0\n- Juiz Instrutor\n\nO resto vem do SCA."}
      />,
    );
    expect(screen.getByText("12 perfis").tagName).toBe("STRONG");
    expect(screen.getAllByRole("listitem").map((li) => li.textContent)).toEqual([
      "Administrador BNMP 2.0",
      "Juiz Instrutor",
    ]);
    expect(container.querySelectorAll(".texto > p")).toHaveLength(2);
  });

  it("não interpreta HTML, nem links, e título vira destaque", () => {
    const { container } = render(
      <TextoAfirmacao texto={"# Regra\n\nVer <b>isto</b> em [docs](https://x.y)."} />,
    );
    expect(container.querySelector("h1")).toBeNull();
    expect(screen.getByText("Regra")).toHaveClass("destaque-afirmacao");
    expect(container.querySelector("b")).toBeNull();
    expect(container.querySelector("a")).toBeNull();
    expect(screen.getByText("docs")).toHaveClass("link-externo");
  });

  it("recolhe o bloco de código cercado e não o marca de novo", () => {
    const fonte = "Antes:\n\n```\nconst x = {a: [1, 2, 3], b: {c: 'longo o bastante'}};\n```";
    expect(comCodigoMarcado(fonte)).toBe(fonte);
    render(<TextoAfirmacao texto={fonte} />);
    expect(screen.queryByText(/const x/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "ver código" }));
    expect(screen.getByText(/const x/)).toBeVisible();
    expect(screen.getAllByRole("button", { name: "ocultar código" })).toHaveLength(1);
  });

  it("mantém o sublinhado no meio dos nomes, sem itálico", () => {
    const { container } = render(
      <TextoAfirmacao texto="Marcas BNMP_EXTERNO_INTERACAO, _CONSULTA ou _MP; _ênfase_ continua." />,
    );
    expect(container.textContent).toContain("BNMP_EXTERNO_INTERACAO, _CONSULTA ou _MP");
    expect(container.querySelectorAll("em")).toHaveLength(1);
    expect(container.querySelector("em")).toHaveTextContent("ênfase");
  });
});
