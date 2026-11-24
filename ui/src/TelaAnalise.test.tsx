import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaAnalise } from "./TelaAnalise";
import { apiAnaliseFalsa, apiTerminalFalsa, telaFalsa } from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

function montar(analise = apiAnaliseFalsa()) {
  const terminal = apiTerminalFalsa();
  render(<TelaAnalise api={analise.api} apiTerminal={terminal.api} criarTela={telaFalsa} />);
  return { ...analise, terminal };
}

/** "Nova análise" → "Perguntar ao sistema": o formulário da pergunta. */
async function abrirPergunta() {
  fireEvent.click(await screen.findByRole("button", { name: "Nova análise" }));
  fireEvent.click(screen.getByRole("button", { name: "Perguntar ao sistema" }));
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
          arquivo: "/home/ana/.claude.json",
          item: { tipo: "confianca", chave: "/codigo" },
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
    await abrirPergunta();
    expect(
      await screen.findByText("o servidor MCP do app não está registrado para esta pasta"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Perguntar" })).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "Ver e preparar" }));
    expect(await screen.findByText(/Servidor MCP "cba" para a pasta \/codigo/)).toBeInTheDocument();
    expect(screen.getByText("Hook SessionStart")).toBeInTheDocument();
    // A confiança na pasta entra no plano (#218).
    expect(
      screen.getByText(/Pasta \/codigo marcada como confiável no Claude Code/),
    ).toBeInTheDocument();
    expect(analise.api.prepararAmbiente).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Concordo, ligar o agente" }));
    await waitFor(() =>
      expect(analise.api.prepararAmbiente).toHaveBeenCalledWith("faturamento-exemplo"),
    );
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Preparar o ambiente" })).not.toBeInTheDocument(),
    );
    fireEvent.change(screen.getByLabelText("Pergunta"), { target: { value: "Qual a multa?" } });
    expect(screen.getByRole("button", { name: "Perguntar" })).toBeEnabled();
  });

  it("avisa que os avisos vão para outra instalação e pede concordância (#213)", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.verificarAmbiente
      .mockResolvedValueOnce({ pronto: false, pendencias: ["os hooks estão desatualizados"] })
      .mockResolvedValueOnce({ pronto: true, pendencias: [] });
    const dev = "/home/ana/projetos/cba/target/debug/cba";
    analise.api.planejarPreparo.mockResolvedValue({
      diretorio: "/codigo",
      gravado: false,
      alteracoes: ["SessionStart", "Stop"].map((evento) => ({
        arquivo: "/home/ana/.claude/settings.json",
        item: { tipo: "hook" as const, evento },
        acao: "atualizado" as const,
        outraInstalacao: dev,
      })),
    });
    montar(analise);
    fireEvent.click(await screen.findByRole("button", { name: "Ver e preparar" }));
    const aviso = await screen.findByRole("note");
    expect(aviso).toHaveTextContent("Hoje os avisos do Claude vão para outra instalação do app");
    expect(within(aviso).getAllByText(dev)).toHaveLength(1);
    expect(screen.getByText("Hook SessionStart").parentElement).toHaveTextContent(
      "hoje vai para outra instalação do app",
    );
    const concordo = screen.getByRole("button", { name: "Concordo, ligar o agente" });
    expect(concordo).toBeDisabled();
    fireEvent.click(within(aviso).getByRole("checkbox"));
    expect(concordo).toBeEnabled();
    fireEvent.click(concordo);
    await waitFor(() =>
      expect(analise.api.prepararAmbiente).toHaveBeenCalledWith("faturamento-exemplo"),
    );
  });

  it("cria a análise da pergunta e abre o terminal com o pedido de início", async () => {
    const { api, terminal } = montar();
    await abrirPergunta();
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
    expect(await screen.findByRole("button", { name: "Nova análise" })).toBeInTheDocument();
  });

  it("mostra a recusa ao criar a análise", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.criarAnalise.mockRejectedValue(
      "a pasta da base de conhecimento não está acessível",
    );
    montar(analise);
    await abrirPergunta();
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
    expect(await screen.findByText("aguardando revisão")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "A-0001: Multa por atraso" }));
    // O documento à esquerda; a conversa à direita, sem o bloco "Como continuar".
    expect(await screen.findByRole("article", { name: "Documento da análise" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Escolher onde continuar" })).toBeInTheDocument();
    expect(screen.queryByText("Como continuar esta análise?")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continuar a conversa" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    expect(analise.api.carregarAnalise).toHaveBeenCalledWith("faturamento-exemplo", "A-0001");
    expect(terminal.api.abrir).not.toHaveBeenCalled();
  });

  it("filtra a lista por situação, tipo e busca, com o mesmo rótulo de estado", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.listarAnalises.mockResolvedValue([
      { id: "A-0001", titulo: "Multa por atraso", fluxo: "perguntar", estado: "concluida", atualizadaMs: 1 },
      { id: "A-0002", titulo: "Parcelamento", fluxo: "impacto", estado: "aprovada", atualizadaMs: 2 },
      { id: "A-0003", titulo: "Cancelamento", fluxo: "perguntar", estado: "em-andamento", atualizadaMs: 3 },
    ]);
    montar(analise);
    const lista = await screen.findByRole("region", { name: "Análises do projeto" });
    expect(within(lista).getAllByRole("listitem")).toHaveLength(3);
    fireEvent.click(within(lista).getByRole("button", { name: "Aguardando revisão" }));
    expect(within(lista).getAllByRole("listitem")).toHaveLength(1);
    expect(within(lista).getByText("A-0001: Multa por atraso")).toBeInTheDocument();
    fireEvent.click(within(lista).getByRole("button", { name: "Aguardando revisão" }));
    fireEvent.change(screen.getByLabelText("Tipo de análise"), { target: { value: "impacto" } });
    expect(within(lista).getAllByRole("listitem")).toHaveLength(1);
    expect(within(lista).getByText("A-0002: Parcelamento")).toBeInTheDocument();
    fireEvent.click(within(lista).getByRole("button", { name: "Limpar filtros" }));
    fireEvent.change(screen.getByLabelText("Buscar análise"), { target: { value: "cancelamento" } });
    expect(within(lista).getAllByRole("listitem")).toHaveLength(1);
    fireEvent.change(screen.getByLabelText("Buscar análise"), { target: { value: "zzz" } });
    expect(within(lista).getByText("Nenhuma análise com este filtro.")).toBeInTheDocument();
  });

  it("abre já filtrada pela situação do link e com a escolha do tipo da ação global", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.listarAnalises.mockResolvedValue([
      { id: "A-0001", titulo: "Multa", fluxo: "perguntar", estado: "concluida", atualizadaMs: 1 },
      { id: "A-0002", titulo: "Parcelamento", fluxo: "perguntar", estado: "aprovada", atualizadaMs: 2 },
    ]);
    render(<TelaAnalise api={analise.api} situacaoInicial="concluida" novaInicial />);
    const lista = await screen.findByRole("region", { name: "Análises do projeto" });
    await waitFor(() => expect(within(lista).getAllByRole("listitem")).toHaveLength(1));
    expect(within(lista).getByRole("button", { name: "Aguardando revisão" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("group", { name: "Tipo de análise" })).toBeInTheDocument();
  });
});
