import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { App, PRODUTO } from "./App";
import { criarTelaXterm } from "./telaXterm";
import { apiAnaliseFalsa, apiPrimeiroUsoFalsa, situacaoInicial, telaFalsa } from "./test/falsos";

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
      projetos: [{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }],
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

  it("depois do primeiro uso, abre a navegação por projeto, com Atividades e Análises no menu", async () => {
    render(<App api={concluido()} />);
    const menu = await screen.findByRole("navigation", { name: "Seções do projeto" });
    expect(screen.queryByRole("region", { name: "Primeiro uso" })).not.toBeInTheDocument();
    fireEvent.click(within(menu).getByRole("button", { name: /^Atividades/ }));
    expect(await screen.findByRole("region", { name: "Início" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Clientes de IA" })).toBeInTheDocument();
    fireEvent.click(within(menu).getByRole("button", { name: /^Análises/ }));
    expect(await screen.findByRole("region", { name: "Análises" })).toBeInTheDocument();
  });

  it("no primeiro uso, abre o assistente e, ao concluir, inicia a primeira análise (#227)", async () => {
    const api = apiPrimeiroUsoFalsa();
    const analise = apiAnaliseFalsa();
    // A análise abre o terminal do agente.
    vi.mocked(criarTelaXterm).mockReturnValue(telaFalsa());
    api.situacao.mockResolvedValueOnce(situacaoInicial()).mockResolvedValue(
      situacaoInicial({
        base: "/base",
        projetos: [{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }],
        primeiroUsoConcluido: true,
      }),
    );
    render(<App api={api} apiAnalise={analise.api} />);
    expect(await screen.findByRole("region", { name: "Primeiro uso" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Análises" })).not.toBeInTheDocument();

    fireEvent.click(await screen.findByRole("button", { name: "Continuar" }));
    await screen.findByText(/o app vai criá-la/);
    fireEvent.click(screen.getByRole("button", { name: "Usar esta pasta" }));
    fireEvent.click(await screen.findByRole("button", { name: /Escolher a pasta do código/ }));
    await screen.findByRole("button", { name: "Trocar a pasta do código" });
    fireEvent.click(screen.getByRole("button", { name: "Continuar" }));
    await screen.findByRole("region", { name: "Prévia do inventário" });
    fireEvent.click(screen.getByRole("button", { name: "Continuar" }));
    await screen.findByRole("heading", { name: "Conectar as demandas (opcional)" });
    fireEvent.click(screen.getByRole("button", { name: "Continuar" }));
    fireEvent.click(await screen.findByRole("button", { name: "Cadastrar o projeto" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Ligar o agente a este projeto" }),
    );
    await screen.findByText(/O ambiente do Claude está preparado/);
    fireEvent.click(screen.getByRole("button", { name: "Continuar" }));
    fireEvent.click(await screen.findByRole("button", { name: "Fazer a primeira análise" }));

    // A análise é criada e aberta: não sobra a tela da pergunta para o analista preencher.
    expect(await screen.findByRole("tab", { name: "A-0001" })).toBeInTheDocument();
    expect(analise.api.criarAnalise).toHaveBeenCalledWith(
      "faturamento-exemplo",
      expect.stringContaining("regras de negócio"),
    );
    expect(screen.queryByRole("region", { name: "Análises" })).not.toBeInTheDocument();
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Primeiro uso" })).not.toBeInTheDocument(),
    );
    expect(api.concluir).toHaveBeenCalled();

    // Cada passo pode ser refeito.
    fireEvent.click(screen.getByRole("button", { name: "Refazer o primeiro uso" }));
    expect(await screen.findByRole("region", { name: "Primeiro uso" })).toBeInTheDocument();
  });

  it("mostra uma vez o aviso da migração das configurações para as pastas", async () => {
    const apiMigracao = {
      migrar: vi.fn(() =>
        Promise.resolve({
          mensagem:
            "As configurações do CBA no Claude Code agora valem só nas pastas dos projetos cadastrados",
          projetos: ["faturamento-exemplo"],
          arquivosLocais: ["/codigo/faturamento/.claude/settings.local.json"],
          settings: "/home/ana/.claude/settings.json",
          eventosRetirados: ["SessionStart", "Stop"],
          regrasRetiradas: [],
          ficaram: ["PreToolUse"],
          pendentes: [],
        }),
      ),
      marcarVisto: vi.fn(() => Promise.resolve()),
    };
    render(<App api={concluido()} apiMigracao={apiMigracao} />);
    const aviso = await screen.findByRole("status", { name: "Configurações do Claude Code" });
    expect(aviso).toHaveTextContent("agora valem só nas pastas dos projetos cadastrados");
    expect(aviso).toHaveTextContent("/codigo/faturamento/.claude/settings.local.json");
    expect(aviso).toHaveTextContent("SessionStart, Stop");
    expect(aviso).toHaveTextContent("foram mudados depois do CBA, os hooks de: PreToolUse");
    fireEvent.click(screen.getByRole("button", { name: "Entendi" }));
    await waitFor(() => expect(apiMigracao.marcarVisto).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("status", { name: "Configurações do Claude Code" })).toBeNull();
  });

  it("sem migração a fazer, não mostra aviso", async () => {
    const apiMigracao = {
      migrar: vi.fn(() => Promise.resolve(null)),
      marcarVisto: vi.fn(() => Promise.resolve()),
    };
    render(<App api={concluido()} apiMigracao={apiMigracao} />);
    await waitFor(() => expect(apiMigracao.migrar).toHaveBeenCalled());
    expect(screen.queryByRole("status", { name: "Configurações do Claude Code" })).toBeNull();
  });
});
