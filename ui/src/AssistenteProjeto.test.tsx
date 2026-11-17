import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { AssistenteProjeto } from "./AssistenteProjeto";
import type { RepoDetectado, ResultadoClone } from "./repositorios";
import { PASTA_CLONES, apiConfiguracoesFalsa } from "./test/configuracoesFalso";
import { inventarioFicticio } from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const PASTA = "/codigo/faturamento-exemplo";

const DETECTADOS: RepoDetectado[] = [
  {
    caminho: `${PASTA}/faturamento-api`,
    branch: "main",
    remoto: "git@github.com:acme/faturamento-api.git",
    forja: "github",
    host: "github.com",
    caminhoRepo: "acme/faturamento-api",
  },
  {
    caminho: `${PASTA}/faturamento-web`,
    branch: "develop",
    remoto: "https://gitlab.empresa.com.br/fin/faturamento-web.git",
    forja: "gitlab",
    host: "gitlab.empresa.com.br",
    caminhoRepo: "fin/faturamento-web",
  },
];

type Api = ReturnType<typeof apiConfiguracoesFalsa>;

function montar(api: Api = apiConfiguracoesFalsa()) {
  const aoConcluir = vi.fn();
  const aoPrepararAmbiente = vi.fn();
  const aoCancelar = vi.fn();
  render(
    <AssistenteProjeto
      api={api}
      aoConcluir={aoConcluir}
      aoPrepararAmbiente={aoPrepararAmbiente}
      aoCancelar={aoCancelar}
    />,
  );
  return { api, aoConcluir, aoPrepararAmbiente, aoCancelar };
}

function botao(nome: string | RegExp) {
  return screen.getByRole("button", { name: nome });
}

function titulo(nome: string | RegExp) {
  return screen.getByRole("heading", { level: 2, name: nome });
}

async function escolherPasta() {
  fireEvent.click(botao(/Escolher a pasta do código/));
  await screen.findByRole("button", { name: "Trocar a pasta do código" });
}

/** Passo 1 pela pasta existente, até a prévia do passo 2 aparecer. */
async function ateOPasso2() {
  await escolherPasta();
  fireEvent.click(botao("Continuar"));
  await screen.findByRole("region", { name: "Prévia do inventário" });
}

async function ateOPasso3() {
  await ateOPasso2();
  fireEvent.click(botao("Continuar"));
  await screen.findByRole("heading", {
    level: 2,
    name: "Conectar as demandas (opcional)",
  });
}

describe("Assistente de projeto: caminho comum", () => {
  it("cadastra com uma escolha de pasta e Continuar até o fim", async () => {
    const { api, aoConcluir } = montar();
    // A pasta existente já vem escolhida como origem.
    expect(
      screen.getByRole("radio", { name: /Usar uma pasta que já tenho/ }),
    ).toBeChecked();
    await escolherPasta();
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("region", { name: "Prévia do inventário" });
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", {
      name: "Conectar as demandas (opcional)",
    });
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", { name: "Tudo certo?" });
    fireEvent.click(botao("Cadastrar o projeto"));
    await waitFor(() =>
      expect(api.cadastrarProjeto).toHaveBeenCalledWith({
        nome: "Faturamento Exemplo",
        pastas: [PASTA],
        exclusoes: [],
        inclusoes: [],
        repositorios: { codigo: [], tracker: null },
      }),
    );
    expect(
      await screen.findByRole("heading", {
        name: "“Faturamento Exemplo” está cadastrado",
      }),
    ).toBeInTheDocument();
    expect(api.escolherPasta).toHaveBeenCalledTimes(1);
    expect(api.salvarToken).not.toHaveBeenCalled();
    fireEvent.click(botao("Fazer depois"));
    expect(aoConcluir).toHaveBeenCalledWith(
      expect.objectContaining({ id: "faturamento-exemplo" }),
    );
  });

  it("mostra as etapas: a atual marcada, as feitas clicáveis e as futuras não", async () => {
    montar();
    const etapas = within(
      screen.getByRole("navigation", { name: "Etapas do cadastro" }),
    );
    expect(etapas.getByRole("button", { name: /Código/ })).toHaveAttribute(
      "aria-current",
      "step",
    );
    expect(
      etapas.getByRole("button", { name: /O que será lido/ }),
    ).toBeDisabled();
    await ateOPasso2();
    const codigo = etapas.getByRole("button", { name: /Código/ });
    expect(codigo).toBeEnabled();
    expect(codigo).not.toHaveAttribute("aria-current");
    expect(
      etapas.getByRole("button", { name: /O que será lido/ }),
    ).toHaveAttribute("aria-current", "step");
    expect(etapas.getByRole("button", { name: /Demandas/ })).toBeDisabled();
    // O foco vai para o título do passo novo.
    expect(titulo("É isto que a IA vai ler")).toHaveFocus();
    fireEvent.click(codigo);
    expect(titulo("De onde vem o código do projeto?")).toHaveFocus();
  });

  it("Cancelar avisa quem abriu o assistente", async () => {
    const { aoCancelar } = montar();
    await act(async () => undefined);
    fireEvent.click(botao("Cancelar"));
    expect(aoCancelar).toHaveBeenCalled();
  });
});

describe("Assistente de projeto: passo 1, pasta existente", () => {
  it("lista os repositórios git da pasta e sugere o nome", async () => {
    const api = apiConfiguracoesFalsa();
    api.detectarRepositorios.mockResolvedValue(DETECTADOS);
    montar(api);
    expect(
      screen.getByText(
        /Pode ser a pasta de um repositório ou uma pasta com vários clones/,
      ),
    ).toBeInTheDocument();
    await escolherPasta();
    expect(api.detectarRepositorios).toHaveBeenCalledWith(PASTA);
    expect(
      await screen.findByText(/Encontrei 2 repositórios git dentro dela/),
    ).toBeInTheDocument();
    const lista = within(
      screen.getByRole("list", { name: "Repositórios git encontrados" }),
    );
    expect(lista.getByText("faturamento-api/")).toBeInTheDocument();
    expect(
      lista.getByText("GitLab · gitlab.empresa.com.br"),
    ).toBeInTheDocument();
    expect(lista.getByText(/branch develop/)).toBeInTheDocument();
    expect(screen.getByLabelText("Nome do projeto")).toHaveValue(
      "Faturamento Exemplo",
    );

    fireEvent.click(botao("Continuar"));
    await waitFor(() =>
      expect(api.previaProjeto).toHaveBeenCalledWith(
        expect.objectContaining({
          pastas: [PASTA],
          repositorios: {
            codigo: [
              expect.objectContaining({
                forja: "github",
                host: "github.com",
                caminho: "acme/faturamento-api",
                url_clone: "git@github.com:acme/faturamento-api.git",
                pasta: `${PASTA}/faturamento-api`,
                token: "global",
              }),
              expect.objectContaining({
                forja: "gitlab",
                caminho: "fin/faturamento-web",
              }),
            ],
            tracker: null,
          },
        }),
      ),
    );
  });

  it("não segue sem a pasta nem sem o nome", async () => {
    montar();
    fireEvent.click(botao("Continuar"));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Escolha a pasta do código",
    );
    await escolherPasta();
    fireEvent.change(screen.getByLabelText("Nome do projeto"), {
      target: { value: " " },
    });
    fireEvent.click(botao("Continuar"));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Dê um nome ao projeto",
    );
    expect(titulo("De onde vem o código do projeto?")).toBeInTheDocument();
  });
});

describe("Assistente de projeto: passo 1, clonar", () => {
  function url(n: number) {
    return screen.getByLabelText(`URL do repositório ${n}`);
  }

  it("monta os repositórios pelas URLs, guarda os tokens e clona com progresso", async () => {
    const api = apiConfiguracoesFalsa();
    let concluir: (r: ResultadoClone[]) => void = () => undefined;
    let avisar: (
      e: Parameters<Parameters<Api["clonarRepositorios"]>[2]>[0],
    ) => void = () => undefined;
    api.clonarRepositorios.mockImplementationOnce(
      (repos, base, aoEvento) =>
        new Promise((resolver) => {
          avisar = aoEvento;
          concluir = () =>
            resolver(
              repos.map((r) => ({
                repositorio: {
                  ...r,
                  pasta: `${base}/${r.caminho.split("/").pop()}`,
                },
                erro: null,
              })),
            );
        }),
    );
    montar(api);
    fireEvent.click(
      screen.getByRole("radio", { name: /Clonar do GitHub ou GitLab/ }),
    );
    expect(botao("Clonar e continuar")).toBeInTheDocument();
    expect(await screen.findByDisplayValue(PASTA_CLONES)).toBeInTheDocument();

    fireEvent.change(url(1), {
      target: {
        value: "https://gitlab.empresa.com.br/fin/faturamento-web.git",
      },
    });
    expect(
      screen.getByText("GitLab · gitlab.empresa.com.br"),
    ).toBeInTheDocument();
    expect(screen.getByText("faturamento-web")).toBeInTheDocument();

    // Colar várias URLs, uma por linha, acrescenta um repositório por URL.
    fireEvent.click(botao("+ Adicionar repositório"));
    expect(url(2)).toHaveFocus();
    fireEvent.paste(url(2), {
      clipboardData: {
        getData: () =>
          "git@github.com:acme/faturamento-api.git\nhttps://github.com/acme/docs\n",
      },
    });
    expect(url(2)).toHaveValue("git@github.com:acme/faturamento-api.git");
    expect(url(3)).toHaveValue("https://github.com/acme/docs");
    fireEvent.change(screen.getAllByLabelText("Branch (opcional)")[0]!, {
      target: { value: "develop" },
    });

    // Tokens globais: uma linha por host.
    fireEvent.change(
      await screen.findByLabelText("Token global de gitlab.empresa.com.br"),
      {
        target: { value: "glpat-123" },
      },
    );
    expect(
      screen.getByLabelText("Token global de github.com"),
    ).toBeInTheDocument();
    expect(screen.getByText(/Escopo mínimo/)).toBeInTheDocument();
    fireEvent.click(botao("Salvar o token de gitlab.empresa.com.br"));
    await waitFor(() =>
      expect(api.salvarToken).toHaveBeenCalledWith(
        { host: "gitlab.empresa.com.br", caminho: null },
        "glpat-123",
      ),
    );
    expect(
      await screen.findByRole("button", {
        name: "Trocar o token de gitlab.empresa.com.br",
      }),
    ).toBeInTheDocument();

    // O repositório 3 tem um token só dele.
    const repo3 = within(
      screen.getByRole("listitem", { name: "Repositório 3" }),
    );
    fireEvent.click(repo3.getByRole("radio", { name: "Só deste repositório" }));
    fireEvent.change(
      repo3.getByLabelText("Token de acesso deste repositório (PAT)"),
      {
        target: { value: "ghp_proprio" },
      },
    );
    fireEvent.click(repo3.getByRole("button", { name: "Testar acesso" }));
    await waitFor(() =>
      expect(api.testarAcesso).toHaveBeenCalledWith(
        expect.objectContaining({ caminho: "acme/docs", token: "proprio" }),
        "ghp_proprio",
      ),
    );
    expect(await repo3.findByText("Acesso OK.")).toBeInTheDocument();

    // O nome vem do primeiro repositório (a pasta dos clones é a padrão).
    expect(screen.getByLabelText("Nome do projeto")).toHaveValue(
      "Faturamento Web",
    );

    fireEvent.click(botao("Clonar e continuar"));
    await waitFor(() => expect(api.clonarRepositorios).toHaveBeenCalled());
    expect(api.salvarToken).toHaveBeenCalledWith(
      { host: "github.com", caminho: "acme/docs" },
      "ghp_proprio",
    );
    const [repos, base] = api.clonarRepositorios.mock.calls[0]!;
    expect(base).toBe(PASTA_CLONES);
    expect(repos.map((r) => [r.forja, r.host, r.caminho, r.branch])).toEqual([
      ["gitlab", "gitlab.empresa.com.br", "fin/faturamento-web", "develop"],
      ["github", "github.com", "acme/faturamento-api", ""],
      ["github", "github.com", "acme/docs", ""],
    ]);
    expect(botao("Clonando…")).toBeDisabled();

    act(() => {
      avisar({ tipo: "inicio", indice: 0 });
      avisar({
        tipo: "progresso",
        indice: 0,
        etapa: "Recebendo objetos",
        percentual: 40,
      });
    });
    const barra = await screen.findByRole("progressbar", {
      name: "Clone de faturamento-web",
    });
    expect(barra).toHaveAttribute("aria-valuenow", "40");
    expect(screen.getByText("Recebendo objetos · 40%")).toBeInTheDocument();
    await act(async () => {
      avisar({
        tipo: "concluido",
        indice: 0,
        pasta: `${PASTA_CLONES}/faturamento-web`,
      });
      concluir([]);
    });

    // Tudo clonado: segue para o passo 2 com as pastas clonadas.
    await screen.findByRole("heading", { name: "É isto que a IA vai ler" });
    await waitFor(() =>
      expect(api.previaProjeto).toHaveBeenCalledWith(
        expect.objectContaining({
          nome: "Faturamento Web",
          pastas: [
            `${PASTA_CLONES}/faturamento-web`,
            `${PASTA_CLONES}/faturamento-api`,
            `${PASTA_CLONES}/docs`,
          ],
        }),
      ),
    );

    // De volta ao passo 1: o resumo do clone.
    fireEvent.click(botao("Voltar"));
    const resumo = within(
      await screen.findByRole("group", { name: "Repositórios clonados" }),
    );
    expect(resumo.getByText(/3 repositórios clonados em/)).toBeInTheDocument();
    expect(resumo.getByText("faturamento-web/")).toBeInTheDocument();
    expect(
      resumo.getByText(
        /branch develop · token global de gitlab.empresa.com.br/,
      ),
    ).toBeInTheDocument();
    expect(resumo.getByText(/token próprio/)).toBeInTheDocument();
    expect(botao("Continuar")).toBeInTheDocument();
    fireEvent.click(
      resumo.getByRole("button", { name: "Alterar repositórios" }),
    );
    expect(url(1)).toHaveValue(
      "https://gitlab.empresa.com.br/fin/faturamento-web.git",
    );
  });

  it("mostra o erro de um repositório e tenta de novo só ele", async () => {
    const api = apiConfiguracoesFalsa();
    api.clonarRepositorios.mockImplementationOnce((repos, base, aoEvento) => {
      aoEvento({ tipo: "inicio", indice: 0 });
      aoEvento({
        tipo: "concluido",
        indice: 0,
        pasta: `${base}/faturamento-web`,
      });
      aoEvento({ tipo: "inicio", indice: 1 });
      aoEvento({ tipo: "falhou", indice: 1, erro: "acesso negado" });
      return Promise.resolve([
        {
          repositorio: { ...repos[0]!, pasta: `${base}/faturamento-web` },
          erro: null,
        },
        { repositorio: repos[1]!, erro: "acesso negado" },
      ]);
    });
    montar(api);
    fireEvent.click(
      screen.getByRole("radio", { name: /Clonar do GitHub ou GitLab/ }),
    );
    await screen.findByDisplayValue(PASTA_CLONES);
    fireEvent.change(url(1), {
      target: {
        value: "https://gitlab.empresa.com.br/fin/faturamento-web.git",
      },
    });
    fireEvent.click(botao("+ Adicionar repositório"));
    fireEvent.change(url(2), {
      target: { value: "git@github.com:acme/faturamento-api.git" },
    });
    fireEvent.click(botao("Clonar e continuar"));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Não deu para clonar faturamento-api: acesso negado",
    );
    expect(titulo("De onde vem o código do projeto?")).toBeInTheDocument();
    expect(
      screen.getByRole("progressbar", { name: "Clone de faturamento-web" }),
    ).toHaveAttribute("aria-valuenow", "100");

    fireEvent.click(botao("Tentar de novo o clone de faturamento-api"));
    await waitFor(() =>
      expect(api.clonarRepositorios).toHaveBeenCalledTimes(2),
    );
    expect(
      api.clonarRepositorios.mock.calls[1]![0].map((r) => r.caminho),
    ).toEqual(["acme/faturamento-api"]);
    expect(
      await screen.findByText(/2 repositórios clonados em/),
    ).toBeInTheDocument();
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", { name: "É isto que a IA vai ler" });
  });

  it("recusa URL que não é de clone e a troca do token por um comando", async () => {
    const api = apiConfiguracoesFalsa();
    montar(api);
    fireEvent.click(
      screen.getByRole("radio", { name: /Clonar do GitHub ou GitLab/ }),
    );
    await screen.findByDisplayValue(PASTA_CLONES);
    fireEvent.click(botao("Clonar e continuar"));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Informe a URL de pelo menos um repositório",
    );
    fireEvent.change(url(1), { target: { value: "faturamento" } });
    expect(screen.getByText(/Não reconheci esta URL/)).toBeInTheDocument();
    fireEvent.click(botao("Clonar e continuar"));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Repositório 1: não reconheci a URL de clone",
    );

    fireEvent.change(url(1), {
      target: { value: "https://gitlab.empresa.com.br/fin/web.git" },
    });
    fireEvent.click(
      await screen.findByRole("button", {
        name: "Prefiro um comando que imprime o token de gitlab.empresa.com.br",
      }),
    );
    fireEvent.click(botao("Clonar e continuar"));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "informe o comando",
    );
    fireEvent.change(
      screen.getByLabelText("Comando que imprime o token de web"),
      {
        target: { value: "pass show gitlab | head -1" },
      },
    );
    fireEvent.click(botao("Clonar e continuar"));
    await waitFor(() =>
      expect(api.clonarRepositorios).toHaveBeenCalledWith(
        [
          expect.objectContaining({
            token: "comando",
            comando_credencial: "pass show gitlab | head -1",
          }),
        ],
        PASTA_CLONES,
        expect.any(Function),
      ),
    );
    expect(api.clonarRepositorios).not.toHaveBeenCalledTimes(2);
  });
});

describe("Assistente de projeto: passo 2, o que será lido", () => {
  it("mostra a prévia sozinha ao entrar e destaca os possíveis segredos", async () => {
    const api = apiConfiguracoesFalsa();
    api.previaProjeto.mockImplementation((p) =>
      Promise.resolve({
        id: "faturamento-exemplo",
        nome: p.nome,
        pastas: [],
        exclusoes: p.exclusoes,
        inclusoes: p.inclusoes,
        inclusoesVistas: [],
        jaCadastrado: false,
        inventario: inventarioFicticio({
          analisados: 1284,
          porMotivo: [
            { chave: "dependencia", rotulo: "Dependências", quantidade: 3870 },
            { chave: "segredo", rotulo: "possível segredo", quantidade: 1 },
          ],
          ignorados: 3871,
          itensIgnorados: [
            ...inventarioFicticio().itensIgnorados,
            {
              caminho: "docs/manual.pdf",
              pasta: false,
              arquivos: 1,
              motivo: "binario",
              rotulo: "binário",
              regra: null,
              ajustavel: true,
            },
          ],
          gruposIgnorados: [
            ...inventarioFicticio().gruposIgnorados,
            {
              pasta: "docs",
              tipo: "pdf",
              motivo: "binario",
              rotulo: "binário",
              itens: 1,
              arquivos: 1,
              exemplos: ["docs/manual.pdf"],
              padroes: ["/docs/manual.pdf"],
              ajustavel: true,
            },
          ],
        }),
      }),
    );
    montar(api);
    await ateOPasso2();
    // Nenhum botão de "ver prévia": ela veio sozinha.
    expect(api.previaProjeto).toHaveBeenCalledTimes(1);
    expect(
      screen.queryByRole("button", { name: /Ver o que será analisado/ }),
    ).toBeNull();
    const previa = within(
      screen.getByRole("region", { name: "Prévia do inventário" }),
    );
    expect(previa.getByText("1.284")).toBeInTheDocument();
    expect(
      previa.getByText(/arquivos serão analisados · 46,9 KB/),
    ).toBeInTheDocument();
    expect(previa.getByText("SQL")).toBeInTheDocument();
    expect(previa.getByText("Dependências")).toBeInTheDocument();
    const segredos = within(
      previa.getByRole("group", { name: "Podem conter segredos" }),
    );
    expect(
      segredos.getByText(/ficou fora por parecer ter senhas ou tokens/),
    ).toBeInTheDocument();

    fireEvent.click(
      segredos.getByRole("button", { name: "Incluir .env na análise" }),
    );
    await waitFor(() =>
      expect(api.previaProjeto).toHaveBeenLastCalledWith(
        expect.objectContaining({ inclusoes: ["/.env"] }),
      ),
    );

    // O ajuste fino fica recolhido.
    const ajustar = botao("Ajustar o que fica fora · avançado");
    expect(ajustar).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByLabelText(/Excluir também/)).not.toBeVisible();
    fireEvent.click(ajustar);
    expect(ajustar).toHaveAttribute("aria-expanded", "true");
    expect(botao("Incluir docs/manual.pdf na análise")).toBeInTheDocument();
    expect(
      within(screen.getByRole("group", { name: "Reincluídos" })).getByText(
        "/.env",
      ),
    ).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText(/Excluir também/), {
      target: { value: "cobol/\n\n*.log" },
    });
    await waitFor(() =>
      expect(api.previaProjeto).toHaveBeenLastCalledWith(
        expect.objectContaining({
          exclusoes: ["cobol/", "*.log"],
          inclusoes: ["/.env"],
        }),
      ),
    );
    await waitFor(() => expect(botao("Continuar")).toBeEnabled());
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", {
      name: "Conectar as demandas (opcional)",
    });
  });

  it("não segue com um projeto de mesmo nome e mostra a recusa da prévia", async () => {
    const api = apiConfiguracoesFalsa();
    api.previaProjeto
      .mockRejectedValueOnce(
        "pasta não encontrada: /codigo/faturamento-exemplo",
      )
      .mockImplementation((p) =>
        Promise.resolve({
          id: "faturamento-exemplo",
          nome: p.nome,
          pastas: [],
          exclusoes: [],
          inclusoes: [],
          inclusoesVistas: [],
          jaCadastrado: true,
          inventario: inventarioFicticio(),
        }),
      );
    montar(api);
    await escolherPasta();
    fireEvent.click(botao("Continuar"));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "pasta não encontrada",
    );
    fireEvent.click(botao("Tentar de novo"));
    expect(
      await screen.findByText(/Já existe um projeto cadastrado/),
    ).toBeInTheDocument();
    fireEvent.click(botao("Continuar"));
    expect(titulo("É isto que a IA vai ler")).toBeInTheDocument();
  });
});

describe("Assistente de projeto: passo 3, demandas", () => {
  it("pode ser pulado", async () => {
    const api = apiConfiguracoesFalsa();
    montar(api);
    await ateOPasso3();
    expect(screen.getByText(/As pastas do passo 1/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("radio", { name: /Jira/ }));
    fireEvent.click(botao("Pular esta etapa"));
    await screen.findByRole("heading", { name: "Tudo certo?" });
    expect(
      screen.getByText("Nenhuma (demandas criadas no app)"),
    ).toBeInTheDocument();
  });

  it("usa as issues do GitHub de um repositório do passo 1", async () => {
    const api = apiConfiguracoesFalsa();
    api.detectarRepositorios.mockResolvedValue(DETECTADOS);
    montar(api);
    await escolherPasta();
    await screen.findByText(/Encontrei 2 repositórios/);
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("region", { name: "Prévia do inventário" });
    fireEvent.click(botao("Continuar"));
    expect(
      await screen.findByText(
        "GitHub · acme/faturamento-api e GitLab · fin/faturamento-web",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Vem dos repositórios do passo 1, com os mesmos tokens/),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("radio", { name: /GitHub Issues/ }));
    const select = screen.getByLabelText("Repositório das issues");
    expect(select).toHaveValue("github.com/acme/faturamento-api");
    expect(
      screen.getByText("Usa o mesmo token do repositório escolhido."),
    ).toBeInTheDocument();
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", { name: "Tudo certo?" });
    expect(
      screen.getByText("GitHub Issues · acme/faturamento-api"),
    ).toBeInTheDocument();
    fireEvent.click(botao("Cadastrar o projeto"));
    await waitFor(() =>
      expect(api.cadastrarProjeto).toHaveBeenCalledWith(
        expect.objectContaining({
          repositorios: expect.objectContaining({
            tracker: { kind: "github", repo: "acme/faturamento-api" },
          }),
        }),
      ),
    );
  });

  it("Testar conexão no passo Demandas: issues desligadas e depois ok", async () => {
    const api = apiConfiguracoesFalsa();
    api.detectarRepositorios.mockResolvedValue(DETECTADOS);
    const passo = (situacao: "ok" | "falhou", detalhe: string | null) => ({
      titulo: "Issues (GitHub)",
      passos: [
        {
          id: "issues",
          rotulo: "Repositório com as issues ligadas",
          situacao,
          detalhe,
          orientacao:
            situacao === "falhou" ? "Ligue as Issues do repositório." : null,
        },
      ],
    });
    api.testarDemandas
      .mockResolvedValueOnce(
        passo("falhou", "as issues estão desligadas em acme/faturamento-api"),
      )
      .mockResolvedValueOnce(passo("ok", null));
    montar(api);
    await escolherPasta();
    await screen.findByText(/Encontrei 2 repositórios/);
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("region", { name: "Prévia do inventário" });
    fireEvent.click(botao("Continuar"));
    fireEvent.click(
      await screen.findByRole("radio", { name: /GitHub Issues/ }),
    );
    fireEvent.click(botao("Testar conexão"));
    await waitFor(() =>
      expect(api.testarDemandas).toHaveBeenCalledWith(
        { kind: "github", repo: "acme/faturamento-api" },
        null,
      ),
    );
    expect(
      await screen.findByText(/As issues estão desligadas nesse repositório/),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Ligue as Issues do repositório."),
    ).toBeInTheDocument();
    fireEvent.click(botao("Testar conexão"));
    expect(
      await screen.findByText(/Conexão com GitHub funcionando/),
    ).toBeInTheDocument();
  });

  it("usa as issues do GitLab com o mesmo token do repositório (comando vazio)", async () => {
    const api = apiConfiguracoesFalsa();
    api.detectarRepositorios.mockResolvedValue(DETECTADOS);
    montar(api);
    await escolherPasta();
    await screen.findByText(/Encontrei 2 repositórios/);
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("region", { name: "Prévia do inventário" });
    fireEvent.click(botao("Continuar"));
    fireEvent.click(
      await screen.findByRole("radio", { name: /GitLab Issues/ }),
    );
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", { name: "Tudo certo?" });
    fireEvent.click(botao("Cadastrar o projeto"));
    await waitFor(() =>
      expect(api.cadastrarProjeto).toHaveBeenCalledWith(
        expect.objectContaining({
          repositorios: expect.objectContaining({
            tracker: {
              kind: "gitlab",
              host: "gitlab.empresa.com.br",
              path: "fin/faturamento-web",
              clone_url:
                "https://gitlab.empresa.com.br/fin/faturamento-web.git",
              credential_cmd: "",
            },
          }),
        }),
      ),
    );
  });

  it("conecta o Jira com URL, chave e token, testa a conexão e guarda o token", async () => {
    const api = apiConfiguracoesFalsa();
    api.testarJira.mockResolvedValue({
      titulo: "Jira",
      passos: [
        {
          id: "acesso",
          rotulo: "Acesso ao Jira",
          situacao: "ok",
          detalhe: null,
          orientacao: null,
        },
        {
          id: "sprint",
          rotulo: "Campo da sprint",
          situacao: "falhou",
          detalhe: "não encontrado",
          orientacao: "Informe o campo em Mais opções.",
        },
      ],
    });
    montar(api);
    await ateOPasso3();
    fireEvent.click(screen.getByRole("radio", { name: /Jira/ }));
    fireEvent.change(screen.getByLabelText("Endereço do Jira"), {
      target: { value: "https://jira.empresa.com.br" },
    });
    fireEvent.change(screen.getByLabelText("Chave do projeto"), {
      target: { value: "fat" },
    });
    expect(screen.getByLabelText("Chave do projeto")).toHaveValue("FAT");
    expect(
      screen.getByLabelText("Usar só as issues da sprint ativa"),
    ).toBeChecked();

    // Sem token, não segue.
    await waitFor(() => expect(api.tokensGuardados).toHaveBeenCalled());
    fireEvent.click(botao("Continuar"));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Informe o token do Jira",
    );

    fireEvent.change(screen.getByLabelText("Token do Jira"), {
      target: { value: "segredo" },
    });
    expect(screen.getByText(/Fica no chaveiro do sistema/)).toBeInTheDocument();
    fireEvent.click(botao("Testar conexão"));
    await waitFor(() =>
      expect(api.testarJira).toHaveBeenCalledWith(
        expect.objectContaining({
          url: "https://jira.empresa.com.br",
          project_key: "FAT",
          active_sprint_only: true,
          credential_cmd: "",
        }),
        "segredo",
      ),
    );
    expect(await screen.findByText("Acesso ao Jira")).toBeInTheDocument();
    expect(
      screen.getByText("Informe o campo em Mais opções."),
    ).toBeInTheDocument();

    // JQL e campos ficam em "Mais opções".
    expect(screen.getByLabelText("Campo do Epic Link")).not.toBeVisible();
    const mais = botao(/Mais opções do Jira/);
    expect(mais).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(mais);
    fireEvent.change(screen.getByLabelText("Campo da sprint"), {
      target: { value: "customfield_10600" },
    });

    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", { name: "Tudo certo?" });
    expect(
      screen.getByText("Jira · projeto FAT, sprint ativa"),
    ).toBeInTheDocument();
    expect(api.salvarToken).not.toHaveBeenCalled();
    fireEvent.click(botao("Cadastrar o projeto"));
    await waitFor(() => expect(api.cadastrarProjeto).toHaveBeenCalled());
    expect(api.salvarToken).toHaveBeenCalledWith(
      { host: "jira.empresa.com.br", caminho: null },
      "segredo",
    );
    expect(
      api.cadastrarProjeto.mock.calls[0]![0].repositorios?.tracker,
    ).toEqual(
      expect.objectContaining({
        kind: "jira",
        project_key: "FAT",
        sprint_field: "customfield_10600",
        credential_cmd: "",
      }),
    );
  });
});

describe("Assistente de projeto: passo 4, revisar", () => {
  it("resume o cadastro, volta pelo Editar e cadastra", async () => {
    const api = apiConfiguracoesFalsa();
    montar(api);
    await ateOPasso3();
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", { name: "Tudo certo?" });
    expect(
      screen.getByText("Código", { selector: "dt" }).nextSibling,
    ).toHaveTextContent("Pasta existente");
    expect(
      screen.getByText("Nome", { selector: "dt" }).nextSibling,
    ).toHaveTextContent("Faturamento Exemplo");
    expect(
      screen.getByText("O que será lido", { selector: "dt" }).nextSibling,
    ).toHaveTextContent("14 arquivos (46,9 KB) · 1 arquivo fora");
    expect(
      screen.getByText(/O app só lê as pastas do código/),
    ).toBeInTheDocument();

    fireEvent.click(botao("Editar nome"));
    expect(titulo("De onde vem o código do projeto?")).toHaveFocus();
    fireEvent.change(screen.getByLabelText("Nome do projeto"), {
      target: { value: "Faturamento Novo" },
    });
    fireEvent.click(botao("Continuar"));
    await waitFor(() =>
      expect(api.previaProjeto).toHaveBeenLastCalledWith(
        expect.objectContaining({ nome: "Faturamento Novo" }),
      ),
    );
    await screen.findByRole("region", { name: "Prévia do inventário" });
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", {
      name: "Conectar as demandas (opcional)",
    });
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", { name: "Tudo certo?" });
    fireEvent.click(botao("Editar o que será lido"));
    expect(titulo("É isto que a IA vai ler")).toBeInTheDocument();
    fireEvent.click(botao("Continuar"));
    fireEvent.click(await screen.findByRole("button", { name: "Continuar" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Cadastrar o projeto" }),
    );
    await waitFor(() =>
      expect(api.cadastrarProjeto).toHaveBeenCalledWith(
        expect.objectContaining({ nome: "Faturamento Novo", pastas: [PASTA] }),
      ),
    );
  });

  it("mostra a recusa do cadastro", async () => {
    const api = apiConfiguracoesFalsa();
    api.cadastrarProjeto.mockRejectedValue("a pasta sumiu");
    montar(api);
    await ateOPasso3();
    fireEvent.click(botao("Continuar"));
    fireEvent.click(
      await screen.findByRole("button", { name: "Cadastrar o projeto" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("a pasta sumiu");
    expect(titulo("Tudo certo?")).toBeInTheDocument();
  });
});

describe("Assistente de projeto: passo 5, concluído", () => {
  it("leva ao preparo do ambiente do Claude ou deixa para depois", async () => {
    const { aoPrepararAmbiente, aoConcluir } = montar();
    await ateOPasso3();
    fireEvent.click(botao("Continuar"));
    fireEvent.click(
      await screen.findByRole("button", { name: "Cadastrar o projeto" }),
    );
    const concluido = await screen.findByRole("heading", {
      name: "“Faturamento Exemplo” está cadastrado",
    });
    expect(concluido).toHaveFocus();
    expect(
      screen.getByText(/14 arquivos inventariados em 1 pasta/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Falta um passo: deixar o Claude conversar com o app/),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("navigation", { name: "Etapas do cadastro" }),
    ).toBeNull();
    fireEvent.click(botao("Preparar o ambiente do Claude"));
    expect(aoPrepararAmbiente).toHaveBeenCalledWith(
      expect.objectContaining({ nome: "Faturamento Exemplo" }),
    );
    expect(aoConcluir).not.toHaveBeenCalled();
  });
});

describe("Assistente de projeto: ajustes do teste de uso (#224)", () => {
  function url(n: number) {
    return screen.getByLabelText(`URL do repositório ${n}`);
  }

  async function escolherClonar(api: Api) {
    fireEvent.click(
      screen.getByRole("radio", { name: /Clonar do GitHub ou GitLab/ }),
    );
    await screen.findByDisplayValue(PASTA_CLONES);
    return api;
  }

  it("sugere o nome pelo primeiro repositório, não pela pasta dos clones", async () => {
    const api = apiConfiguracoesFalsa();
    montar(api);
    await escolherClonar(api);
    // O analista escolhe uma pasta chamada "Clones" (a pasta-mãe).
    fireEvent.change(screen.getByLabelText("Pasta dos repositórios"), {
      target: { value: "/Users/analista/Clones" },
    });
    expect(screen.getByLabelText("Nome do projeto")).toHaveValue("");
    fireEvent.change(url(1), {
      target: { value: "https://github.com/acme/faturamento-exemplo.git" },
    });
    expect(screen.getByLabelText("Nome do projeto")).toHaveValue(
      "Faturamento Exemplo",
    );
    await act(async () => undefined);
  });

  it("recusa o nome repetido no próprio campo, no passo 1", async () => {
    const api = apiConfiguracoesFalsa();
    api.nomeEmUso.mockImplementation((nome) =>
      Promise.resolve(nome === "Faturamento Exemplo"),
    );
    montar(api);
    await escolherPasta();
    const aviso = await screen.findByText(
      /Já existe um projeto cadastrado com o nome “Faturamento Exemplo”/,
    );
    expect(aviso).toBeInTheDocument();
    expect(screen.getByLabelText("Nome do projeto")).toBeInvalid();
    fireEvent.click(botao("Continuar"));
    await waitFor(() => expect(api.nomeEmUso).toHaveBeenCalledTimes(2));
    expect(titulo("De onde vem o código do projeto?")).toBeInTheDocument();
    expect(screen.getByLabelText("Nome do projeto")).toHaveFocus();
    expect(api.previaProjeto).not.toHaveBeenCalled();

    fireEvent.change(screen.getByLabelText("Nome do projeto"), {
      target: { value: "Faturamento Novo" },
    });
    await waitFor(() =>
      expect(
        screen.queryByText(/Já existe um projeto cadastrado/),
      ).not.toBeInTheDocument(),
    );
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("region", { name: "Prévia do inventário" });
  });

  it("Cancelar interrompe o clone e deixa tentar de novo, sem mostrar falha", async () => {
    const api = apiConfiguracoesFalsa();
    let terminar: (r: ResultadoClone[]) => void = () => undefined;
    api.clonarRepositorios.mockImplementationOnce(
      (repos) =>
        new Promise((resolver) => {
          terminar = resolver;
          void repos;
        }),
    );
    api.cancelarClone.mockImplementation(() => {
      terminar([
        {
          repositorio: {
            forja: "github",
            host: "github.com",
            caminho: "acme/faturamento-api",
          } as never,
          erro: "clone cancelado",
          cancelado: true,
        },
      ]);
      return Promise.resolve(true);
    });
    montar(api);
    await escolherClonar(api);
    fireEvent.change(url(1), {
      target: { value: "https://github.com/acme/faturamento-api.git" },
    });
    // Sem clone em andamento não há o que cancelar.
    expect(
      screen.queryByRole("button", { name: "Cancelar clone" }),
    ).toBeNull();
    fireEvent.click(botao("Clonar e continuar"));
    await waitFor(() => expect(api.clonarRepositorios).toHaveBeenCalled());
    const cancelar = await screen.findByRole("button", {
      name: "Cancelar clone",
    });
    fireEvent.click(cancelar);
    expect(api.cancelarClone).toHaveBeenCalledTimes(1);
    expect(
      await screen.findByText(/Clone cancelado\. A pasta que ele tinha começado/),
    ).toBeInTheDocument();
    // Não é falha: sem alerta, sem barra, e dá para clonar de novo.
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(titulo("De onde vem o código do projeto?")).toBeInTheDocument();
    expect(botao("Clonar e continuar")).toBeEnabled();
    expect(
      screen.queryByRole("button", { name: /Cancelar clone|Cancelando/ }),
    ).toBeNull();
  });

  it("Voltar ao Revisar depois de Editar, sem refazer os passos 2 e 3", async () => {
    const api = apiConfiguracoesFalsa();
    montar(api);
    await ateOPasso3();
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", { name: "Tudo certo?" });
    // O atalho só existe depois de um "Editar".
    expect(screen.queryByRole("button", { name: "Voltar ao Revisar" })).toBeNull();

    fireEvent.click(botao("Editar nome"));
    expect(titulo("De onde vem o código do projeto?")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Nome do projeto"), {
      target: { value: "Faturamento Novo" },
    });
    fireEvent.click(botao("Voltar ao Revisar"));
    await screen.findByRole("heading", { name: "Tudo certo?" });
    // Já com o nome novo e a prévia refeita para ele.
    expect(
      screen.getByText("Nome", { selector: "dt" }).nextSibling,
    ).toHaveTextContent("Faturamento Novo");
    await waitFor(() =>
      expect(api.previaProjeto).toHaveBeenLastCalledWith(
        expect.objectContaining({ nome: "Faturamento Novo" }),
      ),
    );
    await waitFor(() => expect(botao("Cadastrar o projeto")).toBeEnabled());
    expect(screen.queryByRole("button", { name: "Voltar ao Revisar" })).toBeNull();

    // De novo, agora pelo passo 2.
    fireEvent.click(botao("Editar o que será lido"));
    expect(titulo("É isto que a IA vai ler")).toBeInTheDocument();
    await screen.findByRole("region", { name: "Prévia do inventário" });
    fireEvent.click(botao("Voltar ao Revisar"));
    await screen.findByRole("heading", { name: "Tudo certo?" });
    fireEvent.click(botao("Cadastrar o projeto"));
    await waitFor(() =>
      expect(api.cadastrarProjeto).toHaveBeenCalledWith(
        expect.objectContaining({ nome: "Faturamento Novo" }),
      ),
    );
  });

  it("no clone, o Revisar e o resumo mostram “sem token” sem token global", async () => {
    const api = apiConfiguracoesFalsa();
    api.tokensGuardados.mockImplementation((alvos) =>
      Promise.resolve(alvos.map((a) => a.host === "gitlab.empresa.com.br")),
    );
    montar(api);
    await escolherClonar(api);
    fireEvent.change(url(1), {
      target: { value: "https://gitlab.empresa.com.br/fin/faturamento-web.git" },
    });
    fireEvent.click(botao("+ Adicionar repositório"));
    fireEvent.change(url(2), {
      target: { value: "git@github.com:acme/faturamento-api.git" },
    });
    fireEvent.click(botao("Clonar e continuar"));
    await screen.findByRole("heading", { name: "É isto que a IA vai ler" });
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", { name: "Conectar as demandas (opcional)" });
    fireEvent.click(botao("Continuar"));
    await screen.findByRole("heading", { name: "Tudo certo?" });
    const tokens = screen.getByText("Tokens", { selector: "dt" }).nextSibling;
    await waitFor(() =>
      expect(tokens).toHaveTextContent(
        "faturamento-web: global do GitLab · faturamento-api: sem token",
      ),
    );

    // O resumo do passo 1 diz o mesmo.
    fireEvent.click(botao("Editar código"));
    const resumo = within(
      await screen.findByRole("group", { name: "Repositórios clonados" }),
    );
    await waitFor(() =>
      expect(resumo.getByText(/branch padrão · sem token/)).toBeInTheDocument(),
    );
    expect(
      resumo.getByText(/token global de gitlab.empresa.com.br/),
    ).toBeInTheDocument();
  });

  it("agrupa o que fica fora por pasta e tipo, com busca, e mostra o efeito da reinclusão", async () => {
    const api = apiConfiguracoesFalsa();
    const icones = Array.from({ length: 40 }, (_, i) => ({
      pasta: `client/icons/pacote-${i}`,
      tipo: "png",
      motivo: "binario",
      rotulo: "binário",
      itens: 25,
      arquivos: 25,
      exemplos: [`client/icons/pacote-${i}/a.png`],
      padroes: [`/client/icons/pacote-${i}/*.png`],
      ajustavel: true,
    }));
    const outros = [
      {
        pasta: "docs",
        tipo: "pdf",
        motivo: "binario",
        rotulo: "binário",
        itens: 12,
        arquivos: 12,
        exemplos: ["docs/a.pdf"],
        padroes: ["/docs/*.pdf"],
        ajustavel: true,
      },
      {
        pasta: "",
        tipo: "pasta",
        motivo: "usuario",
        rotulo: "excluído pelo analista",
        itens: 1,
        arquivos: 18,
        exemplos: ["test"],
        padroes: ["/test"],
        ajustavel: true,
      },
    ];
    api.previaProjeto.mockImplementation((p) =>
      Promise.resolve({
        id: "faturamento-exemplo",
        nome: p.nome,
        pastas: [],
        exclusoes: p.exclusoes,
        inclusoes: p.inclusoes,
        // A pasta "test" tem 18 arquivos; os PNG continuam binários pelo conteúdo.
        inclusoesVistas: p.inclusoes.map((padrao) => ({
          padrao,
          entram: padrao === "/test" ? 18 : 0,
          continuamFora: padrao === "/test" ? 0 : 25,
        })),
        jaCadastrado: false,
        inventario: inventarioFicticio({
          ignorados: 1000 + 12 + 18,
          totalItensIgnorados: 1000 + 12 + 1,
          gruposIgnorados: [...icones, ...outros],
          porMotivo: [
            { chave: "binario", rotulo: "binário", quantidade: 1012 },
            { chave: "usuario", rotulo: "excluído pelo analista", quantidade: 18 },
          ],
        }),
      }),
    );
    montar(api);
    await ateOPasso2();
    const previa = within(
      screen.getByRole("region", { name: "Prévia do inventário" }),
    );
    // Tudo em arquivos: a pasta excluída conta os 18 arquivos dela.
    expect(previa.getByText(/arquivos ficam fora/)).toBeInTheDocument();
    expect(previa.getByText("1.030")).toBeInTheDocument();
    expect(previa.getByText("18")).toBeInTheDocument();

    fireEvent.click(botao("Ajustar o que fica fora · avançado"));
    const lista = within(
      screen.getByRole("group", { name: "Itens fora que você pode incluir" }),
    );
    // 42 grupos em vez de 1.000 itens soltos (e nada de "E mais N itens").
    expect(lista.getByText("42 grupos (por pasta e tipo).")).toBeInTheDocument();
    expect(lista.getByText("25 arquivos .png em client/icons/pacote-3/")).toBeInTheDocument();
    expect(lista.queryByText(/E mais/)).toBeNull();

    // A busca acha por pasta ou tipo.
    fireEvent.change(lista.getByLabelText("Buscar por pasta ou tipo"), {
      target: { value: "pdf" },
    });
    expect(lista.getByText("12 arquivos .pdf em docs/")).toBeInTheDocument();
    expect(lista.getAllByRole("listitem")).toHaveLength(1);
    fireEvent.change(lista.getByLabelText("Buscar por pasta ou tipo"), {
      target: { value: "nada disso" },
    });
    expect(lista.getByText("Nada encontrado com essa busca.")).toBeInTheDocument();
    fireEvent.change(lista.getByLabelText("Buscar por pasta ou tipo"), {
      target: { value: "" },
    });

    // Incluir um grupo manda os padrões dele; o efeito aparece em "Incluídos por você".
    fireEvent.click(
      lista.getByRole("button", { name: "Incluir test/ na análise" }),
    );
    const incluirPng = lista.getByRole("button", {
      name: "Incluir 25 arquivos .png em client/icons/pacote-0/ na análise",
    });
    // Enquanto a prévia é refeita, os botões esperam.
    await waitFor(() => expect(incluirPng).toBeEnabled());
    fireEvent.click(incluirPng);
    await waitFor(() =>
      expect(api.previaProjeto).toHaveBeenLastCalledWith(
        expect.objectContaining({
          inclusoes: ["/test", "/client/icons/pacote-0/*.png"],
        }),
      ),
    );
    const incluidos = within(screen.getByRole("group", { name: "Reincluídos" }));
    expect(
      await incluidos.findByText(/18 arquivos entram na análise/),
    ).toBeInTheDocument();
    expect(
      incluidos.getByText(/continua fora: 25 arquivos têm conteúdo binário/),
    ).toBeInTheDocument();
  });
});
