import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { TerminalTeste } from "./TerminalTeste";
import { criarTelaXterm } from "./telaXterm";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: vi.fn(function (this: { onmessage?: unknown }) {
    this.onmessage = undefined;
  }),
}));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

const mockInvoke = vi.mocked(invoke);

function telaFalsa() {
  return {
    abrirEm: vi.fn(),
    escrever: vi.fn(),
    aoDigitar: vi.fn(),
    ajustar: vi.fn(() => ({ linhas: 30, colunas: 100 })),
    limpar: vi.fn(),
    focar: vi.fn(),
    descartar: vi.fn(),
  };
}

function responder(ultima: unknown) {
  mockInvoke.mockImplementation((comando: string) => {
    switch (comando) {
      case "listar_sistemas":
        return Promise.resolve([{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }]);
      case "ultima_sessao_terminal":
        return Promise.resolve(ultima);
      case "abrir_terminal_claude":
        return Promise.resolve(1);
      default:
        return Promise.resolve();
    }
  });
}

describe("TerminalTeste", () => {
  beforeEach(() => {
    vi.mocked(criarTelaXterm).mockImplementation(telaFalsa);
  });

  it("abre o terminal do sistema escolhido com o prompt inicial", async () => {
    responder(null);
    render(<TerminalTeste />);
    expect(await screen.findByRole("option", { name: "Faturamento Exemplo" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText(/Prompt inicial/), {
      target: { value: "Inicie a análise pelo ctower" },
    });
    expect(screen.queryByRole("button", { name: /Retomar/ })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Abrir no terminal" }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "abrir_terminal_claude",
        expect.objectContaining({
          sistema: "faturamento-exemplo",
          pedido: { promptInicial: "Inicie a análise pelo ctower" },
          tamanho: { linhas: 30, colunas: 100 },
        }),
      ),
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Continuar no Claude Desktop" })).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "Continuar no Claude Desktop" }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("enviar_prompt_terminal", {
        id: 1,
        texto: "/desktop",
      }),
    );
  });

  it("oferece retomar a última sessão do sistema", async () => {
    responder({ idSessao: "abc-123", abertaEm: 1794427000 });
    render(<TerminalTeste />);
    fireEvent.click(await screen.findByRole("button", { name: /Retomar a sessão anterior/ }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "abrir_terminal_claude",
        expect.objectContaining({ pedido: { retomada: { modo: "sessao", id: "abc-123" } } }),
      ),
    );
  });
});
