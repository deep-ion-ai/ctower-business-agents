import { act } from "react";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { PainelAnalise, type Abertura } from "./PainelAnalise";
import type { VistaAnalise } from "./analise";
import type { ApiRevisao, Prontidao } from "./revisao";
import type { OpcoesConversa } from "./conversa";
import { INTERVALO_DAS_EVIDENCIAS_MS } from "./useAnaliseAoVivo";
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

function montar(
  abertura: Abertura,
  vista?: VistaAnalise,
  opcoes?: OpcoesConversa,
  abrirUrl?: (url: string) => Promise<void>,
  apiRevisao?: ApiRevisao,
) {
  const analise = apiAnaliseFalsa(vista);
  const terminal = apiTerminalFalsa();
  // Como o Rust: a sessão da análise vem nas opções (conversa gravada, retomável).
  const conversa = apiConversaFalsa(
    opcoes ??
      (vista?.sessao
        ? opcoesFicticias({ sessao: vista.sessao.id })
        : undefined),
  );
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
      abrirUrl={abrirUrl}
      apiRevisao={apiRevisao}
      apiPublicacao={{
        previa: () => new Promise(() => {}),
        publicar: () => new Promise(() => {}),
      }}
    />,
  );
  return { ...analise, terminal, conversa, aoVoltar, tela, ...r };
}

/** Abre o menu "continuar a conversa" da barra e devolve o painel. */
function abrirMenu() {
  const botao = screen.getByRole("button", { name: "Continuar a conversa" });
  if (botao.getAttribute("aria-expanded") !== "true") fireEvent.click(botao);
  return screen.getByRole("region", { name: "Onde continuar a conversa" });
}

async function terminalAberto(t: ReturnType<typeof apiTerminalFalsa>, id = 1) {
  await waitFor(() => expect(t.aberturas.length).toBeGreaterThan(0));
  await act(async () => t.aberturas[t.aberturas.length - 1].resolver(id));
}

/** O seletor "Mostrar evidências de código" ligado, como guardado pelo app (#404). */
function mostrarEvidencias() {
  localStorage.setItem("cba.evidenciasVisiveis", "1");
}

beforeEach(() => {
  localStorage.clear();
});

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
    expect(
      await screen.findByRole("heading", { name: "A-0001: Multa por atraso" }),
    ).toBeVisible();
  });

  it("atualiza o documento ao vivo a cada aviso sobre a análise", async () => {
    const { api, avisar } = montar({ tipo: "reabrir" });
    expect(
      await screen.findByText("Nenhuma afirmação registrada ainda."),
    ).toBeInTheDocument();
    expect(api.vigiar).toHaveBeenCalledWith(
      "faturamento-exemplo",
      expect.any(Function),
    );
    expect(api.carregarAnalise).toHaveBeenCalledTimes(1);

    // Aviso de outra análise, ou de versão já mostrada, não relê.
    act(() =>
      avisar({
        tipo: "mudanca",
        analise: "A-0002",
        mudanca: "afirmacao",
        versao: 5,
      }),
    );
    act(() =>
      avisar({
        tipo: "mudanca",
        analise: "A-0001",
        mudanca: "afirmacao",
        versao: 1,
      }),
    );
    expect(api.carregarAnalise).toHaveBeenCalledTimes(1);

    api.carregarAnalise.mockResolvedValue(
      vistaFicticia({}, { versao: 2, afirmacoes: [AFIRMACAO_MULTA] }),
    );
    act(() =>
      avisar({
        tipo: "mudanca",
        analise: "A-0001",
        mudanca: "afirmacao",
        versao: 2,
      }),
    );
    expect(await screen.findByText(AFIRMACAO_MULTA.texto)).toBeInTheDocument();
    expect(screen.getByText("com evidência")).toHaveAttribute("data-tone", "done");
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

  it("confere as evidências a cada poucos segundos e mostra e tira a faixa do código alterado", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const revalidar = {
        ...AFIRMACAO_MULTA,
        estado: "a-revalidar" as const,
        evidencias: [{ ...AFIRMACAO_MULTA.evidencias[0], estado: "a-revalidar" as const }],
      };
      const { api, unmount } = montar(
        { tipo: "reabrir" },
        vistaFicticia({}, { versao: 1, afirmacoes: [AFIRMACAO_MULTA] }),
      );
      expect(await screen.findByText(AFIRMACAO_MULTA.texto)).toBeInTheDocument();
      expect(api.conferirEvidencias).toHaveBeenCalledWith("faturamento-exemplo", "A-0001");
      expect(screen.queryByRole("region", { name: "Código alterado" })).not.toBeInTheDocument();

      // O analista edita o arquivo citado: a próxima conferência marca a afirmação.
      api.carregarAnalise.mockResolvedValue(
        vistaFicticia({}, { versao: 2, afirmacoes: [revalidar] }),
      );
      api.conferirEvidencias.mockResolvedValue(true);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(INTERVALO_DAS_EVIDENCIAS_MS);
      });
      const faixa = await screen.findByRole("region", { name: "Código alterado" });
      expect(within(faixa).getByText("db/03_pkg_cobranca.sql")).toBeInTheDocument();
      expect(within(faixa).getByText(/afirmação 1/)).toBeInTheDocument();

      // Desfeita a edição, a marca sai e a faixa some.
      api.carregarAnalise.mockResolvedValue(
        vistaFicticia({}, { versao: 3, afirmacoes: [AFIRMACAO_MULTA] }),
      );
      await act(async () => {
        await vi.advanceTimersByTimeAsync(INTERVALO_DAS_EVIDENCIAS_MS);
      });
      await waitFor(() =>
        expect(screen.queryByRole("region", { name: "Código alterado" })).not.toBeInTheDocument(),
      );

      // Fora da tela, não confere mais.
      unmount();
      const chamadas = api.conferirEvidencias.mock.calls.length;
      await act(async () => {
        await vi.advanceTimersByTimeAsync(3 * INTERVALO_DAS_EVIDENCIAS_MS);
      });
      expect(api.conferirEvidencias).toHaveBeenCalledTimes(chamadas);
    } finally {
      vi.useRealTimers();
    }
  });

  it("abre o trecho da evidência com as linhas citadas destacadas", async () => {
    mostrarEvidencias();
    const { api } = montar(
      { tipo: "reabrir" },
      vistaFicticia({}, { afirmacoes: [AFIRMACAO_MULTA] }),
    );
    fireEvent.click(
      await screen.findByRole("button", {
        name: "03_pkg_cobranca.sql:71-72",
      }),
    );
    expect(api.trechoEvidencia).toHaveBeenCalledWith(
      "faturamento-exemplo",
      "db/03_pkg_cobranca.sql",
      71,
      72,
    );
    const trecho = await screen.findByRole("region", {
      name: "Trecho da evidência",
    });
    await waitFor(() =>
      expect(trecho.querySelector('[data-linha="71"]')).not.toBeNull(),
    );
    const linha = (n: number) => trecho.querySelector(`[data-linha="${n}"]`);
    expect(linha(70)).not.toHaveAttribute("data-citada");
    expect(linha(71)).toHaveAttribute("data-citada", "true");
    expect(linha(72)).toHaveAttribute("data-citada", "true");
    expect(linha(73)).not.toHaveAttribute("data-citada");
    expect(linha(71)).toHaveAttribute("aria-current", "location");
    expect(
      within(linha(71) as HTMLElement).getByText("linha 71"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "03_pkg_cobranca.sql:71-72" }),
    ).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(within(trecho).getByRole("button", { name: "Fechar" }));
    expect(
      screen.queryByRole("region", { name: "Trecho da evidência" }),
    ).not.toBeInTheDocument();
  });

  it("documento à esquerda, conversa à direita; barra com id, título, estado e ações", async () => {
    montar({ tipo: "reabrir" });
    await screen.findByText("Nenhuma afirmação registrada ainda.");
    const documento = screen.getByRole("region", { name: "Documento ao vivo" });
    const conversa = screen.getByRole("region", { name: "Painel lateral" });
    // O documento vem antes na ordem (à esquerda; primeiro quando as colunas empilham).
    expect(
      documento.compareDocumentPosition(conversa) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    const barra = screen.getByRole("banner");
    expect(
      within(barra).getByRole("heading", { name: "A-0001: Multa por atraso" }),
    ).toBeVisible();
    // Sem o título repetido no documento.
    expect(
      within(documento).queryByRole("heading", { level: 2 }),
    ).not.toBeInTheDocument();
    expect(
      within(barra).getByRole("button", { name: "Exportar" }),
    ).toHaveAttribute("aria-expanded", "false");
    // O menu "Continuar a conversa" saiu da barra: vive no painel lateral (#185).
    expect(
      within(barra).queryByRole("button", { name: "Continuar a conversa" }),
    ).not.toBeInTheDocument();
    expect(
      within(conversa).getByRole("button", { name: "Continuar a conversa" }),
    ).toHaveAttribute("aria-expanded", "false");
    // Um painel por vez; Esc fecha.
    fireEvent.click(within(barra).getByRole("button", { name: "Exportar" }));
    expect(
      screen.getByRole("region", { name: "Exportar o documento" }),
    ).toBeVisible();
    // Publicar a documentação aprovada (#276), ao lado do Exportar: um painel por vez.
    fireEvent.click(within(barra).getByRole("button", { name: "Publicar" }));
    expect(
      screen.getByRole("region", { name: "Publicar a documentação no repositório" }),
    ).toBeVisible();
    expect(
      screen.queryByRole("region", { name: "Exportar o documento" }),
    ).not.toBeInTheDocument();
    fireEvent.click(within(barra).getByRole("button", { name: "Exportar" }));
    // Aprovar abre um diálogo modal; Esc fecha e devolve o foco ao botão.
    const aprovar = within(barra).getByRole("button", { name: /^Aprovar/ });
    fireEvent.click(aprovar);
    expect(
      screen.queryByRole("region", { name: "Exportar o documento" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("dialog", { name: "Aprovar a análise" }),
    ).toBeVisible();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("o chip de evidência abre a aba Código, não o meio do documento", async () => {
    mostrarEvidencias();
    montar(
      { tipo: "reabrir" },
      vistaFicticia({}, { afirmacoes: [AFIRMACAO_MULTA] }),
    );
    const abaCodigo = screen.getByRole("tab", { name: "Código" });
    expect(abaCodigo).toHaveAttribute("aria-selected", "false");
    fireEvent.click(
      await screen.findByRole("button", {
        name: "03_pkg_cobranca.sql:71-72",
      }),
    );
    const trecho = await screen.findByRole("region", {
      name: "Trecho da evidência",
    });
    expect(abaCodigo).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: "Código" })).toContainElement(
      trecho,
    );
    const afirmacao = screen.getByTestId(`afirmacao-${AFIRMACAO_MULTA.numero}`);
    expect(afirmacao).not.toContainElement(trecho);
    // Caminho do arquivo e linhas citadas.
    expect(within(trecho).getByText("db/03_pkg_cobranca.sql")).toBeVisible();
    expect(within(trecho).getByText(/linhas 71-72/)).toBeVisible();
    // Voltar à conversa não perde o trecho.
    fireEvent.click(screen.getByRole("tab", { name: "Conversa com o agente" }));
    expect(
      document.getElementById(abaCodigo.getAttribute("aria-controls")!),
    ).not.toBeVisible();
    fireEvent.click(abaCodigo);
    expect(
      screen.getByRole("region", { name: "Trecho da evidência" }),
    ).toBeVisible();
    // Fechar o trecho volta à conversa.
    fireEvent.click(within(trecho).getByRole("button", { name: "Fechar" }));
    expect(
      screen.getByRole("tab", { name: "Conversa com o agente" }),
    ).toHaveAttribute("aria-selected", "true");
  });

  it("anterior e próxima navegam entre as evidências da mesma afirmação", async () => {
    mostrarEvidencias();
    const segunda = {
      arquivo: "db/03_pkg_cobranca.sql",
      linhas: "90-91",
      trecho: "x",
      hash: "def",
      estado: "valida" as const,
    };
    const { api } = montar(
      { tipo: "reabrir" },
      vistaFicticia(
        {},
        {
          afirmacoes: [
            {
              ...AFIRMACAO_MULTA,
              evidencias: [...AFIRMACAO_MULTA.evidencias, segunda],
            },
          ],
        },
      ),
    );
    fireEvent.click(
      await screen.findByRole("button", {
        name: "03_pkg_cobranca.sql:71-72",
      }),
    );
    const trecho = await screen.findByRole("region", {
      name: "Trecho da evidência",
    });
    expect(within(trecho).getByText("1 de 2")).toBeInTheDocument();
    expect(
      within(trecho).getByRole("button", { name: /Anterior/ }),
    ).toBeDisabled();
    fireEvent.click(within(trecho).getByRole("button", { name: /Próxima/ }));
    await waitFor(() =>
      expect(api.trechoEvidencia).toHaveBeenLastCalledWith(
        "faturamento-exemplo",
        "db/03_pkg_cobranca.sql",
        90,
        91,
      ),
    );
    const novo = await screen.findByRole("region", {
      name: "Trecho da evidência",
    });
    await waitFor(() =>
      expect(novo.querySelector('[data-linha="90"]')).toHaveAttribute(
        "data-citada",
      ),
    );
    expect(within(novo).getByText("2 de 2")).toBeInTheDocument();
    expect(
      within(novo).getByRole("button", { name: /Próxima/ }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "03_pkg_cobranca.sql:90-91" }),
    ).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(within(novo).getByRole("button", { name: /Anterior/ }));
    await waitFor(() =>
      expect(within(novo).getByText("1 de 2")).toBeInTheDocument(),
    );
  });

  it("abre as linhas citadas na forja do repositório", async () => {
    mostrarEvidencias();
    const abrirUrl = vi.fn(() => Promise.resolve());
    const { api } = montar(
      { tipo: "reabrir" },
      vistaFicticia({}, { afirmacoes: [AFIRMACAO_MULTA] }),
      undefined,
      abrirUrl,
    );
    api.linkForja.mockResolvedValue({
      url: "https://gitlab.example.com/g/p/-/blob/abc/db/03_pkg_cobranca.sql#L71-72",
      forja: "gitlab",
    });
    fireEvent.click(
      await screen.findByRole("button", {
        name: "03_pkg_cobranca.sql:71-72",
      }),
    );
    fireEvent.click(
      await screen.findByRole("button", { name: /Abrir no GitLab/ }),
    );
    expect(api.linkForja).toHaveBeenCalledWith(
      "faturamento-exemplo",
      "db/03_pkg_cobranca.sql",
      71,
      72,
      undefined,
    );
    expect(abrirUrl).toHaveBeenCalledWith(
      "https://gitlab.example.com/g/p/-/blob/abc/db/03_pkg_cobranca.sql#L71-72",
    );
  });

  it("sem repositório de código cadastrado, a aba Código fica sem o link da forja", async () => {
    mostrarEvidencias();
    montar(
      { tipo: "reabrir" },
      vistaFicticia({}, { afirmacoes: [AFIRMACAO_MULTA] }),
    );
    fireEvent.click(
      await screen.findByRole("button", {
        name: "03_pkg_cobranca.sql:71-72",
      }),
    );
    await screen.findByRole("region", { name: "Trecho da evidência" });
    expect(
      screen.queryByRole("button", { name: /Abrir no (GitHub|GitLab)/ }),
    ).not.toBeInTheDocument();
  });

  it("as abas se navegam pelo teclado e a aba Código começa vazia", () => {
    montar({ tipo: "reabrir" });
    const conversa = screen.getByRole("tab", { name: "Conversa com o agente" });
    const codigo = screen.getByRole("tab", { name: "Código" });
    expect(conversa).toHaveAttribute("aria-selected", "true");
    expect(conversa).toHaveAttribute("tabindex", "0");
    expect(codigo).toHaveAttribute("tabindex", "-1");
    conversa.focus();
    fireEvent.keyDown(conversa, { key: "ArrowRight" });
    expect(codigo).toHaveAttribute("aria-selected", "true");
    expect(codigo).toHaveFocus();
    expect(screen.getByText(/Nenhuma evidência aberta/)).toBeVisible();
    fireEvent.keyDown(codigo, { key: "Home" });
    expect(conversa).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(conversa, { key: "End" });
    expect(codigo).toHaveAttribute("aria-selected", "true");
  });

  it("evidências ocultas por padrão: o </> do cartão mostra os chips arquivo:linhas (#404)", async () => {
    const { api } = montar(
      { tipo: "reabrir" },
      vistaFicticia({}, { afirmacoes: [AFIRMACAO_MULTA] }),
    );
    const ver = await screen.findByRole("button", {
      name: "Ver a evidência no código",
    });
    expect(
      screen.queryByRole("button", { name: "03_pkg_cobranca.sql:71-72" }),
    ).not.toBeInTheDocument();
    fireEvent.click(ver);
    const chip = screen.getByRole("button", {
      name: "03_pkg_cobranca.sql:71-72",
    });
    expect(chip).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(chip);
    expect(
      await screen.findByRole("region", { name: "Trecho da evidência" }),
    ).toBeInTheDocument();
    expect(api.trechoEvidencia).toHaveBeenCalled();
    expect(chip).toHaveAttribute("aria-pressed", "true");
    // Fechar o trecho solta o chip.
    fireEvent.click(
      within(
        screen.getByRole("region", { name: "Trecho da evidência" }),
      ).getByRole("button", {
        name: "Fechar",
      }),
    );
    expect(
      screen.queryByRole("region", { name: "Trecho da evidência" }),
    ).not.toBeInTheDocument();
    expect(chip).toHaveAttribute("aria-pressed", "false");
  });

  it("mostra a recusa de um trecho fora do escopo", async () => {
    mostrarEvidencias();
    const { api } = montar(
      { tipo: "reabrir" },
      vistaFicticia({}, { afirmacoes: [AFIRMACAO_MULTA] }),
    );
    api.trechoEvidencia.mockRejectedValue(
      "`db/03_pkg_cobranca.sql`: caminho fora das pastas do projeto",
    );
    fireEvent.click(
      await screen.findByRole("button", {
        name: "03_pkg_cobranca.sql:71-72",
      }),
    );
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
    const selo = await screen.findByRole("status", {
      name: "Estado da sessão",
    });
    expect(selo).toHaveTextContent(texto);
    expect(selo).toHaveAttribute("data-estado", estado);
  });

  describe("cabeçalho estável (#219)", () => {
    const PRONTIDAO: Prontidao = {
      estado: "concluida",
      podeAprovar: true,
      pendencias: [],
      avisos: [],
      areas: [{ id: "cobranca", nome: "Cobrança" }],
      areaObrigatoria: false,
      regras: [],
      incorretas: [],
      termosAceitos: [],
    };
    const PENDENTE: Prontidao = {
      ...PRONTIDAO,
      podeAprovar: false,
      pendencias: [
        { tipo: "afirmacao", numero: 1, motivo: "afirmação 1: sem decisão" },
      ],
    };
    const apiRevisao = (p: Prontidao): ApiRevisao => ({
      revisar: vi.fn(),
      prontidao: vi.fn(() => Promise.resolve(p)),
      aprovar: vi.fn(),
      escolherDestino: vi.fn(),
      exportar: vi.fn(),
    });
    const PRONTIDOES = {
      trabalhando: PENDENTE,
      concluida: PRONTIDAO,
      aguardandoRevisao: PENDENTE,
    };
    const ESTADOS = {
      trabalhando: () =>
        vistaFicticia(
          { sessao: { id: "s-1", estado: "ativa", cliente: "claude-code" } },
          { estado: "em-andamento", afirmacoes: [AFIRMACAO_MULTA] },
        ),
      concluida: () =>
        vistaFicticia(
          {
            sessao: { id: "s-1", estado: "concluida", cliente: "claude-code" },
          },
          {
            estado: "concluida",
            afirmacoes: [AFIRMACAO_MULTA],
          },
        ),
      aguardandoRevisao: () =>
        vistaFicticia(
          {
            sessao: {
              id: "s-1",
              estado: "aguardando-analista",
              cliente: "claude-code",
            },
          },
          {
            estado: "concluida",
            afirmacoes: [AFIRMACAO_MULTA],
          },
        ),
    };

    /** Estrutura (tag e classes) do cabeçalho, sem os textos. */
    function estrutura(el: Element): string {
      const filhos = Array.from(el.children).map(estrutura).join("");
      return `<${el.tagName.toLowerCase()} class="${el.getAttribute("class") ?? ""}">${filhos}</${el.tagName.toLowerCase()}>`;
    }

    async function cabecalho(chave: keyof typeof ESTADOS) {
      const { container, unmount } = montar(
        { tipo: "reabrir" },
        ESTADOS[chave](),
        undefined,
        undefined,
        apiRevisao(PRONTIDOES[chave]),
      );
      await screen.findByRole("status", { name: "Situação da aprovação" });
      const barra = container.querySelector(".painel-barra") as HTMLElement;
      const identidade = barra.querySelector(
        ".painel-identidade",
      ) as HTMLElement;
      const acoes = barra.querySelector(".painel-acoes") as HTMLElement;
      const resultado = {
        html: identidade.outerHTML,
        estrutura: estrutura(barra),
        botoes: Array.from(acoes.querySelectorAll("button")).map(
          (b) => b.className,
        ),
      };
      unmount();
      return resultado;
    }

    it.each(Object.keys(ESTADOS) as (keyof typeof ESTADOS)[])(
      "snapshot do cabeçalho: %s",
      async (chave) => {
        expect((await cabecalho(chave)).html).toMatchSnapshot();
      },
    );

    it("tem a mesma estrutura e as mesmas classes nos três estados", async () => {
      const a = await cabecalho("trabalhando");
      const b = await cabecalho("concluida");
      const c = await cabecalho("aguardandoRevisao");
      expect(b.botoes).toEqual(a.botoes);
      expect(c.botoes).toEqual(a.botoes);
      // Só os textos mudam: os selos ficam no mesmo lugar e com as mesmas classes.
      expect(b.estrutura).toEqual(a.estrutura);
      expect(c.estrutura).toEqual(a.estrutura);
    });

    it("os selos têm o texto completo no title (truncam em uma linha)", async () => {
      montar(
        { tipo: "reabrir" },
        ESTADOS.aguardandoRevisao(),
        undefined,
        undefined,
        apiRevisao(PENDENTE),
      );
      const selo = await screen.findByRole("status", {
        name: "Situação da aprovação",
      });
      expect(selo).toHaveAttribute("title", selo.textContent ?? "");
    });
  });

  it("sem sessão ligada, avisa que ela aparece na primeira ferramenta", async () => {
    montar({ tipo: "reabrir" });
    expect(
      await screen.findByRole("status", { name: "Estado da sessão" }),
    ).toHaveTextContent("Nenhuma sessão ligada");
  });

  it("sem prompts prontos, não mostra os botões", async () => {
    const { api, terminal } = montar({ tipo: "nova", promptInicial: "Inicie" });
    await terminalAberto(terminal);
    await waitFor(() =>
      expect(api.promptsProntos).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "A-0001",
      ),
    );
    expect(
      screen.queryByRole("group", { name: "Mais pedidos" }),
    ).not.toBeInTheDocument();
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
    fireEvent.click(
      await screen.findByRole("button", { name: "Mais pedidos" }),
    );
    const botao = await screen.findByRole("button", {
      name: "Resumir para o solicitante",
    });
    expect(botao).toBeDisabled();
    await terminalAberto(terminal, 7);
    await waitFor(() => expect(botao).toBeEnabled());
    fireEvent.click(botao);
    // O menu fecha ao enviar.
    expect(
      screen.queryByRole("group", { name: "Mais pedidos" }),
    ).not.toBeInTheDocument();
    await waitFor(() =>
      expect(terminal.api.enviarPrompt).toHaveBeenCalledWith(
        7,
        "Resuma para o solicitante.",
      ),
    );
  });

  it("continua no Desktop com o documento ao vivo e volta à conversa", async () => {
    const { terminal, conversa } = montar(
      { tipo: "nova", promptInicial: "Inicie" },
      vistaFicticia({
        sessao: { id: "sessao-1", estado: "ativa", cliente: "claude-code" },
      }),
    );
    await terminalAberto(terminal, 3);
    const continuar = await within(abrirMenu()).findByRole("button", {
      name: "Continuar no Claude Desktop",
    });
    await waitFor(() => expect(continuar).toBeEnabled());
    fireEvent.click(continuar);
    await waitFor(() =>
      expect(terminal.api.enviarPrompt).toHaveBeenCalledWith(3, "/desktop"),
    );
    expect(
      await screen.findByText(/A conversa continua no Claude Desktop/),
    ).toBeVisible();
    expect(conversa.passouAoDesktop).toHaveBeenCalledWith(
      "faturamento-exemplo",
      "A-0001",
    );
    // O terminal continua montado, escondido; a coluna mostra só o aviso e o voltar.
    expect(screen.getByTestId("terminal-tela")).toBeInTheDocument();
    expect(screen.getByTestId("terminal-tela")).not.toBeVisible();
    expect(terminal.api.encerrar).not.toHaveBeenCalled();
    expect(
      screen.queryByRole("region", { name: "Onde continuar a conversa" }),
    ).not.toBeInTheDocument();
    // O documento continua na tela.
    expect(
      screen.getByRole("article", { name: "Documento da análise" }),
    ).toBeVisible();

    // O claude saiu depois do /desktop: voltar retoma a mesma conversa aqui.
    act(() =>
      terminal.aberturas[0].aoEvento({ tipo: "encerrado", fim: { codigo: 0 } }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Voltar à conversa aqui" }),
    );
    await waitFor(() => expect(terminal.aberturas).toHaveLength(2));
    expect(terminal.aberturas[1].pedido).toEqual({
      retomada: { modo: "sessao", id: "sessao-1" },
    });
    expect(
      screen.queryByText(/A conversa continua no Claude Desktop/),
    ).not.toBeInTheDocument();
  });

  it("reabre uma análise retomando a conversa ou começando outra", async () => {
    const { terminal } = montar(
      { tipo: "reabrir" },
      vistaFicticia({
        sessao: { id: "sessao-9", estado: "encerrada", cliente: "claude-code" },
      }),
    );
    expect(terminal.api.abrir).not.toHaveBeenCalled();
    // Sem o bloco "Como continuar": a coluna só aponta para o menu da barra.
    fireEvent.click(
      await screen.findByRole("button", { name: "Escolher onde continuar" }),
    );
    expect(
      screen.queryByText("Como continuar esta análise?"),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Continuar a conversa" }),
    ).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(
      within(abrirMenu()).getByRole("button", { name: "Retomar a conversa" }),
    );
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    expect(terminal.aberturas[0].pedido).toEqual({
      retomada: { modo: "sessao", id: "sessao-9" },
    });
  });

  it("depois do fim da sessão, continuar reabre a sessão desta análise, não a última da pasta", async () => {
    const { terminal } = montar(
      { tipo: "reabrir" },
      vistaFicticia({
        sessao: { id: "sessao-9", estado: "encerrada", cliente: "claude-code" },
      }),
    );
    fireEvent.click(
      await screen.findByRole("button", { name: "Escolher onde continuar" }),
    );
    fireEvent.click(
      within(abrirMenu()).getByRole("button", { name: "Retomar a conversa" }),
    );
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    await act(async () => terminal.aberturas[0].resolver(1));
    // O `--resume` falhou (por exemplo, sessão sem transcrição) e o processo saiu.
    act(() =>
      terminal.aberturas[0].aoEvento({ tipo: "encerrado", fim: { codigo: 1 } }),
    );
    const term = await screen.findByRole("region", {
      name: "Terminal do Claude Code",
    });
    fireEvent.click(
      await within(term).findByRole("button", { name: "Continuar a conversa" }),
    );
    await waitFor(() => expect(terminal.aberturas).toHaveLength(2));
    expect(terminal.aberturas[1].pedido).toEqual({
      retomada: { modo: "sessao", id: "sessao-9" },
    });
  });

  it("sessão sem conversa gravada não é oferecida para retomar", async () => {
    const { terminal } = montar(
      { tipo: "reabrir" },
      vistaFicticia({
        sessao: { id: "f435", estado: "encerrada", cliente: "claude-code" },
      }),
      opcoesFicticias({
        sessao: null,
        pedido: "Retome a análise A-0001 pelo ctower",
      }),
    );
    fireEvent.click(
      await screen.findByRole("button", { name: "Escolher onde continuar" }),
    );
    const menu = abrirMenu();
    expect(
      within(menu).queryByRole("button", { name: "Retomar a conversa" }),
    ).not.toBeInTheDocument();
    fireEvent.click(
      await within(menu).findByRole("button", {
        name: "Nova conversa sobre esta análise",
      }),
    );
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    expect(terminal.aberturas[0].pedido).toEqual({
      promptInicial: "Retome a análise A-0001 pelo ctower",
    });
  });

  it("reabre uma análise sem sessão com o pedido de retomar_analise", async () => {
    const { terminal } = montar({ tipo: "reabrir" });
    await screen.findByText("Nenhuma afirmação registrada ainda.");
    const menu = abrirMenu();
    expect(
      within(menu).queryByRole("button", { name: "Retomar a conversa" }),
    ).not.toBeInTheDocument();
    const nova = await within(menu).findByRole("button", {
      name: "Nova conversa sobre esta análise",
    });
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
    // O preferido aparece primeiro, marcado como preferido e como onde a conversa está.
    const menu = abrirMenu();
    expect(menu).toHaveTextContent("A conversa está no Claude Desktop.");
    const cartoes = within(
      within(menu).getByRole("group", { name: "Clientes da conversa" }),
    )
      .getAllByRole("region")
      .map((r) => [
        r.getAttribute("data-cliente"),
        r.getAttribute("data-agora"),
      ]);
    expect(cartoes).toEqual([
      ["claudeDesktop", "true"],
      ["terminalEmbutido", null],
      ["terminalDoAnalista", null],
    ]);
    expect(within(menu).getByText("preferido")).toBeInTheDocument();
    // Voltar à conversa aqui, sem sessão ligada ainda: conversa nova com o pedido da
    // análise, nunca a mais recente da pasta (pode ser de outra análise).
    fireEvent.click(
      within(menu).getByRole("button", { name: "Voltar à conversa aqui" }),
    );
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    expect(terminal.aberturas[0].pedido).toEqual({
      retomada: { modo: "nova" },
      promptInicial: "Inicie a análise A-0001 pelo ctower",
    });
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
    // Sem as opções, o menu tem só o terminal embutido.
    const cartoes = within(abrirMenu())
      .getAllByRole("region")
      .map((r) => r.getAttribute("data-cliente"));
    expect(cartoes).toEqual(["terminalEmbutido"]);
  });

  it("retoma no Desktop a sessão iniciada no terminal embutido e volta", async () => {
    const { terminal, conversa } = montar(
      { tipo: "reabrir" },
      vistaFicticia({
        sessao: { id: "sessao-1", estado: "encerrada", cliente: "claude-code" },
      }),
      opcoesFicticias({ sessao: "sessao-1" }),
    );
    await screen.findByText("Nenhuma afirmação registrada ainda.");
    fireEvent.click(
      await within(abrirMenu()).findByRole("button", {
        name: "Retomar no Claude Desktop",
      }),
    );
    await waitFor(() =>
      expect(conversa.retomarNoDesktop).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "A-0001",
      ),
    );
    expect(
      await screen.findByText(/A conversa continua no Claude Desktop/),
    ).toBeVisible();
    expect(terminal.api.abrir).not.toHaveBeenCalled();
    // E volta ao terminal embutido pela mesma sessão.
    fireEvent.click(
      screen.getByRole("button", { name: "Voltar à conversa aqui" }),
    );
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    expect(terminal.aberturas[0].pedido).toEqual({
      retomada: { modo: "sessao", id: "sessao-1" },
    });
  });

  it("mostra por que o Desktop não abriu a sessão e oferece a conversa nova", async () => {
    const { conversa } = montar(
      { tipo: "reabrir" },
      vistaFicticia({
        sessao: {
          id: "sessao-1",
          estado: "encerrada",
          cliente: "claude-desktop",
        },
      }),
      opcoesFicticias({ sessao: "sessao-1" }),
    );
    conversa.retomarNoDesktop.mockResolvedValue({
      aberto: false,
      codigo: 1,
      mensagem: "Session is open in another terminal",
    });
    await screen.findByText("Nenhuma afirmação registrada ainda.");
    fireEvent.click(
      await within(abrirMenu()).findByRole("button", {
        name: "Retomar no Claude Desktop",
      }),
    );
    const alerta = await screen.findByRole("alert");
    expect(alerta).toHaveTextContent("Session is open in another terminal");
    expect(alerta).toHaveTextContent("conversa nova no Desktop");
    fireEvent.click(
      within(abrirMenu()).getByRole("button", {
        name: "Nova conversa no Claude Desktop",
      }),
    );
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
      vistaFicticia({
        sessao: { id: "sessao-1", estado: "encerrada", cliente: "claude-code" },
      }),
      opcoesFicticias({ sessao: "sessao-1" }),
    );
    conversa.retomarNoDesktop.mockRejectedValue({
      motivo: "desktopIndisponivel",
      detalhe: "versão antiga",
      orientacao: [],
      mensagem: "o Claude Code 2.1.200 não abre sessões no Desktop",
    });
    await screen.findByText("Nenhuma afirmação registrada ainda.");
    fireEvent.click(
      await within(abrirMenu()).findByRole("button", {
        name: "Retomar no Claude Desktop",
      }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "o Claude Code 2.1.200 não abre sessões no Desktop",
    );
  });

  it("oferece o terminal do analista com o comando e o link", async () => {
    const { conversa } = montar(
      { tipo: "reabrir" },
      vistaFicticia({
        sessao: { id: "sessao-1", estado: "encerrada", cliente: "claude-code" },
      }),
      opcoesFicticias({
        sessao: "sessao-1",
        comandoRetomada:
          "cd /codigo/faturamento-exemplo && claude --resume sessao-1",
      }),
    );
    await screen.findByText("Nenhuma afirmação registrada ainda.");
    const cartao = await within(abrirMenu()).findByRole("region", {
      name: "No meu terminal",
    });
    expect(
      within(cartao).getByText(
        "cd /codigo/faturamento-exemplo && claude --resume sessao-1",
      ),
    ).toBeInTheDocument();
    fireEvent.click(
      within(cartao).getByRole("button", {
        name: "Copiar o comando de retomada",
      }),
    );
    await waitFor(() =>
      expect(conversa.copiar).toHaveBeenCalledWith(
        "cd /codigo/faturamento-exemplo && claude --resume sessao-1",
      ),
    );
    expect(await screen.findByText(/Comando copiado/)).toBeInTheDocument();
    fireEvent.click(
      within(cartao).getByRole("button", {
        name: "Copiar o comando de conversa nova",
      }),
    );
    await waitFor(() =>
      expect(conversa.copiar).toHaveBeenLastCalledWith(
        "cd /codigo/faturamento-exemplo && claude 'Inicie a análise A-0001 pelo ctower'",
      ),
    );
    fireEvent.click(
      within(cartao).getByRole("button", {
        name: "Abrir pelo link claude-cli://",
      }),
    );
    await waitFor(() =>
      expect(conversa.abrirPorLink).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "A-0001",
        "terminalDoAnalista",
      ),
    );
    expect(
      await screen.findByText(/A conversa continua no seu terminal/),
    ).toBeVisible();
  });

  it("passa do terminal embutido para o do analista encerrando o daqui", async () => {
    const { terminal } = montar(
      { tipo: "nova", promptInicial: "Inicie" },
      vistaFicticia({
        sessao: { id: "sessao-1", estado: "ativa", cliente: "claude-code" },
      }),
      opcoesFicticias({
        sessao: "sessao-1",
        comandoRetomada:
          "cd /codigo/faturamento-exemplo && claude --resume sessao-1",
      }),
    );
    await terminalAberto(terminal, 4);
    fireEvent.click(
      await within(abrirMenu()).findByRole("button", {
        name: "Continuar no meu terminal",
      }),
    );
    await waitFor(() => expect(terminal.api.encerrar).toHaveBeenCalledWith(4));
    expect(
      await screen.findByText(/A conversa continua no seu terminal/),
    ).toBeVisible();
    // O menu continua aberto, com o comando para copiar.
    expect(
      within(abrirMenu()).getByText(
        "cd /codigo/faturamento-exemplo && claude --resume sessao-1",
      ),
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
    const menu = abrirMenu();
    const meuTerminal = await within(menu).findByRole("button", {
      name: "Continuar no meu terminal",
    });
    expect(meuTerminal).toBeEnabled();
    expect(
      within(menu).queryByRole("button", {
        name: "Continuar no Claude Desktop",
      }),
    ).not.toBeInTheDocument();
    const cartoes = within(menu)
      .getAllByRole("region")
      .map((r) => r.getAttribute("data-cliente"));
    expect(cartoes[cartoes.length - 1]).toBe("claudeDesktop");
    fireEvent.click(meuTerminal);
    expect(
      await screen.findByText("o Claude Desktop não existe para Linux"),
    ).toBeVisible();
  });

  describe("conversa com o agente (#185)", () => {
    it("abrindo o Claude Code, diz que está abrindo", async () => {
      montar({ tipo: "nova", promptInicial: "Inicie" });
      await waitFor(() =>
        expect(
          screen.getByRole("status", { name: "Estado do agente" }),
        ).toHaveTextContent("Abrindo o Claude Code"),
      );
    });

    it.each([
      ["ativa", "Trabalhando na análise"],
      ["aguardando-analista", "Terminou e espera você"],
      ["concluida", "Respondeu e espera a sua próxima mensagem"],
      ["encerrada", "Pronto para conversar"],
    ] as const)('com a sessão %s, mostra "%s"', async (sessao, texto) => {
      const { terminal } = montar(
        { tipo: "nova", promptInicial: "Inicie" },
        vistaFicticia({
          sessao: { id: "s-1", estado: sessao, cliente: "claude-code" },
        }),
      );
      await terminalAberto(terminal);
      await waitFor(() =>
        expect(
          screen.getByRole("status", { name: "Estado do agente" }),
        ).toHaveTextContent(texto),
      );
    });

    it("sem conversa aberta, o estado é 'Nenhuma conversa aberta'", async () => {
      montar({ tipo: "reabrir" });
      expect(
        screen.getByRole("status", { name: "Estado do agente" }),
      ).toHaveTextContent("Nenhuma conversa aberta");
      // O seletor mostra onde escolher, e as ações só valem com uma conversa.
      expect(
        screen.getByRole("button", { name: "Continuar a conversa" }),
      ).toHaveTextContent("Conversa");
      expect(
        screen.queryByRole("group", { name: "Ações rápidas" }),
      ).not.toBeInTheDocument();
    });

    it("o seletor mostra onde a conversa roda", async () => {
      const { terminal } = montar({ tipo: "nova", promptInicial: "Inicie" });
      await terminalAberto(terminal);
      expect(
        screen.getByRole("button", { name: "Continuar a conversa" }),
      ).toHaveTextContent("Aqui no app");
    });

    it("esconder o terminal não encerra a sessão, é lembrado e reabrir mostra o terminal", async () => {
      const { terminal, unmount } = montar({
        tipo: "nova",
        promptInicial: "Inicie",
      });
      await terminalAberto(terminal, 4);
      expect(screen.getByTestId("terminal-tela")).toBeVisible();
      fireEvent.click(
        screen.getByRole("button", { name: "Esconder o terminal" }),
      );
      // Continua montado, só escondido: a sessão não é interrompida.
      expect(screen.getByTestId("terminal-tela")).toBeInTheDocument();
      expect(screen.getByTestId("terminal-tela")).not.toBeVisible();
      expect(terminal.api.encerrar).not.toHaveBeenCalled();
      expect(terminal.api.abrir).toHaveBeenCalledTimes(1);
      expect(screen.getByText(/O terminal está escondido/)).toBeVisible();
      expect(localStorage.getItem("cba.terminalVisivel")).toBe("0");

      // Com o terminal escondido, a caixa continua falando com a mesma sessão.
      fireEvent.change(
        screen.getByRole("textbox", { name: "Escreva ao agente" }),
        {
          target: { value: "Oi" },
        },
      );
      fireEvent.click(screen.getByRole("button", { name: "Enviar" }));
      await waitFor(() =>
        expect(terminal.api.enviarPrompt).toHaveBeenCalledWith(4, "Oi"),
      );

      fireEvent.click(
        screen.getByRole("button", { name: "Mostrar o terminal" }),
      );
      expect(screen.getByTestId("terminal-tela")).toBeVisible();
      expect(terminal.api.abrir).toHaveBeenCalledTimes(1);
      expect(localStorage.getItem("cba.terminalVisivel")).toBe("1");
      unmount();
    });

    it("lembra o terminal escondido na próxima abertura", async () => {
      localStorage.setItem("cba.terminalVisivel", "0");
      const { terminal } = montar({ tipo: "nova", promptInicial: "Inicie" });
      await terminalAberto(terminal);
      expect(screen.getByTestId("terminal-tela")).not.toBeVisible();
      expect(
        screen.getByRole("button", { name: "Mostrar o terminal" }),
      ).toBeInTheDocument();
    });

    it("a caixa de mensagem envia o mesmo que digitar no terminal, com as quebras de linha", async () => {
      const { terminal } = montar({ tipo: "nova", promptInicial: "Inicie" });
      const caixa = await screen.findByRole("textbox", {
        name: "Escreva ao agente",
      });
      expect(caixa).toBeDisabled();
      await terminalAberto(terminal, 9);
      await waitFor(() => expect(caixa).toBeEnabled());
      expect(screen.getByRole("button", { name: "Enviar" })).toBeDisabled();
      fireEvent.change(caixa, {
        target: { value: "Qual é a multa?\nE os juros?" },
      });
      fireEvent.click(screen.getByRole("button", { name: "Enviar" }));
      await waitFor(() =>
        expect(terminal.api.enviarPrompt).toHaveBeenCalledWith(
          9,
          "Qual é a multa?\nE os juros?",
        ),
      );
      await waitFor(() => expect(caixa).toHaveValue(""));
      // Enter envia; Shift+Enter quebra a linha (não envia).
      fireEvent.change(caixa, { target: { value: "Outra" } });
      fireEvent.keyDown(caixa, { key: "Enter", shiftKey: true });
      expect(terminal.api.enviarPrompt).toHaveBeenCalledTimes(1);
      fireEvent.keyDown(caixa, { key: "Enter" });
      await waitFor(() =>
        expect(terminal.api.enviarPrompt).toHaveBeenLastCalledWith(9, "Outra"),
      );
    });

    it("falha ao enviar mantém o texto na caixa e mostra o motivo", async () => {
      const { terminal } = montar({ tipo: "nova", promptInicial: "Inicie" });
      await terminalAberto(terminal, 2);
      terminal.api.enviarPrompt.mockRejectedValueOnce("o processo saiu");
      const caixa = screen.getByRole("textbox", { name: "Escreva ao agente" });
      await waitFor(() => expect(caixa).toBeEnabled());
      fireEvent.change(caixa, { target: { value: "Oi" } });
      fireEvent.click(screen.getByRole("button", { name: "Enviar" }));
      expect(await screen.findByRole("alert")).toHaveTextContent(
        "o processo saiu",
      );
      expect(caixa).toHaveValue("Oi");
    });

    it("as ações rápidas enviam o mesmo pedido que digitar no terminal", async () => {
      const { api, terminal } = montar({
        tipo: "nova",
        promptInicial: "Inicie",
      });
      api.promptsProntos.mockResolvedValue([
        {
          id: "aprofundar",
          rotulo: "Aprofundar a análise",
          descricao: "",
          texto: "Aprofunde a A-0001.",
        },
      ]);
      const grupo = await screen.findByRole("group", { name: "Ações rápidas" });
      const botoes = [
        "Aprofundar as pendentes",
        "Outro exemplo",
        "Explicar sem termos técnicos",
      ].map((nome) => within(grupo).getByRole("button", { name: nome }));
      // Sem a sessão viva, não há para onde enviar.
      for (const b of botoes) expect(b).toBeDisabled();
      await terminalAberto(terminal, 6);
      for (const b of botoes) await waitFor(() => expect(b).toBeEnabled());

      fireEvent.click(botoes[0]);
      await waitFor(() =>
        expect(terminal.api.enviarPrompt).toHaveBeenCalledWith(
          6,
          "Aprofunde a A-0001. Comece pelas afirmações que ainda esperam a revisão do analista.",
        ),
      );
      fireEvent.click(botoes[1]);
      await waitFor(() =>
        expect(terminal.api.enviarPrompt).toHaveBeenCalledWith(
          6,
          expect.stringMatching(/outro exemplo/i),
        ),
      );
      fireEvent.click(botoes[2]);
      await waitFor(() =>
        expect(terminal.api.enviarPrompt).toHaveBeenCalledWith(
          6,
          expect.stringMatching(/sem termos técnicos/),
        ),
      );
      // Tudo numa linha só: o terminal envia a cada quebra de linha.
      for (const [, texto] of terminal.api.enviarPrompt.mock
        .calls as unknown as [number, string][])
        expect(texto).not.toMatch(/\n/);
    });

    it("sem a prompt pronto de aprofundar, a ação rápida usa o pedido padrão", async () => {
      const { terminal } = montar({ tipo: "nova", promptInicial: "Inicie" });
      await terminalAberto(terminal, 6);
      const botao = screen.getByRole("button", {
        name: "Aprofundar as pendentes",
      });
      await waitFor(() => expect(botao).toBeEnabled());
      fireEvent.click(botao);
      await waitFor(() =>
        expect(terminal.api.enviarPrompt).toHaveBeenCalledWith(
          6,
          expect.stringMatching(
            /^Aprofunde a análise pelo ctower\. Comece pelas afirmações pendentes|^Aprofunde a análise pelo ctower\. Comece pelas afirmações que ainda esperam/,
          ),
        ),
      );
    });
  });
});
