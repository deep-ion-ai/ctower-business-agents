import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { Navegacao } from "./Navegacao";
import { REGRA, SISTEMA, apiConhecimentoFalsa } from "./test/conhecimentoFalso";
import { apiAnaliseFalsa, clientesProntos } from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.reject(new Error("fora do Tauri"))),
  Channel: vi.fn(),
}));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));
vi.mock("./clientes", () => ({ estadoClientes: vi.fn(() => Promise.resolve(clientesProntos())) }));

function montar() {
  const conhecimento = apiConhecimentoFalsa();
  const analise = apiAnaliseFalsa();
  render(<Navegacao api={conhecimento.api} apiAnalise={analise.api} />);
  return { conhecimento, analise };
}

describe("Navegacao", () => {
  it("passa pelo início, pelo sistema, pela base e pelas análises", async () => {
    montar();
    const abas = screen.getByRole("navigation", { name: "Telas" });
    expect(await screen.findByRole("region", { name: "Início" })).toBeInTheDocument();
    expect(within(abas).getByRole("button", { name: "Início" })).toHaveAttribute(
      "aria-current",
      "page",
    );

    fireEvent.click(within(abas).getByRole("button", { name: "Sistema" }));
    expect(await screen.findByRole("region", { name: "Regras" })).toBeInTheDocument();

    fireEvent.click(within(abas).getByRole("button", { name: "Base de conhecimento" }));
    expect(screen.getByRole("search", { name: "Busca na base" })).toBeInTheDocument();

    fireEvent.click(within(abas).getByRole("button", { name: "Análises" }));
    expect(await screen.findByRole("region", { name: "Pergunta ao sistema" })).toBeInTheDocument();
  });

  it("do sistema para o documento na base", async () => {
    const { conhecimento } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Faturamento Exemplo" }));
    const regras = await screen.findByRole("region", { name: "Regras" });
    fireEvent.click(within(regras).getByRole("button", { name: /RN-COB-001/ }));
    expect(
      await screen.findByRole("article", { name: /Passada a carência/ }),
    ).toBeInTheDocument();
    expect(conhecimento.api.abrirDocumento).toHaveBeenCalledWith(REGRA);
  });

  it("abre a tela da análise a partir de uma análise da lista do início", async () => {
    const { analise } = montar();
    const recentes = await screen.findByRole("region", { name: "Recentes" });
    fireEvent.click(within(recentes).getByRole("button", { name: "A-0001: Multa por atraso" }));
    expect(await screen.findByRole("region", { name: "Análise" })).toBeInTheDocument();
    await waitFor(() =>
      expect(analise.api.carregarAnalise).toHaveBeenCalledWith(SISTEMA, "A-0001"),
    );
  });
});
