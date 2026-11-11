import { render, screen } from "@testing-library/react";
import { App, PRODUTO } from "./App";

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn(() => Promise.resolve("1.2.3")),
}));

// O estado dos clientes tem testes próprios (ClientesIA.test.tsx).
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.reject(new Error("fora do Tauri"))),
}));

// O xterm.js precisa de canvas, que o jsdom não tem; o terminal tem testes próprios.
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

describe("App", () => {
  it("mostra o nome do produto e a versão do app", async () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: PRODUTO })).toBeInTheDocument();
    expect(await screen.findByText("Versão 1.2.3")).toBeInTheDocument();
  });

  it("mostra a seção dos clientes de IA", async () => {
    render(<App />);
    expect(await screen.findByRole("region", { name: "Clientes de IA" })).toBeInTheDocument();
  });
});
