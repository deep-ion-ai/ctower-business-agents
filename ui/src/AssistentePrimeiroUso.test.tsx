import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { AssistentePrimeiroUso } from "./AssistentePrimeiroUso";
import { EscolhaBase } from "./EscolhaBase";
import { EscolhaCliente } from "./EscolhaCliente";
import { apiPrimeiroUsoFalsa, clientesProntos, situacaoInicial } from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const INSTALAR_CODE = {
  texto: "Abra um terminal e rode o instalador oficial da Anthropic:",
  comando: "curl -fsSL https://claude.ai/install.sh | bash",
};
const DOC_CODE = {
  texto: "Outras formas de instalar:",
  link: "https://code.claude.com/docs/en/setup",
};

function continuar() {
  return screen.getByRole("button", { name: "Continuar" });
}

describe("Primeiro uso: cliente de IA", () => {
  it("com o Claude Code ausente, mostra o passo a passo oficial e só segue depois de instalado", async () => {
    const api = apiPrimeiroUsoFalsa();
    api.estadoClientes
      .mockResolvedValueOnce(
        clientesProntos({
          claudeCode: {
            instalacao: { estado: "naoEncontrado" },
            login: { estado: "naoVerificado" },
            orientacao: [INSTALAR_CODE, DOC_CODE],
          },
        }),
      )
      .mockResolvedValueOnce(clientesProntos());
    const aoConcluir = vi.fn();
    render(<EscolhaCliente api={api} aoConcluir={aoConcluir} />);
    const code = within(await screen.findByRole("article", { name: "Claude Code" }));
    expect(code.getByText("Não instalado")).toBeInTheDocument();
    expect(code.getByText(INSTALAR_CODE.comando)).toBeInTheDocument();
    expect(screen.getByText(/Instale o Claude Code/)).toBeInTheDocument();
    expect(continuar()).toBeDisabled();

    // O link oficial abre no navegador pelo app.
    fireEvent.click(code.getByRole("link", { name: DOC_CODE.link }));
    expect(api.abrirLink).toHaveBeenCalledWith(DOC_CODE.link);

    fireEvent.click(screen.getByRole("button", { name: "Verificar de novo" }));
    await waitFor(() => expect(continuar()).toBeEnabled());
    fireEvent.click(continuar());
    await waitFor(() => expect(aoConcluir).toHaveBeenCalledWith("terminalEmbutido"));
    expect(api.escolherCliente).toHaveBeenCalledWith("terminalEmbutido");
  });

  it("com o Claude Code sem login, pede o login pelo próprio cliente", async () => {
    const api = apiPrimeiroUsoFalsa();
    api.estadoClientes.mockResolvedValue(
      clientesProntos({
        claudeCode: {
          ...clientesProntos().claudeCode,
          login: { estado: "naoLogado" },
          orientacao: [{ texto: "Rode o Claude Code e faça login:", comando: "claude" }],
        },
      }),
    );
    render(<EscolhaCliente api={api} aoConcluir={vi.fn()} />);
    const code = within(await screen.findByRole("article", { name: "Claude Code" }));
    expect(code.getByText("Sem login")).toBeInTheDocument();
    expect(code.getByText("claude")).toBeInTheDocument();
    expect(screen.getByText(/Entre no Claude Code com a sua conta/)).toBeInTheDocument();
    expect(continuar()).toBeDisabled();
  });

  it("com login não confirmado, deixa seguir com um aviso", async () => {
    const api = apiPrimeiroUsoFalsa();
    api.estadoClientes.mockResolvedValue(
      clientesProntos({
        claudeCode: {
          ...clientesProntos().claudeCode,
          login: { estado: "indeterminado", motivo: "sem resposta" },
        },
      }),
    );
    render(<EscolhaCliente api={api} aoConcluir={vi.fn()} />);
    expect(await screen.findByText(/Não deu para confirmar o login/)).toBeInTheDocument();
    expect(continuar()).toBeEnabled();
  });

  it("com o Claude Desktop, pede a confirmação do login no próprio Desktop", async () => {
    const api = apiPrimeiroUsoFalsa();
    const aoConcluir = vi.fn();
    render(<EscolhaCliente api={api} aoConcluir={aoConcluir} />);
    fireEvent.click(await screen.findByRole("radio", { name: /Claude Desktop/ }));
    expect(await screen.findByRole("article", { name: "Claude Desktop" })).toBeInTheDocument();
    expect(continuar()).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: /Já entrei no Claude Desktop/ }));
    fireEvent.click(continuar());
    await waitFor(() => expect(aoConcluir).toHaveBeenCalledWith("claudeDesktop"));
  });

  it("no Linux, o Claude Desktop não pode ser escolhido", async () => {
    const api = apiPrimeiroUsoFalsa();
    api.estadoClientes.mockResolvedValue(
      clientesProntos({
        plataforma: "linux",
        claudeDesktop: { instalacao: { estado: "naoSuportado" }, orientacao: [] },
      }),
    );
    render(<EscolhaCliente api={api} aoConcluir={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByRole("radio", { name: /Claude Desktop/ })).toBeDisabled(),
    );
  });
});

describe("Primeiro uso: base de conhecimento", () => {
  it("sugere Documentos/ctower-business e grava a pasta escolhida", async () => {
    const api = apiPrimeiroUsoFalsa();
    const aoConcluir = vi.fn();
    render(
      <EscolhaBase
        api={api}
        sugerida="/Users/analista/Documents/ctower-business"
        aoConcluir={aoConcluir}
      />,
    );
    expect(screen.getByLabelText("Pasta da base (caminho completo)")).toHaveValue(
      "/Users/analista/Documents/ctower-business",
    );
    expect(await screen.findByText(/o app vai criá-la/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Usar esta pasta" }));
    await waitFor(() =>
      expect(aoConcluir).toHaveBeenCalledWith("/Users/analista/Documents/ctower-business"),
    );
  });

  it("recusa a base dentro do repositório do projeto", async () => {
    const api = apiPrimeiroUsoFalsa();
    const recusa =
      'a base não pode ficar dentro do repositório ou das pastas do projeto "Faturamento Exemplo" (/codigo/faturamento-exemplo)';
    api.avaliarBase.mockImplementation((pasta: string) =>
      Promise.resolve({
        caminho: pasta,
        estado: "nova",
        recusa: pasta.startsWith("/codigo/") ? recusa : null,
        git: pasta.startsWith("/codigo/"),
      }),
    );
    render(<EscolhaBase api={api} aoConcluir={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Pasta da base (caminho completo)"), {
      target: { value: "/codigo/faturamento-exemplo/docs" },
    });
    expect(await screen.findByRole("alert")).toHaveTextContent("dentro do repositório");
    expect(screen.getByRole("button", { name: "Usar esta pasta" })).toBeDisabled();
    expect(api.configurarBase).not.toHaveBeenCalled();
  });

  it("escolhe a pasta pelo seletor do projeto e mostra a recusa da gravação", async () => {
    const api = apiPrimeiroUsoFalsa();
    api.escolherPasta.mockResolvedValue("/Users/analista/Base");
    api.configurarBase.mockRejectedValue("a base não pode ficar numa pasta temporária (/tmp)");
    render(<EscolhaBase api={api} aoConcluir={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Escolher pasta…" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Pasta da base (caminho completo)")).toHaveValue(
        "/Users/analista/Base",
      ),
    );
    await screen.findByText(/o app vai criá-la/);
    fireEvent.click(screen.getByRole("button", { name: "Usar esta pasta" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("pasta temporária");
  });
});

/** O caminho comum do assistente de projeto: uma pasta e Continuar até cadastrar. */
async function cadastrarPeloAssistente() {
  fireEvent.click(screen.getByRole("button", { name: /Escolher a pasta do código/ }));
  await screen.findByRole("button", { name: "Trocar a pasta do código" });
  fireEvent.click(continuar());
  await screen.findByRole("region", { name: "Prévia do inventário" });
  fireEvent.click(continuar());
  await screen.findByRole("heading", { name: "Conectar as demandas (opcional)" });
  fireEvent.click(continuar());
  fireEvent.click(await screen.findByRole("button", { name: "Cadastrar o projeto" }));
  await screen.findByRole("heading", { name: "“Faturamento Exemplo” está cadastrado" });
}

describe("Assistente do primeiro uso", () => {
  async function ateOAmbiente(api: ReturnType<typeof apiPrimeiroUsoFalsa>) {
    fireEvent.click(await screen.findByRole("button", { name: "Continuar" }, { timeout: 2000 }));
    await screen.findByRole("form", { name: "Pasta da base de conhecimento" });
    await screen.findByText(/o app vai criá-la/);
    fireEvent.click(screen.getByRole("button", { name: "Usar esta pasta" }));
    await screen.findByRole("form", { name: "Cadastrar projeto" });
    await cadastrarPeloAssistente();
    fireEvent.click(screen.getByRole("button", { name: "Ligar o agente a este projeto" }));
    await screen.findByRole("region", { name: "Ambiente do Claude" });
    expect(api.configurarBase).toHaveBeenCalledWith("/Users/analista/Documents/ctower-business");
  }

  it("só prepara o ambiente com o consentimento e termina na primeira análise", async () => {
    const api = apiPrimeiroUsoFalsa();
    api.verificarAmbiente
      .mockResolvedValueOnce({
        pronto: false,
        pendencias: ["o servidor MCP do app não está registrado para esta pasta"],
      })
      .mockResolvedValue({ pronto: true, pendencias: [] });
    api.planejarPreparo.mockResolvedValue({
      diretorio: "/codigo/faturamento-exemplo",
      gravado: false,
      alteracoes: [
        {
          arquivo: "/Users/analista/.claude.json",
          item: { tipo: "servidorMcp", chave: "/codigo/faturamento-exemplo" },
          acao: "adicionado",
        },
        {
          arquivo: "/Users/analista/.claude/settings.json",
          item: { tipo: "hook", evento: "SessionStart" },
          acao: "adicionado",
        },
      ],
    });
    const aoConcluir = vi.fn();
    render(
      <AssistentePrimeiroUso api={api} situacao={situacaoInicial()} aoConcluir={aoConcluir} />,
    );
    await ateOAmbiente(api);

    expect(
      await screen.findByText("o servidor MCP do app não está registrado para esta pasta"),
    ).toBeInTheDocument();
    expect(continuar()).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Ver e preparar" }));
    expect(await screen.findByText("Hook SessionStart")).toBeInTheDocument();
    expect(api.prepararAmbiente).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Concordo, ligar o agente" }));
    await waitFor(() => expect(api.prepararAmbiente).toHaveBeenCalledWith("faturamento-exemplo"));
    expect(await screen.findByText(/O ambiente do Claude está preparado/)).toBeInTheDocument();
    fireEvent.click(continuar());

    const pronto = within(await screen.findByRole("region", { name: "Primeira análise" }));
    fireEvent.click(
      pronto.getByRole("button", {
        name: "Quais tabelas guardam os dados principais e como elas se relacionam?",
      }),
    );
    fireEvent.click(pronto.getByRole("button", { name: "Fazer a primeira análise" }));
    await waitFor(() =>
      expect(aoConcluir).toHaveBeenCalledWith({
        projeto: "faturamento-exemplo",
        pergunta: "Quais tabelas guardam os dados principais e como elas se relacionam?",
        ambientePronto: true,
      }),
    );
    expect(api.concluir).toHaveBeenCalled();
  });

  it("sem o consentimento, nada é gravado e o analista pode preparar depois", async () => {
    const api = apiPrimeiroUsoFalsa();
    api.verificarAmbiente.mockResolvedValue({
      pronto: false,
      pendencias: ["os hooks do app não estão registrados"],
    });
    render(<AssistentePrimeiroUso api={api} situacao={situacaoInicial()} aoConcluir={vi.fn()} />);
    await ateOAmbiente(api);
    fireEvent.click(await screen.findByRole("button", { name: "Ver e preparar" }));
    fireEvent.click(await screen.findByRole("button", { name: "Agora não" }));
    expect(api.prepararAmbiente).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Preparar depois" }));
    expect(await screen.findByText(/ainda não foi ligado/)).toBeInTheDocument();
    expect(api.prepararAmbiente).not.toHaveBeenCalled();
  });

  it("“Fazer depois” no fim do cadastro segue para a primeira análise sem preparar", async () => {
    const api = apiPrimeiroUsoFalsa();
    render(<AssistentePrimeiroUso api={api} situacao={situacaoInicial()} aoConcluir={vi.fn()} />);
    fireEvent.click(await screen.findByRole("button", { name: "Continuar" }, { timeout: 2000 }));
    await screen.findByText(/o app vai criá-la/);
    fireEvent.click(screen.getByRole("button", { name: "Usar esta pasta" }));
    await screen.findByRole("form", { name: "Cadastrar projeto" });
    await cadastrarPeloAssistente();
    expect(api.cadastrarProjeto).toHaveBeenCalledWith(
      expect.objectContaining({ nome: "Faturamento Exemplo", pastas: ["/codigo/faturamento-exemplo"] }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Fazer depois" }));
    expect(await screen.findByRole("region", { name: "Primeira análise" })).toBeInTheDocument();
    expect(screen.getByText(/ainda não foi ligado/)).toBeInTheDocument();
    expect(api.prepararAmbiente).not.toHaveBeenCalled();
  });

  it("permite usar um projeto já cadastrado ao refazer o primeiro uso", async () => {
    const api = apiPrimeiroUsoFalsa();
    render(
      <AssistentePrimeiroUso
        api={api}
        situacao={situacaoInicial({
          base: "/Users/analista/Base",
          cliente: "terminalEmbutido",
          projetos: [{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }],
          primeiroUsoConcluido: true,
        })}
        aoConcluir={vi.fn()}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Continuar" }));
    expect(await screen.findByLabelText("Pasta da base (caminho completo)")).toHaveValue(
      "/Users/analista/Base",
    );
    await screen.findByText(/o app vai criá-la/);
    fireEvent.click(screen.getByRole("button", { name: "Usar esta pasta" }));
    fireEvent.click(await screen.findByRole("button", { name: "Usar este" }));
    expect(await screen.findByRole("region", { name: "Ambiente do Claude" })).toBeInTheDocument();
    expect(api.cadastrarProjeto).not.toHaveBeenCalled();
  });
});
