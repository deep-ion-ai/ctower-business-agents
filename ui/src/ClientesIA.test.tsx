import { fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { ClientesIA } from "./ClientesIA";
import type { EstadoClientes } from "./clientes";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const mockInvoke = vi.mocked(invoke);

const INSTALAR_CODE = {
  texto: "Abra um terminal e rode o instalador oficial da Anthropic:",
  comando: "curl -fsSL https://claude.ai/install.sh | bash",
};
const INSTALAR_DESKTOP = {
  texto: "Baixe e instale o Claude Desktop pela página oficial:",
  link: "https://claude.ai/download",
};

function estado(parcial: Partial<EstadoClientes>): EstadoClientes {
  return {
    plataforma: "macOs",
    claudeCode: {
      instalacao: {
        estado: "encontrado",
        caminho: "/Users/analista/.local/bin/claude",
        origem: "path",
        versao: "2.1.0",
      },
      login: { estado: "logado", metodo: "claude.ai", provedor: "firstParty" },
      orientacao: [],
    },
    claudeDesktop: {
      instalacao: { estado: "encontrado", caminho: "/Applications/Claude.app" },
      orientacao: [{ texto: "Confira no próprio Claude Desktop se você entrou." }],
    },
    ...parcial,
  };
}

async function cartao(nome: string) {
  return within(await screen.findByRole("article", { name: nome }));
}

describe("ClientesIA", () => {
  // Sem mockReset/mockClear num beforeEach: com eles, o Vitest 3.2 acusa como não
  // tratada a promessa rejeitada do último teste. Cada teste define a sua resposta.

  it("mostra os dois clientes instalados e o Claude Code logado", async () => {
    mockInvoke.mockResolvedValue(estado({}));
    render(<ClientesIA />);
    const code = await cartao("Claude Code");
    expect(code.getByText("Instalado, versão 2.1.0")).toBeInTheDocument();
    expect(code.getByText("Logado")).toBeInTheDocument();
    expect(code.queryByRole("list")).not.toBeInTheDocument();
    const desktop = await cartao("Claude Desktop");
    expect(desktop.getByText("Instalado")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("estado_clientes");
  });

  it("mostra o passo a passo quando nenhum cliente está instalado", async () => {
    mockInvoke.mockResolvedValue(
      estado({
        claudeCode: {
          instalacao: { estado: "naoEncontrado" },
          login: { estado: "naoVerificado" },
          orientacao: [INSTALAR_CODE],
        },
        claudeDesktop: {
          instalacao: { estado: "naoEncontrado" },
          orientacao: [INSTALAR_DESKTOP],
        },
      }),
    );
    render(<ClientesIA />);
    const code = await cartao("Claude Code");
    expect(code.getByText("Não instalado")).toBeInTheDocument();
    expect(code.getByText(INSTALAR_CODE.comando)).toBeInTheDocument();
    const desktop = await cartao("Claude Desktop");
    expect(desktop.getByText("Não instalado")).toBeInTheDocument();
    expect(desktop.getByText(INSTALAR_DESKTOP.link)).toBeInTheDocument();
  });

  it("mostra o Claude Code instalado sem login", async () => {
    mockInvoke.mockResolvedValue(
      estado({
        claudeCode: {
          instalacao: {
            estado: "encontrado",
            caminho: "C:\\Users\\analista\\.local\\bin\\claude.exe",
            origem: "localPadrao",
            versao: null,
          },
          login: { estado: "naoLogado" },
          orientacao: [{ texto: "Rode o Claude Code e faça login:", comando: "claude" }],
        },
      }),
    );
    render(<ClientesIA />);
    const code = await cartao("Claude Code");
    expect(code.getByText("Instalado")).toBeInTheDocument();
    expect(code.getByText("Sem login")).toBeInTheDocument();
    expect(code.getByText("claude")).toBeInTheDocument();
  });

  it("diz quando o login não pôde ser confirmado", async () => {
    mockInvoke.mockResolvedValue(
      estado({
        claudeCode: {
          ...estado({}).claudeCode,
          login: { estado: "indeterminado", motivo: "sem resposta" },
        },
      }),
    );
    render(<ClientesIA />);
    expect((await cartao("Claude Code")).getByText("Login não confirmado")).toBeInTheDocument();
  });

  it("no Linux, o Claude Desktop aparece como não disponível", async () => {
    mockInvoke.mockResolvedValue(
      estado({
        plataforma: "linux",
        claudeDesktop: { instalacao: { estado: "naoSuportado" }, orientacao: [] },
      }),
    );
    render(<ClientesIA />);
    expect(
      (await cartao("Claude Desktop")).getByText(/Não disponível neste projeto/),
    ).toBeInTheDocument();
  });

  it("verifica de novo ao clicar no botão", async () => {
    mockInvoke.mockResolvedValueOnce(
      estado({
        claudeDesktop: { instalacao: { estado: "naoEncontrado" }, orientacao: [INSTALAR_DESKTOP] },
      }),
    );
    mockInvoke.mockResolvedValueOnce(estado({}));
    const antes = mockInvoke.mock.calls.length;
    render(<ClientesIA />);
    expect((await cartao("Claude Desktop")).getByText("Não instalado")).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Verificar de novo" }));
    expect(await (await cartao("Claude Desktop")).findByText("Instalado")).toBeInTheDocument();
    expect(mockInvoke.mock.calls.length - antes).toBe(2);
  });

  it("avisa quando a verificação falha", async () => {
    mockInvoke.mockImplementation(() => Promise.reject(new Error("fora do Tauri")));
    render(<ClientesIA />);
    expect(
      await screen.findByText("Não foi possível verificar os clientes de IA."),
    ).toBeInTheDocument();
  });
});
