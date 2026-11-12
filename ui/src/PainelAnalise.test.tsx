import { act } from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { PainelAnalise, type Abertura } from "./PainelAnalise";
import type { VistaAnalise } from "./analise";
import type { OpcoesConversa } from "./conversa";
import {
  AFIRMACAO_MULTA,
  apiAnaliseFalsa,
  apiConversaFalsa,
  apiTerminalFalsa,
  opcoesFicticias,
  telaFalsa,
  vistaFicticia,
} from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

function montar(abertura: Abertura, vista?: VistaAnalise, opcoes?: OpcoesConversa) {
  const analise = apiAnaliseFalsa(vista);
  const terminal = apiTerminalFalsa();
  const conversa = apiConversaFalsa(opcoes);
  const tela = telaFalsa();
  const aoVoltar = vi.fn();
  const r = render(
    <PainelAnalise
      api={analise.api}
      projeto="faturamento-exemplo"
      analise="A-0001"
      abertura={abertura}
      aoVoltar={aoVoltar}
      apiTerminal={terminal.api}
      apiConversa={conversa}
      criarTela={() => tela}
    />,
  );
  return { ...analise, terminal, conversa, aoVoltar, ...r };
}

async function terminalAberto(t: ReturnType<typeof apiTerminalFalsa>, id = 1) {
  await waitFor(() => expect(t.aberturas.length).toBeGreaterThan(0));
  await act(async () => t.aberturas[t.aberturas.length - 1].resolver(id));
}

describe("PainelAnalise", () => {
  it("análise nova abre o terminal com o pedido de iniciar_analise", async () => {
    const { terminal } = montar({
      tipo: "nova",
      promptInicial: "Inicie a análise A-0001 pelo ctower",
    });
    await waitFor(() => expect(terminal.api.abrir).toHaveBeenCalled());
    expect(terminal.aberturas[0].projeto).toBe("faturamento-exemplo");
    expect(terminal.aberturas[0].pedido).toEqual({
      promptInicial: "Inicie a análise A-0001 pelo ctower",
    });
    expect(await screen.findByRole("heading", { name: "A-0001: Multa por atraso" })).toBeVisible();
  });

  it("atualiza o documento ao vivo a cada aviso sobre a análise", async () => {
    const { api, avisar } = montar({ tipo: "reabrir" });
    expect(await screen.findByText("Nenhuma afirmação registrada ainda.")).toBeInTheDocument();
    expect(api.vigiar).toHaveBeenCalledWith("faturamento-exemplo", expect.any(Function));
    expect(api.carregarAnalise).toHaveBeenCalledTimes(1);

    // Aviso de outra análise, ou de versão já mostrada, não relê.
    act(() => avisar({ tipo: "mudanca", analise: "A-0002", mudanca: "afirmacao", versao: 5 }));
    act(() => avisar({ tipo: "mudanca", analise: "A-0001", mudanca: "afirmacao", versao: 1 }));
    expect(api.carregarAnalise).toHaveBeenCalledTimes(1);

    api.carregarAnalise.mockResolvedValue(
      vistaFicticia({}, { versao: 2, afirmacoes: [AFIRMACAO_MULTA] }),
    );
    act(() => avisar({ tipo: "mudanca", analise: "A-0001", mudanca: "afirmacao", versao: 2 }));
    expect(await screen.findByText(AFIRMACAO_MULTA.texto)).toBeInTheDocument();
    expect(screen.getByText("confirmado")).toHaveAttribute("data-tone", "done");
    expect(api.carregarAnalise).toHaveBeenCalledTimes(2);

    // Reinício da vigia: relê, mesmo sem versão nova.
    act(() => avisar({ tipo: "reinicio" }));
    await waitFor(() => expect(api.carregarAnalise).toHaveBeenCalledTimes(3));
  });

  it("para a vigia ao sair da tela", async () => {
    const { parar, unmount } = montar({ tipo: "reabrir" });
    await screen.findByText("Nenhuma afirmação registrada ainda.");
    unmount();
    expect(parar).toHaveBeenCalled();
  });

  it("abre o trecho da evidência com as linhas citadas destacadas", async () => {
    const { api } = montar(
      { tipo: "reabrir" },
      vistaFicticia({}, { afirmacoes: [AFIRMACAO_MULTA] }),
    );
    fireEvent.click(await screen.findByRole("button", { name: "db/03_pkg_cobranca.sql:71-72" }));
    expect(api.trechoEvidencia).toHaveBeenCalledWith(
      "faturamento-exemplo",
      "db/03_pkg_cobranca.sql",
      71,
      72,
    );
    const trecho = await screen.findByRole("region", { name: "Trecho da evidência" });
    await waitFor(() => expect(trecho.querySelector('[data-linha="71"]')).not.toBeNull());
    const linha = (n: number) => trecho.querySelector(`[data-linha="${n}"]`);
    expect(linha(70)).not.toHaveAttribute("data-citada");
    expect(linha(71)).toHaveAttribute("data-citada", "true");
    expect(linha(72)).toHaveAttribute("data-citada", "true");
    expect(linha(73)).not.toHaveAttribute("data-citada");
    expect(linha(71)).toHaveAttribute("aria-current", "location");
    expect(within(linha(71) as HTMLElement).getByText("linha 71")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "db/03_pkg_cobranca.sql:71-72" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.click(within(trecho).getByRole("button", { name: "Fechar" }));
    expect(screen.queryByRole("region", { name: "Trecho da evidência" })).not.toBeInTheDocument();
  });

  it("mostra a recusa de um trecho fora do escopo", async () => {
    const { api } = montar(
      { tipo: "reabrir" },
      vistaFicticia({}, { afirmacoes: [AFIRMACAO_MULTA] }),
    );
    api.trechoEvidencia.mockRejectedValue(
      "`db/03_pkg_cobranca.sql`: caminho fora das pastas do projeto",
    );
    fireEvent.click(await screen.findByRole("button", { name: "db/03_pkg_cobranca.sql:71-72" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "caminho fora das pastas do projeto",
    );
  });

  it.each([
    ["ativa", "Sessão ativa"],
    ["aguardando-analista", "Aguardando você"],
    ["concluida", "Resposta concluída"],
    ["encerrada", "Sessão encerrada"],
  ] as const)("mostra o estado da sessão %s", async (estado, texto) => {
    montar(
      { tipo: "reabrir" },
      vistaFicticia({ sessao: { id: "s-1", estado, cliente: "claude-code" } }),
    );
    const selo = await screen.findByRole("status", { name: "Estado da sessão" });
    expect(selo).toHaveTextContent(texto);
    expect(selo).toHaveAttribute("data-estado", estado);
  });

  it("sem sessão ligada, avisa que ela aparece na primeira ferramenta", async () => {
    montar({ tipo: "reabrir" });
    expect(await screen.findByRole("status", { name: "Estado da sessão" })).toHaveTextContent(
      "Nenhuma sessão ligada",
    );
  });

  it("sem prompts prontos, não mostra os botões", async () => {
    const { api, terminal } = montar({ tipo: "nova", promptInicial: "Inicie" });
    await terminalAberto(terminal);
    await waitFor(() =>
      expect(api.promptsProntos).toHaveBeenCalledWith("faturamento-exemplo", "A-0001"),
    );
    expect(screen.queryByRole("group", { name: "Prompts prontos" })).not.toBeInTheDocument();
  });

  it("envia um prompt pronto à sessão viva", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.promptsProntos.mockResolvedValue([
      {
        id: "resumir",
        rotulo: "Resumir para o solicitante",
        descricao: "Resumo em linguagem de negócio",
        texto: "Resuma para o solicitante.",
      },
    ]);
    const terminal = apiTerminalFalsa();
    render(
      <PainelAnalise
        api={analise.api}
        projeto="faturamento-exemplo"
        analise="A-0001"
        abertura={{ tipo: "nova", promptInicial: "Inicie" }}
        aoVoltar={() => undefined}
        apiTerminal={terminal.api}
        apiConversa={apiConversaFalsa()}
        criarTela={telaFalsa}
      />,
    );
    const botao = await screen.findByRole("button", { name: "Resumir para o solicitante" });
    expect(botao).toBeDisabled();
    await terminalAberto(terminal, 7);
    await waitFor(() => expect(botao).toBeEnabled());
    fireEvent.click(botao);
    await waitFor(() =>
      expect(terminal.api.enviarPrompt).toHaveBeenCalledWith(7, "Resuma para o solicitante."),
    );
  });

  it("continua no Desktop com o documento ao vivo e volta à conversa", async () => {
    const { terminal, conversa } = montar(
      { tipo: "nova", promptInicial: "Inicie" },
      vistaFicticia({ sessao: { id: "sessao-1", estado: "ativa", cliente: "claude-code" } }),
    );
    await terminalAberto(terminal, 3);
    const continuar = screen.getByRole("button", { name: "Continuar no Claude Desktop" });
    await waitFor(() => expect(continuar).toBeEnabled());
    fireEvent.click(continuar);
    await waitFor(() => expect(terminal.api.enviarPrompt).toHaveBeenCalledWith(3, "/desktop"));
    expect(await screen.findByText(/A conversa continua no Claude Desktop/)).toBeVisible();
    expect(conversa.passouAoDesktop).toHaveBeenCalledWith("faturamento-exemplo", "A-0001");
    expect(screen.getByTestId("terminal-tela")).not.toBeVisible();
    // O documento continua na tela.
    expect(screen.getByRole("article", { name: "Documento da análise" })).toBeVisible();

    // O claude saiu depois do /desktop: voltar retoma a mesma conversa aqui.
    act(() => terminal.aberturas[0].aoEvento({ tipo: "encerrado", fim: { codigo: 0 } }));
    fireEvent.click(screen.getByRole("button", { name: "Voltar à conversa aqui" }));
    await waitFor(() => expect(terminal.aberturas).toHaveLength(2));
    expect(terminal.aberturas[1].pedido).toEqual({
      retomada: { modo: "sessao", id: "sessao-1" },
    });
    expect(screen.queryByText(/A conversa continua no Claude Desktop/)).not.toBeInTheDocument();
  });

  it("reabre uma análise retomando a conversa ou começando outra", async () => {
    const { terminal } = montar(
      { tipo: "reabrir" },
      vistaFicticia({ sessao: { id: "sessao-9", estado: "encerrada", cliente: "claude-code" } }),
    );
    expect(terminal.api.abrir).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole("button", { name: "Retomar a conversa" }));
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    expect(terminal.aberturas[0].pedido).toEqual({ retomada: { modo: "sessao", id: "sessao-9" } });
  });

  it("reabre uma análise sem sessão com o pedido de retomar_analise", async () => {
    const { terminal } = montar({ tipo: "reabrir" });
    expect(screen.queryByRole("button", { name: "Retomar a conversa" })).not.toBeInTheDocument();
    const nova = await screen.findByRole("button", { name: "Nova conversa sobre esta análise" });
    await waitFor(() => expect(nova).toBeEnabled());
    fireEvent.click(nova);
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    expect(terminal.aberturas[0].pedido).toEqual({
      promptInicial: "Retome a análise A-0001 pelo ctower",
    });
  });

  it("com o Desktop preferido, a análise nova abre o link do Desktop e não o terminal", async () => {
    const { terminal, conversa } = montar(
      { tipo: "nova", promptInicial: "Inicie" },
      undefined,
      opcoesFicticias({
        preferido: "claudeDesktop",
        opcoes: [
          { cliente: "claudeDesktop", disponivel: true },
          { cliente: "terminalEmbutido", disponivel: true },
          { cliente: "terminalDoAnalista", disponivel: true },
        ],
      }),
    );
    await waitFor(() =>
      expect(conversa.abrirPorLink).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "A-0001",
        "claudeDesktop",
      ),
    );
    expect(conversa.abrirPorLink).toHaveBeenCalledTimes(1);
    expect(await screen.findByText(/confirme a pasta e envie/)).toBeVisible();
    expect(terminal.api.abrir).not.toHaveBeenCalled();
    // O preferido aparece primeiro, marcado; o terminal embutido fica no botão de voltar.
    const cartoes = within(screen.getByRole("group", { name: "Clientes da conversa" }))
      .getAllByRole("region")
      .map((r) => r.getAttribute("data-cliente"));
    expect(cartoes).toEqual(["claudeDesktop", "terminalDoAnalista"]);
    expect(screen.getByText("preferido")).toBeInTheDocument();
    // Voltar à conversa aqui, sem sessão ligada ainda: a mais recente da pasta.
    fireEvent.click(screen.getByRole("button", { name: "Voltar à conversa aqui" }));
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    expect(terminal.aberturas[0].pedido).toEqual({ retomada: { modo: "continuar" } });
  });

  it("sem as opções (comando ausente), a análise nova usa o terminal embutido", async () => {
    const analise = apiAnaliseFalsa();
    const terminal = apiTerminalFalsa();
    const conversa = apiConversaFalsa();
    conversa.opcoes.mockRejectedValue("comando desconhecido");
    render(
      <PainelAnalise
        api={analise.api}
        projeto="faturamento-exemplo"
        analise="A-0001"
        abertura={{ tipo: "nova", promptInicial: "Inicie" }}
        aoVoltar={() => undefined}
        apiTerminal={terminal.api}
        apiConversa={conversa}
        criarTela={telaFalsa}
      />,
    );
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    expect(terminal.aberturas[0].pedido).toEqual({ promptInicial: "Inicie" });
    // Sem as opções, o botão do Desktop não aparece.
    expect(
      screen.queryByRole("button", { name: "Continuar no Claude Desktop" }),
    ).not.toBeInTheDocument();
  });

  it("retoma no Desktop a sessão iniciada no terminal embutido e volta", async () => {
    const { terminal, conversa } = montar(
      { tipo: "reabrir" },
      vistaFicticia({ sessao: { id: "sessao-1", estado: "encerrada", cliente: "claude-code" } }),
      opcoesFicticias({ sessao: "sessao-1" }),
    );
    fireEvent.click(await screen.findByRole("button", { name: "Retomar no Claude Desktop" }));
    await waitFor(() =>
      expect(conversa.retomarNoDesktop).toHaveBeenCalledWith("faturamento-exemplo", "A-0001"),
    );
    expect(await screen.findByText(/A conversa continua no Claude Desktop/)).toBeVisible();
    expect(terminal.api.abrir).not.toHaveBeenCalled();
    // E volta ao terminal embutido pela mesma sessão.
    fireEvent.click(screen.getByRole("button", { name: "Voltar à conversa aqui" }));
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    expect(terminal.aberturas[0].pedido).toEqual({
      retomada: { modo: "sessao", id: "sessao-1" },
    });
  });

  it("mostra por que o Desktop não abriu a sessão e oferece a conversa nova", async () => {
    const { conversa } = montar(
      { tipo: "reabrir" },
      vistaFicticia({
        sessao: { id: "sessao-1", estado: "encerrada", cliente: "claude-desktop" },
      }),
      opcoesFicticias({ sessao: "sessao-1" }),
    );
    conversa.retomarNoDesktop.mockResolvedValue({
      aberto: false,
      codigo: 1,
      mensagem: "Session is open in another terminal",
    });
    fireEvent.click(await screen.findByRole("button", { name: "Retomar no Claude Desktop" }));
    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Session is open in another terminal");
    expect(alerta).toHaveTextContent("conversa nova no Desktop");
    fireEvent.click(screen.getByRole("button", { name: "Nova conversa no Claude Desktop" }));
    await waitFor(() =>
      expect(conversa.abrirPorLink).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "A-0001",
        "claudeDesktop",
      ),
    );
  });

  it("recusa do Desktop vinda do Rust mostra a mensagem", async () => {
    const { conversa } = montar(
      { tipo: "reabrir" },
      vistaFicticia({ sessao: { id: "sessao-1", estado: "encerrada", cliente: "claude-code" } }),
      opcoesFicticias({ sessao: "sessao-1" }),
    );
    conversa.retomarNoDesktop.mockRejectedValue({
      motivo: "desktopIndisponivel",
      detalhe: "versão antiga",
      orientacao: [],
      mensagem: "o Claude Code 2.1.200 não abre sessões no Desktop",
    });
    fireEvent.click(await screen.findByRole("button", { name: "Retomar no Claude Desktop" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "o Claude Code 2.1.200 não abre sessões no Desktop",
    );
  });

  it("oferece o terminal do analista com o comando e o link", async () => {
    const { conversa } = montar(
      { tipo: "reabrir" },
      vistaFicticia({ sessao: { id: "sessao-1", estado: "encerrada", cliente: "claude-code" } }),
      opcoesFicticias({
        sessao: "sessao-1",
        comandoRetomada: "cd /codigo/faturamento-exemplo && claude --resume sessao-1",
      }),
    );
    const cartao = await screen.findByRole("region", { name: "No meu terminal" });
    expect(
      within(cartao).getByText("cd /codigo/faturamento-exemplo && claude --resume sessao-1"),
    ).toBeInTheDocument();
    fireEvent.click(within(cartao).getByRole("button", { name: "Copiar o comando de retomada" }));
    await waitFor(() =>
      expect(conversa.copiar).toHaveBeenCalledWith(
        "cd /codigo/faturamento-exemplo && claude --resume sessao-1",
      ),
    );
    expect(await screen.findByText(/Comando copiado/)).toBeInTheDocument();
    fireEvent.click(
      within(cartao).getByRole("button", { name: "Copiar o comando de conversa nova" }),
    );
    await waitFor(() =>
      expect(conversa.copiar).toHaveBeenLastCalledWith(
        "cd /codigo/faturamento-exemplo && claude 'Inicie a análise A-0001 pelo ctower'",
      ),
    );
    fireEvent.click(within(cartao).getByRole("button", { name: "Abrir pelo link claude-cli://" }));
    await waitFor(() =>
      expect(conversa.abrirPorLink).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "A-0001",
        "terminalDoAnalista",
      ),
    );
    expect(await screen.findByText(/A conversa continua no seu terminal/)).toBeVisible();
  });

  it("passa do terminal embutido para o do analista encerrando o daqui", async () => {
    const { terminal } = montar(
      { tipo: "nova", promptInicial: "Inicie" },
      vistaFicticia({ sessao: { id: "sessao-1", estado: "ativa", cliente: "claude-code" } }),
      opcoesFicticias({
        sessao: "sessao-1",
        comandoRetomada: "cd /codigo/faturamento-exemplo && claude --resume sessao-1",
      }),
    );
    await terminalAberto(terminal, 4);
    fireEvent.click(screen.getByRole("button", { name: "Continuar no meu terminal" }));
    await waitFor(() => expect(terminal.api.encerrar).toHaveBeenCalledWith(4));
    expect(await screen.findByText(/A conversa continua no seu terminal/)).toBeVisible();
    expect(
      screen.getByText("cd /codigo/faturamento-exemplo && claude --resume sessao-1"),
    ).toBeVisible();
  });

  it("Desktop indisponível fica no fim, com o motivo, e sem o botão do /desktop", async () => {
    const { terminal } = montar(
      { tipo: "nova", promptInicial: "Inicie" },
      undefined,
      opcoesFicticias({
        plataforma: "linux",
        opcoes: [
          { cliente: "terminalEmbutido", disponivel: true },
          { cliente: "terminalDoAnalista", disponivel: true },
          {
            cliente: "claudeDesktop",
            disponivel: false,
            motivo: "o Claude Desktop não existe para Linux",
          },
        ],
      }),
    );
    await terminalAberto(terminal);
    expect(screen.getByRole("button", { name: "Continuar no meu terminal" })).toBeEnabled();
    expect(
      screen.queryByRole("button", { name: "Continuar no Claude Desktop" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continuar no meu terminal" }));
    expect(await screen.findByText("o Claude Desktop não existe para Linux")).toBeVisible();
  });
});
