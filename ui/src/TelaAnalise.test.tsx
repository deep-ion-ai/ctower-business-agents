import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { TelaAnalise } from "./TelaAnalise";
import { apiAnaliseFalsa, apiTerminalFalsa, telaFalsa } from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

function montar(analise = apiAnaliseFalsa()) {
  const terminal = apiTerminalFalsa();
  render(<TelaAnalise api={analise.api} apiTerminal={terminal.api} criarTela={telaFalsa} />);
  return { ...analise, terminal };
}

describe("TelaAnalise", () => {
  it("sem base ou sem projeto, leva ao primeiro uso", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.situacao.mockResolvedValueOnce({ base: "/base", projetos: [] });
    const aoPrimeiroUso = vi.fn();
    render(<TelaAnalise api={analise.api} aoPrimeiroUso={aoPrimeiroUso} />);
    expect(await screen.findByText(/Nenhum projeto cadastrado/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Abrir o primeiro uso" }));
    expect(aoPrimeiroUso).toHaveBeenCalled();
    expect(screen.queryByRole("form", { name: "Nova pergunta" })).not.toBeInTheDocument();
  });

  it("abre com o projeto e a pergunta sugeridos pelo primeiro uso", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.situacao.mockResolvedValueOnce({
      base: "/base",
      projetos: [
        { id: "estoque-exemplo", nome: "Estoque Exemplo" },
        { id: "faturamento-exemplo", nome: "Faturamento Exemplo" },
      ],
    });
    render(
      <TelaAnalise
        api={analise.api}
        projetoInicial="faturamento-exemplo"
        perguntaInicial="Quais são as principais regras de negócio deste projeto?"
      />,
    );
    expect(await screen.findByLabelText("Pergunta")).toHaveValue(
      "Quais são as principais regras de negócio deste projeto?",
    );
    expect(screen.getByRole("combobox")).toHaveValue("faturamento-exemplo");
  });

  it("só prepara o ambiente depois de mostrar o plano e do consentimento", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.verificarAmbiente
      .mockResolvedValueOnce({
        pronto: false,
        pendencias: ["o servidor MCP do app não está registrado para esta pasta"],
      })
      .mockResolvedValueOnce({ pronto: true, pendencias: [] });
    analise.api.planejarPreparo.mockResolvedValue({
      diretorio: "/codigo",
      gravado: false,
      alteracoes: [
        {
          arquivo: "/home/ana/.claude.json",
          item: { tipo: "servidorMcp", chave: "/codigo" },
          acao: "adicionado",
        },
        {
          arquivo: "/home/ana/.claude/settings.json",
          item: { tipo: "hook", evento: "SessionStart" },
          acao: "adicionado",
        },
      ],
    });
    montar(analise);
    expect(
      await screen.findByText("o servidor MCP do app não está registrado para esta pasta"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Perguntar" })).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "Ver o que será alterado" }));
    expect(await screen.findByText(/Servidor MCP "cba" para a pasta \/codigo/)).toBeInTheDocument();
    expect(screen.getByText("Hook SessionStart")).toBeInTheDocument();
    expect(analise.api.prepararAmbiente).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Concordo, preparar o ambiente" }));
    await waitFor(() =>
      expect(analise.api.prepararAmbiente).toHaveBeenCalledWith("faturamento-exemplo"),
    );
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Preparar o ambiente" })).not.toBeInTheDocument(),
    );
    fireEvent.change(screen.getByLabelText("Pergunta"), { target: { value: "Qual a multa?" } });
    expect(screen.getByRole("button", { name: "Perguntar" })).toBeEnabled();
  });

  it("cria a análise da pergunta e abre o terminal com o pedido de início", async () => {
    const { api, terminal } = montar();
    fireEvent.change(await screen.findByLabelText("Pergunta"), {
      target: { value: "Como é calculada a multa por atraso de pagamento?" },
    });
    const perguntar = screen.getByRole("button", { name: "Perguntar" });
    await waitFor(() => expect(perguntar).toBeEnabled());
    fireEvent.click(perguntar);
    await waitFor(() =>
      expect(api.criarAnalise).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "Como é calculada a multa por atraso de pagamento?",
        undefined,
      ),
    );
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    expect(terminal.aberturas[0].pedido).toEqual({
      promptInicial: "Inicie a análise A-0001 pelo ctower",
    });
    expect(await screen.findByRole("article", { name: "Documento da análise" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "← Análises" }));
    expect(await screen.findByRole("form", { name: "Nova pergunta" })).toBeInTheDocument();
  });

  it("mostra a recusa ao criar a análise", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.criarAnalise.mockRejectedValue(
      "a pasta da base de conhecimento não está acessível",
    );
    montar(analise);
    fireEvent.change(await screen.findByLabelText("Pergunta"), { target: { value: "Qual?" } });
    const perguntar = screen.getByRole("button", { name: "Perguntar" });
    await waitFor(() => expect(perguntar).toBeEnabled());
    fireEvent.click(perguntar);
    expect(await screen.findByRole("alert")).toHaveTextContent("não está acessível");
  });

  it("lista e reabre as análises do projeto", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.listarAnalises.mockResolvedValue([
      {
        id: "A-0001",
        titulo: "Multa por atraso",
        fluxo: "perguntar",
        estado: "concluida",
        atualizadaMs: 1,
      },
    ]);
    const { terminal } = montar(analise);
    expect(await screen.findByText("concluída")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "A-0001: Multa por atraso" }));
    expect(await screen.findByText("Como continuar esta análise?")).toBeInTheDocument();
    expect(analise.api.carregarAnalise).toHaveBeenCalledWith("faturamento-exemplo", "A-0001");
    expect(terminal.api.abrir).not.toHaveBeenCalled();
  });
});
