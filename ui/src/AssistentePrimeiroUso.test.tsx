import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { AssistentePrimeiroUso } from "./AssistentePrimeiroUso";
import { CadastroSistema } from "./CadastroSistema";
import { EscolhaBase } from "./EscolhaBase";
import { EscolhaCliente } from "./EscolhaCliente";
import {
  apiPrimeiroUsoFalsa,
  clientesProntos,
  inventarioFicticio,
  situacaoInicial,
} from "./test/falsos";

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

  it("recusa a base dentro do repositório do sistema", async () => {
    const api = apiPrimeiroUsoFalsa();
    const recusa =
      'a base não pode ficar dentro do repositório ou das pastas do sistema "Faturamento Exemplo" (/codigo/faturamento-exemplo)';
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

  it("escolhe a pasta pelo seletor do sistema e mostra a recusa da gravação", async () => {
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

describe("Primeiro uso: cadastro do sistema", () => {
  function preencher() {
    fireEvent.change(screen.getByLabelText("Nome do sistema"), {
      target: { value: "Faturamento Exemplo" },
    });
    fireEvent.change(screen.getByLabelText(/Pasta principal/), {
      target: { value: "/codigo/faturamento-exemplo" },
    });
  }

  it("mostra o inventário e o que fica fora antes de cadastrar", async () => {
    const api = apiPrimeiroUsoFalsa();
    render(<CadastroSistema api={api} aoConcluir={vi.fn()} />);
    const cadastrar = screen.getByRole("button", { name: "Cadastrar o sistema" });
    expect(cadastrar).toBeDisabled();
    preencher();
    fireEvent.click(screen.getByRole("button", { name: "Ver o que será analisado" }));
    const previa = within(await screen.findByRole("region", { name: "Prévia do inventário" }));
    expect(previa.getByText(/Será analisado: 14 arquivos/)).toBeInTheDocument();
    expect(previa.getByText("SQL")).toBeInTheDocument();
    expect(previa.getByText(/Fica fora: 1 itens/)).toBeInTheDocument();
    expect(previa.getAllByText("possível segredo").length).toBeGreaterThan(0);
    expect(previa.getByText(".env")).toBeInTheDocument();
    expect(cadastrar).toBeEnabled();
  });

  it("aplica as exclusões e reinclusões ajustadas pelo analista", async () => {
    const api = apiPrimeiroUsoFalsa();
    const aoConcluir = vi.fn();
    render(<CadastroSistema api={api} aoConcluir={aoConcluir} />);
    preencher();
    fireEvent.click(screen.getByRole("button", { name: "Ver o que será analisado" }));
    await screen.findByRole("region", { name: "Prévia do inventário" });

    // Excluir a pasta cobol: a prévia fica desatualizada até ser refeita.
    fireEvent.change(screen.getByLabelText(/Excluir também/), {
      target: { value: "cobol/\n\n*.log" },
    });
    expect(screen.getByText(/Atualize a prévia antes de cadastrar/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cadastrar o sistema" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Atualizar a prévia" }));
    await waitFor(() =>
      expect(api.previaSistema).toHaveBeenLastCalledWith(
        expect.objectContaining({ exclusoes: ["cobol/", "*.log"], inclusoes: [] }),
      ),
    );

    // Reincluir o .env pela lista do que fica fora.
    fireEvent.click(await screen.findByRole("button", { name: "Incluir .env na análise" }));
    await waitFor(() =>
      expect(api.previaSistema).toHaveBeenLastCalledWith(
        expect.objectContaining({ inclusoes: ["/.env"] }),
      ),
    );
    expect(
      within(screen.getByRole("group", { name: "Reincluídos" })).getByText("/.env"),
    ).toBeInTheDocument();

    const cadastrar = screen.getByRole("button", { name: "Cadastrar o sistema" });
    await waitFor(() => expect(cadastrar).toBeEnabled());
    fireEvent.click(cadastrar);
    await waitFor(() =>
      expect(api.cadastrarSistema).toHaveBeenCalledWith({
        nome: "Faturamento Exemplo",
        pastas: ["/codigo/faturamento-exemplo"],
        exclusoes: ["cobol/", "*.log"],
        inclusoes: ["/.env"],
      }),
    );
    await waitFor(() => expect(aoConcluir).toHaveBeenCalled());
  });

  it("aceita mais de uma pasta e avisa do sistema já cadastrado", async () => {
    const api = apiPrimeiroUsoFalsa();
    api.previaSistema.mockImplementation((p) =>
      Promise.resolve({
        id: "faturamento-exemplo",
        nome: p.nome,
        pastas: [],
        exclusoes: [],
        inclusoes: [],
        jaCadastrado: true,
        inventario: inventarioFicticio(),
      }),
    );
    render(<CadastroSistema api={api} aoConcluir={vi.fn()} />);
    preencher();
    fireEvent.click(screen.getByRole("button", { name: "Acrescentar outra pasta" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Escolher pasta…" })[1]);
    await waitFor(() =>
      expect(screen.getByLabelText(/Pasta 2/)).toHaveValue("/codigo/faturamento-exemplo"),
    );
    fireEvent.change(screen.getByLabelText(/Pasta 2/), { target: { value: "/codigo/telas" } });
    fireEvent.click(screen.getByRole("button", { name: "Ver o que será analisado" }));
    expect(await screen.findByText(/Já existe um sistema cadastrado/)).toBeInTheDocument();
    expect(api.previaSistema).toHaveBeenCalledWith(
      expect.objectContaining({ pastas: ["/codigo/faturamento-exemplo", "/codigo/telas"] }),
    );
    expect(screen.getByRole("button", { name: "Cadastrar o sistema" })).toBeDisabled();
  });

  it("mostra a recusa do cadastro", async () => {
    const api = apiPrimeiroUsoFalsa();
    api.previaSistema.mockRejectedValue("pasta não encontrada: /codigo/faturamento-exemplo");
    render(<CadastroSistema api={api} aoConcluir={vi.fn()} />);
    preencher();
    fireEvent.click(screen.getByRole("button", { name: "Ver o que será analisado" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("pasta não encontrada");
  });
});

describe("Assistente do primeiro uso", () => {
  async function ateOAmbiente(api: ReturnType<typeof apiPrimeiroUsoFalsa>) {
    fireEvent.click(await screen.findByRole("button", { name: "Continuar" }, { timeout: 2000 }));
    await screen.findByRole("form", { name: "Pasta da base de conhecimento" });
    await screen.findByText(/o app vai criá-la/);
    fireEvent.click(screen.getByRole("button", { name: "Usar esta pasta" }));
    await screen.findByRole("form", { name: "Cadastrar sistema" });
    fireEvent.change(screen.getByLabelText("Nome do sistema"), {
      target: { value: "Faturamento Exemplo" },
    });
    fireEvent.change(screen.getByLabelText(/Pasta principal/), {
      target: { value: "/codigo/faturamento-exemplo" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Ver o que será analisado" }));
    await screen.findByRole("region", { name: "Prévia do inventário" });
    fireEvent.click(screen.getByRole("button", { name: "Cadastrar o sistema" }));
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
    fireEvent.click(screen.getByRole("button", { name: "Ver o que será alterado" }));
    expect(await screen.findByText("Hook SessionStart")).toBeInTheDocument();
    expect(api.prepararAmbiente).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Concordo, preparar o ambiente" }));
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
        sistema: "faturamento-exemplo",
        pergunta: "Quais tabelas guardam os dados principais e como elas se relacionam?",
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
    fireEvent.click(await screen.findByRole("button", { name: "Ver o que será alterado" }));
    fireEvent.click(await screen.findByRole("button", { name: "Agora não" }));
    expect(api.prepararAmbiente).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Preparar depois" }));
    expect(await screen.findByText(/ainda não foi preparado/)).toBeInTheDocument();
    expect(api.prepararAmbiente).not.toHaveBeenCalled();
  });

  it("permite usar um sistema já cadastrado ao refazer o primeiro uso", async () => {
    const api = apiPrimeiroUsoFalsa();
    render(
      <AssistentePrimeiroUso
        api={api}
        situacao={situacaoInicial({
          base: "/Users/analista/Base",
          cliente: "terminalEmbutido",
          sistemas: [{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }],
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
    expect(api.cadastrarSistema).not.toHaveBeenCalled();
  });
});
