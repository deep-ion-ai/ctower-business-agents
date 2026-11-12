import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { Navegacao } from "./Navegacao";
import { REGRA, SISTEMA, apiConhecimentoFalsa } from "./test/conhecimentoFalso";
import { apiConfiguracoesFalsa } from "./test/configuracoesFalso";
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

  it("abre as configurações e relê os sistemas quando elas mudam", async () => {
    const conhecimento = apiConhecimentoFalsa();
    const configuracoes = apiConfiguracoesFalsa();
    render(
      <Navegacao
        api={conhecimento.api}
        apiAnalise={apiAnaliseFalsa().api}
        apiConfiguracoes={configuracoes}
      />,
    );
    await screen.findByRole("region", { name: "Início" });
    const abas = screen.getByRole("navigation", { name: "Telas" });
    fireEvent.click(within(abas).getByRole("button", { name: "Configurações" }));
    expect(await screen.findByRole("region", { name: "Configurações" })).toBeInTheDocument();
    const lidas = conhecimento.api.situacao.mock.calls.length;
    fireEvent.click(await screen.findByRole("button", { name: "Descadastrar o sistema" }));
    fireEvent.click(screen.getByRole("checkbox", { name: /Confirmo o descadastro/ }));
    fireEvent.click(screen.getByRole("button", { name: "Descadastrar" }));
    await waitFor(() =>
      expect(conhecimento.api.situacao.mock.calls.length).toBeGreaterThan(lidas),
    );
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

  it("abre cada objeto numa aba e fecha a aba ativa", async () => {
    montar();
    await screen.findByRole("region", { name: "Início" });
    const abas = screen.getByRole("tablist", { name: "Abas abertas" });
    const inicio = within(screen.getByRole("region", { name: "Início" }));
    fireEvent.click(inicio.getByRole("button", { name: "Faturamento Exemplo" }));
    await screen.findByRole("region", { name: "Regras" });

    expect(within(abas).getAllByRole("tab")).toHaveLength(2);
    expect(within(abas).getByRole("tab", { name: "Faturamento Exemplo" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    // Voltar a uma aba não abre outra.
    fireEvent.click(within(abas).getByRole("tab", { name: "Início" }));
    expect(await screen.findByRole("region", { name: "Início" })).toBeInTheDocument();
    expect(within(abas).getAllByRole("tab")).toHaveLength(2);

    fireEvent.click(within(abas).getByRole("button", { name: "Fechar Início" }));
    expect(within(abas).getAllByRole("tab")).toHaveLength(1);
    expect(await screen.findByRole("region", { name: "Regras" })).toBeInTheDocument();
  });

  it("lista os sistemas e as análises no painel lateral", async () => {
    const { analise } = montar();
    const painel = screen.getByRole("region", { name: "Painel lateral" });
    const analises = within(painel).getByRole("group", { name: "Análises" });
    fireEvent.click(await within(analises).findByRole("button", { name: /A-0001: Multa por atraso/ }));
    expect(await screen.findByRole("region", { name: "Análise" })).toBeInTheDocument();
    await waitFor(() =>
      expect(analise.api.carregarAnalise).toHaveBeenCalledWith(SISTEMA, "A-0001"),
    );
    expect(
      within(painel).getByRole("group", { name: "Sistemas" }),
    ).toHaveTextContent("Faturamento Exemplo");
  });

  it("a área atual da barra de atividades mostra e esconde o painel lateral", async () => {
    montar();
    await screen.findByRole("region", { name: "Início" });
    const atividades = screen.getByRole("navigation", { name: "Telas" });
    const alternar = screen.getByRole("button", { name: "Fechar painel lateral" });
    expect(alternar).toHaveAttribute("aria-expanded", "true");

    fireEvent.click(within(atividades).getByRole("button", { name: "Início" }));
    expect(screen.queryByRole("region", { name: "Painel lateral" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Abrir painel lateral" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    fireEvent.click(within(atividades).getByRole("button", { name: "Início" }));
    expect(screen.getByRole("region", { name: "Painel lateral" })).toBeInTheDocument();
  });

  it("em tela estreita, o painel lateral é gaveta: começa fechado e fecha ao escolher", async () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn((q: string) => ({
        matches: q.includes("max-width: 900px"),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );
    try {
      montar();
      await screen.findByRole("region", { name: "Início" });
      expect(screen.queryByRole("region", { name: "Painel lateral" })).not.toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "Abrir painel lateral" }));
      const painel = screen.getByRole("region", { name: "Painel lateral" });
      const sistemas = within(painel).getByRole("group", { name: "Sistemas" });
      fireEvent.click(await within(sistemas).findByRole("button", { name: /Faturamento Exemplo/ }));
      expect(await screen.findByRole("region", { name: "Regras" })).toBeInTheDocument();
      expect(screen.queryByRole("region", { name: "Painel lateral" })).not.toBeInTheDocument();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("troca o tema pela barra de título", async () => {
    montar();
    await screen.findByRole("region", { name: "Início" });
    const tema = screen.getByRole("group", { name: "Tema" });
    fireEvent.click(within(tema).getByRole("button", { name: "Tema claro" }));
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(within(tema).getByRole("button", { name: "Tema claro" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.click(within(tema).getByRole("button", { name: "Tema escuro" }));
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
