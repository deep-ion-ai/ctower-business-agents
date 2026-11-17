import { render, screen } from "@testing-library/react";
import { ErroTecnico, traduzirErro } from "./erroTecnico";
import { rotuloDaEvidencia } from "./analise";

describe("ErroTecnico", () => {
  it("traduz o erro do git e recolhe o original", () => {
    const cru = "fatal: repository 'https://git.exemplo.com/faturamento-exemplo.git/' not found";
    const { container } = render(<ErroTecnico mensagem={cru} />);
    expect(screen.getByText(/Repositório não encontrado/)).toBeTruthy();
    expect(container.querySelector("details")?.textContent).toContain(cru);
    expect(container.querySelector("details")?.hasAttribute("open")).toBe(false);
  });
  it("traduz o erro do parser", () => {
    expect(traduzirErro("found unexpected end of stream")).toMatch(/mal formada/);
  });
  it("deixa mensagens desconhecidas como estão", () => {
    const { container } = render(<ErroTecnico mensagem="Token inválido." />);
    expect(container.textContent).toBe("Token inválido.");
  });
});

describe("rotuloDaEvidencia", () => {
  it("mostra Arquivo.java:94-98", () => {
    expect(
      rotuloDaEvidencia({ arquivo: "src/main/java/exemplo/faturamento/Fatura.java", linhas: "94-98" }),
    ).toBe("Fatura.java:94-98");
  });
});
