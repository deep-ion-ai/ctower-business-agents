import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { App, PRODUTO } from "./App";
import { apiPrimeiroUsoFalsa, situacaoInicial } from "./test/falsos";

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn(() => Promise.resolve("1.2.3")),
}));

// O estado dos clientes e a tela da análise têm testes próprios.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.reject(new Error("fora do Tauri"))),
  Channel: vi.fn(),
}));

// O xterm.js precisa de canvas, que o jsdom não tem; o terminal tem testes próprios.
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

function concluido() {
  const api = apiPrimeiroUsoFalsa();
  api.situacao.mockResolvedValue(
    situacaoInicial({
      base: "/base",
      sistemas: [{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }],
      cliente: "terminalEmbutido",
      primeiroUsoConcluido: true,
    }),
  );
  return api;
}

describe("App", () => {
  it("mostra o nome do produto e a versão do app", async () => {
    render(<App api={concluido()} />);
    expect(screen.getByRole("heading", { name: PRODUTO })).toBeInTheDocument();
    expect(await screen.findByText("Versão 1.2.3")).toBeInTheDocument();
  });

  it("depois do primeiro uso, abre o início com os clientes de IA e a navegação", async () => {
    render(<App api={concluido()} />);
    expect(await screen.findByRole("region", { name: "Início" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Clientes de IA" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Primeiro uso" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Análises" }));
    expect(await screen.findByRole("region", { name: "Pergunta ao sistema" })).toBeInTheDocument();
  });

  it("no primeiro uso, abre o assistente e, ao concluir, vai para a análise", async () => {
    const api = apiPrimeiroUsoFalsa();
    api.situacao.mockResolvedValueOnce(situacaoInicial()).mockResolvedValue(
      situacaoInicial({
        base: "/base",
        sistemas: [{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }],
        primeiroUsoConcluido: true,
      }),
    );
    render(<App api={api} />);
    expect(await screen.findByRole("region", { name: "Primeiro uso" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Pergunta ao sistema" })).not.toBeInTheDocument();

    fireEvent.click(await screen.findByRole("button", { name: "Continuar" }));
    await screen.findByText(/o app vai criá-la/);
    fireEvent.click(screen.getByRole("button", { name: "Usar esta pasta" }));
    fireEvent.change(await screen.findByLabelText("Nome do sistema"), {
      target: { value: "Faturamento Exemplo" },
    });
    fireEvent.change(screen.getByLabelText(/Pasta principal/), {
      target: { value: "/codigo/faturamento-exemplo" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Ver o que será analisado" }));
    await screen.findByRole("region", { name: "Prévia do inventário" });
    fireEvent.click(screen.getByRole("button", { name: "Cadastrar o sistema" }));
    await screen.findByText(/O ambiente do Claude está preparado/);
    fireEvent.click(screen.getByRole("button", { name: "Continuar" }));
    fireEvent.click(await screen.findByRole("button", { name: "Fazer a primeira análise" }));

    expect(await screen.findByRole("region", { name: "Pergunta ao sistema" })).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Primeiro uso" })).not.toBeInTheDocument(),
    );
    expect(api.concluir).toHaveBeenCalled();

    // Cada passo pode ser refeito.
    fireEvent.click(screen.getByRole("button", { name: "Refazer o primeiro uso" }));
    expect(await screen.findByRole("region", { name: "Primeiro uso" })).toBeInTheDocument();
  });
});
