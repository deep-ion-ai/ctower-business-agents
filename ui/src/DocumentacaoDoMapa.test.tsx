import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { DocumentacaoDoMapa, INTERVALO_DO_LOTE_MS } from "./DocumentacaoDoMapa";
import { EVENTO_DOCUMENTACAO, type ApiDocumentacao } from "./documentacaoMapa";
import {
  apiDocumentacaoFalsa,
  COLUNAS,
  linha,
  sem,
  tabelaDe,
  tabelaInicial,
} from "./test/documentacaoFalsa";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

function telaFalsa() {
  return {
    abrirEm: vi.fn(),
    escrever: vi.fn(),
    aoDigitar: vi.fn(),
    ajustar: vi.fn(() => ({ linhas: 24, colunas: 80 })),
    limpar: vi.fn(),
    focar: vi.fn(),
    descartar: vi.fn(),
  };
}

function terminalFalso() {
  return {
    abrir: vi.fn().mockResolvedValue(7),
    escrever: vi.fn().mockResolvedValue(undefined),
    redimensionar: vi.fn().mockResolvedValue(undefined),
    enviarPrompt: vi.fn().mockResolvedValue(undefined),
    encerrar: vi.fn().mockResolvedValue(undefined),
  };
}

function abrir(
  api: ApiDocumentacao,
  extra: Partial<Parameters<typeof DocumentacaoDoMapa>[0]> = {},
  terminal = terminalFalso(),
) {
  const aoAbrirFuncionalidade = vi.fn();
  render(
    <DocumentacaoDoMapa
      projeto="faturamento-exemplo"
      api={api}
      apiTerminal={terminal}
      criarTela={() => telaFalsa() as never}
      aoAbrirFuncionalidade={aoAbrirFuncionalidade}
      {...extra}
    />,
  );
  return { terminal, aoAbrirFuncionalidade };
}

const tabela = () => screen.findByRole("table", { name: "Documentação por funcionalidade" });
const celulas = (t: HTMLElement, rotulo: string) =>
  within(within(t).getByRole("row", { name: rotulo })).getAllByRole("cell").slice(1);

/** Caso de uso como rascunho, o resto sem documento. */
const rascunho = (f: string, n: string) =>
  linha(f, n, [
    {
      tipo: "caso-de-uso",
      caminho: `funcionalidades/${f}/caso-de-uso.md`,
      estado: { estado: "proposta_pendente", versao: "1.0", revisada: false },
      acao: "Revisar proposta",
    },
    ...COLUNAS.slice(1).map((c) => sem(c.tipo)),
  ]);

describe("DocumentacaoDoMapa", () => {
  afterEach(() => vi.useRealTimers());

  it("no RUP, mostra as colunas vivas e o estado de cada célula, com a ação no foco", async () => {
    abrir(apiDocumentacaoFalsa().api);
    const t = await tabela();
    const cabecalhos = within(t)
      .getAllByRole("columnheader")
      .map((c) => c.textContent);
    expect(cabecalhos.slice(2)).toEqual([
      "Especificação de caso de usoagente",
      "Catálogo de regras de negócioapp · na hora",
      "Especificação suplementarapp · na hora",
      "Manual do usuárioagente",
    ]);
    const [caso, catalogo, suplementar, manual] = celulas(t, "F-FAT-001: Cancelar fatura");
    expect(caso).toHaveTextContent("v1.3 em dia");
    expect(within(caso).queryByRole("button")).toBeNull();
    expect(
      within(catalogo).getByRole("button", {
        name: "Revisar proposta: Catálogo de regras de negócio de F-FAT-001",
      }),
    ).toHaveTextContent("rascunho v1.0 · a revisar");
    expect(suplementar).toHaveTextContent("pronta para gerar");
    expect(
      within(suplementar).getByRole("button", {
        name: "Gerar versão inicial: Especificação suplementar de F-FAT-001",
      }),
    ).toHaveTextContent("Gerar na hora");
    expect(manual).toHaveTextContent("v1.0 desatualizado");
    expect(
      within(manual).getByRole("button", {
        name: "Propor atualização: Manual do usuário de F-FAT-001",
      }),
    ).toBeInTheDocument();
  });

  it("a cobertura conta por estado e cada número filtra a tabela", async () => {
    abrir(apiDocumentacaoFalsa().api);
    const t = await tabela();
    const catalogo = screen.getByLabelText("Cobertura de Catálogo de regras de negócio");
    expect(catalogo).toHaveTextContent("1 de 3");
    expect(catalogo).toHaveTextContent("montado pelo app · na hora");
    const filtrar = within(catalogo).getByRole("button", { name: "1 a revisar" });
    within(catalogo).getByRole("button", { name: "2 prontas para gerar" });
    fireEvent.click(filtrar);
    expect(filtrar).toHaveAttribute("aria-pressed", "true");
    expect(within(t).getByRole("row", { name: "F-FAT-001: Cancelar fatura" })).toBeInTheDocument();
    expect(within(t).queryByRole("row", { name: "F-FAT-002: Calcular multa" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "limpar" }));
    expect(within(t).getByRole("row", { name: "F-FAT-002: Calcular multa" })).toBeInTheDocument();
  });

  it("busca por nome ou código e filtra pelas abas de estado, com as contagens", async () => {
    const { api } = apiDocumentacaoFalsa(
      tabelaDe([
        ...tabelaInicial().linhas,
        linha("F-FAT-004", "Emitir boleto", undefined, 0, false),
      ]),
    );
    abrir(api);
    const t = await tabela();
    const estado = screen.getByRole("group", { name: "Estado" });
    expect(within(estado).getByRole("button", { name: /Todas/ })).toHaveTextContent("Todas 4");
    expect(within(estado).getByRole("button", { name: /A revisar/ })).toHaveTextContent("1");
    expect(within(estado).getByRole("button", { name: /Aguardando/ })).toHaveTextContent("1");
    fireEvent.click(within(estado).getByRole("button", { name: /Aguardando/ }));
    expect(within(t).getAllByRole("row", { name: /^F-FAT/ })).toHaveLength(1);
    expect(within(t).getByRole("row", { name: "F-FAT-004: Emitir boleto" })).toBeInTheDocument();
    fireEvent.click(within(estado).getByRole("button", { name: /Todas/ }));

    fireEvent.change(screen.getByRole("searchbox", { name: "Buscar funcionalidade" }), {
      target: { value: "MULTA" },
    });
    expect(within(t).getAllByRole("row", { name: /^F-FAT/ })).toHaveLength(1);
    fireEvent.change(screen.getByRole("searchbox", { name: "Buscar funcionalidade" }), {
      target: { value: "f-fat-003" },
    });
    expect(within(t).getByRole("row", { name: "F-FAT-003: Atualizar contato" })).toBeInTheDocument();
  });

  it("agrupa por área, com a cobertura da área, e recolhe quando o mapa é grande", async () => {
    const muitas = Array.from({ length: 40 }, (_, i) => {
      const l = linha(`F-COB-${String(i + 1).padStart(3, "0")}`, `Funcionalidade ${i + 1}`);
      return i < 20 ? l : { ...l, area: "contrato", areaNome: "Contrato" };
    });
    abrir(apiDocumentacaoFalsa(tabelaDe(muitas)).api);
    const t = await tabela();
    const cobranca = within(t).getByRole("row", { name: "Área Cobrança" });
    expect(cobranca).toHaveTextContent("20");
    expect(within(cobranca).getAllByRole("cell")[1]).toHaveTextContent("0/20");
    // Mais de 30 linhas: as áreas abrem recolhidas.
    expect(within(t).queryByRole("row", { name: /^F-COB-001/ })).toBeNull();
    fireEvent.click(within(cobranca).getByRole("button", { name: "▸ Cobrança" }));
    expect(within(t).getByRole("row", { name: "F-COB-001: Funcionalidade 1" })).toBeInTheDocument();
    expect(within(t).queryByRole("row", { name: /^F-COB-021/ })).toBeNull();
    // Com busca, abre o que casa.
    fireEvent.change(screen.getByRole("searchbox", { name: "Buscar funcionalidade" }), {
      target: { value: "Funcionalidade 33" },
    });
    expect(within(t).getByRole("row", { name: "F-COB-033: Funcionalidade 33" })).toBeInTheDocument();
  });

  it("sem regras, a célula diz o que falta e abre os pré-requisitos, sem gerar", async () => {
    const { api } = apiDocumentacaoFalsa(
      tabelaDe([
        linha("F-FAT-002", "Calcular multa", undefined, 0, false),
        linha("F-FAT-003", "Atualizar contato", undefined, 0, true),
      ]),
    );
    const { aoAbrirFuncionalidade } = abrir(api);
    const t = await tabela();
    const [caso, catalogo] = celulas(t, "F-FAT-002: Calcular multa");
    expect(caso).toHaveTextContent("falta explicação");
    expect(within(caso).queryByRole("button", { name: /Gerar/ })).toBeNull();
    expect(celulas(t, "F-FAT-003: Atualizar contato")[0]).toHaveTextContent("sem regras na base");
    // Os tipos montados pelo app não dependem das regras.
    expect(within(catalogo).getByRole("button", { name: /Gerar versão inicial/ })).toBeEnabled();

    fireEvent.click(
      within(caso).getByRole("button", {
        name: "falta explicação: pré-requisitos de Especificação de caso de uso de F-FAT-002",
      }),
    );
    const painel = screen.getByRole("dialog", {
      name: "Pré-requisitos: Especificação de caso de uso de F-FAT-002",
    });
    expect(painel).toHaveTextContent("Ainda não dá para gerar a versão inicial.");
    expect(painel).toHaveTextContent("Mapeamento aceito");
    expect(painel).toHaveTextContent("Explicação aprovada");
    expect(painel).toHaveTextContent("Regras de negócio na base · 0");
    fireEvent.click(within(painel).getByRole("button", { name: "Explicar a funcionalidade" }));
    expect(aoAbrirFuncionalidade).toHaveBeenCalledWith("F-FAT-002");
    fireEvent.keyDown(painel, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.gerar).not.toHaveBeenCalled();
  });

  it("a seleção abre a barra do lote, que conta as elegíveis por tipo e diz por que as outras ficam de fora", async () => {
    const { api } = apiDocumentacaoFalsa(
      tabelaDe([
        ...tabelaInicial().linhas,
        linha("F-FAT-004", "Emitir boleto", undefined, 0, false),
      ]),
    );
    abrir(api);
    await tabela();
    expect(screen.queryByRole("group", { name: "Geração em lote" })).toBeNull();
    fireEvent.click(screen.getByRole("checkbox", { name: "Selecionar a área Cobrança" }));
    const barra = screen.getByRole("group", { name: "Geração em lote" });
    expect(barra).toHaveTextContent("4 funcionalidades selecionadas · Cobrança");
    const caso = within(barra).getByRole("button", { name: "Gerar caso de uso · 2 de 4" });
    expect(caso).toHaveTextContent("2 sessões em sequência · 1 já tem documento · 1 falta explicação");
    expect(
      within(barra).getByRole("button", { name: "Gerar catálogo de regras · 3 de 4" }),
    ).toHaveTextContent("montado na hora · 1 já tem documento");
    fireEvent.click(within(barra).getByRole("button", { name: "Limpar" }));
    expect(screen.queryByRole("group", { name: "Geração em lote" })).toBeNull();
  });

  it("o lote do agente roda numa gaveta: progresso por funcionalidade, só as células do lote ocupadas e parar depois desta", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const nomes = ["Cancelar fatura", "Calcular multa", "Atualizar contato"];
    const ids = ["F-FAT-001", "F-FAT-002", "F-FAT-003"];
    const { api, estado } = apiDocumentacaoFalsa(tabelaDe(ids.map((f, i) => linha(f, nomes[i]))));
    api.gerar.mockResolvedValue({
      tipo: "caso-de-uso",
      geracao: "agente",
      itens: ids.map((f) => ({
        resultado: "sessao" as const,
        funcionalidade: f,
        pedido: `Gere a versão inicial de ${f}: chame iniciar_versao_inicial`,
      })),
    });
    const { terminal } = abrir(api);
    const t = await tabela();
    fireEvent.click(screen.getByRole("checkbox", { name: "Selecionar a área Cobrança" }));
    fireEvent.click(screen.getByRole("button", { name: "Gerar caso de uso · 3 de 3" }));
    await waitFor(() =>
      expect(api.gerar).toHaveBeenCalledWith("faturamento-exemplo", "caso-de-uso", ids),
    );
    const gaveta = await screen.findByRole("complementary", { name: "Lote em andamento" });
    expect(within(gaveta).getByLabelText("Progresso do lote")).toHaveTextContent(
      "Sessão 1 de 3: F-FAT-001",
    );
    await waitFor(() => expect(terminal.abrir).toHaveBeenCalledTimes(1));
    expect(terminal.abrir.mock.calls[0][1]).toEqual({
      promptInicial: expect.stringContaining("F-FAT-001"),
    });
    expect(celulas(t, "F-FAT-001: Cancelar fatura")[0]).toHaveTextContent("gerando…");
    expect(celulas(t, "F-FAT-003: Atualizar contato")[0]).toHaveTextContent("na fila · 3º");
    // O resto da tabela continua: o catálogo (do app) ainda gera.
    expect(
      within(celulas(t, "F-FAT-001: Cancelar fatura")[1]).getByRole("button", {
        name: /Gerar versão inicial/,
      }),
    ).toBeEnabled();
    // Outro tipo do agente espera o lote terminar.
    expect(
      within(celulas(t, "F-FAT-001: Cancelar fatura")[3]).getByRole("button", {
        name: /Gerar versão inicial/,
      }),
    ).toBeDisabled();

    // O rascunho de F-FAT-001 aparece: o lote passa para a próxima.
    estado.tabela = tabelaDe(
      ids.map((f, j) => (j === 0 ? rascunho(f, nomes[j]) : linha(f, nomes[j]))),
    );
    await act(async () => {
      vi.advanceTimersByTime(INTERVALO_DO_LOTE_MS);
    });
    await waitFor(() =>
      expect(within(gaveta).getByLabelText("Progresso do lote")).toHaveTextContent(
        "Sessão 2 de 3: F-FAT-002",
      ),
    );
    expect(celulas(t, "F-FAT-001: Cancelar fatura")[0]).toHaveTextContent(
      "rascunho v1.0 · a revisar",
    );

    // Parar depois desta: termina F-FAT-002 e não abre F-FAT-003.
    fireEvent.click(within(gaveta).getByRole("button", { name: "Parar depois desta" }));
    estado.tabela = tabelaDe(
      ids.map((f, j) => (j <= 1 ? rascunho(f, nomes[j]) : linha(f, nomes[j]))),
    );
    await act(async () => {
      vi.advanceTimersByTime(INTERVALO_DO_LOTE_MS);
    });
    await waitFor(() =>
      expect(within(gaveta).getByLabelText("Progresso do lote")).toHaveTextContent(
        "Lote concluído: 2 rascunhos gerados · 1 não gerada",
      ),
    );
    expect(terminal.abrir).toHaveBeenCalledTimes(2);
    expect(
      within(gaveta).getByRole("list", { name: "Funcionalidades do lote" }),
    ).toHaveTextContent("F-FAT-003 Atualizar contato não gerado");
    expect(screen.getByRole("button", { name: "Revisão final · 2 rascunhos" })).toBeEnabled();
    expect(api.publicar).not.toHaveBeenCalled();

    // Recolher a gaveta deixa o selo do lote no cabeçalho.
    fireEvent.click(within(gaveta).getByRole("button", { name: "Recolher" }));
    expect(screen.queryByRole("complementary", { name: "Lote em andamento" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Lote 3/3" }));
    expect(screen.getByRole("complementary", { name: "Lote em andamento" })).toBeInTheDocument();
  });

  it("o lote diz quais ficaram de fora e por quê", async () => {
    const { api } = apiDocumentacaoFalsa();
    api.gerar.mockResolvedValue({
      tipo: "caso-de-uso",
      geracao: "agente",
      itens: [
        { resultado: "sessao", funcionalidade: "F-FAT-002", pedido: "Gere F-FAT-002" },
        {
          resultado: "pulada",
          funcionalidade: "F-FAT-003",
          motivo: "sem regras de negócio na base",
        },
      ],
    });
    abrir(api);
    await tabela();
    fireEvent.click(
      screen.getByRole("button", {
        name: "Gerar versão inicial: Especificação de caso de uso de F-FAT-002",
      }),
    );
    const gaveta = await screen.findByRole("complementary", { name: "Lote em andamento" });
    expect(gaveta).toHaveTextContent("Fora do lote: F-FAT-003 (sem regras de negócio na base)");
  });

  it("os tipos do app saem na hora, como rascunho", async () => {
    const { api } = apiDocumentacaoFalsa();
    api.gerar.mockResolvedValue({
      tipo: "especificacao-suplementar",
      geracao: "app",
      itens: [
        { resultado: "gerada", funcionalidade: "F-FAT-002", caminho: "a.md" },
        { resultado: "gerada", funcionalidade: "F-FAT-003", caminho: "b.md" },
      ],
    });
    const mudou = vi.fn();
    window.addEventListener(EVENTO_DOCUMENTACAO, mudou);
    abrir(api);
    await tabela();
    fireEvent.click(screen.getByRole("checkbox", { name: "Selecionar a área Cobrança" }));
    fireEvent.click(screen.getByRole("button", { name: "Gerar suplementar · 3 de 3" }));
    await waitFor(() =>
      expect(api.gerar).toHaveBeenCalledWith("faturamento-exemplo", "especificacao-suplementar", [
        "F-FAT-001",
        "F-FAT-002",
        "F-FAT-003",
      ]),
    );
    expect(
      await screen.findByText(/2 rascunhos gerados \(v1\.0\), à espera da revisão final\./),
    ).toBeInTheDocument();
    expect(screen.queryByRole("complementary", { name: "Lote em andamento" })).toBeNull();
    expect(mudou).toHaveBeenCalled();
    window.removeEventListener(EVENTO_DOCUMENTACAO, mudou);
  });

  it("com a navegação, a revisão abre numa aba própria; o botão da revisão final abre o primeiro da fila", async () => {
    const aoRevisar = vi.fn();
    abrir(apiDocumentacaoFalsa().api, { aoRevisar });
    await tabela();
    fireEvent.click(
      screen.getByRole("button", {
        name: "Revisar proposta: Catálogo de regras de negócio de F-FAT-001",
      }),
    );
    const pedido = {
      caminho: "funcionalidades/cobranca/cancelar-fatura/catalogo-regras.md",
      funcionalidade: "F-FAT-001",
      titulo: "Revisão · F-FAT-001 · Catálogo de regras",
    };
    expect(aoRevisar).toHaveBeenCalledWith(pedido);
    expect(screen.queryByRole("region", { name: "Revisão da proposta" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Revisão final · 1 rascunho" }));
    expect(aoRevisar).toHaveBeenCalledTimes(2);
    expect(aoRevisar).toHaveBeenLastCalledWith(pedido);
  });

  it("sem a navegação, a proposta é revisada aqui e só é publicada depois de marcada", async () => {
    const { api } = apiDocumentacaoFalsa();
    abrir(api);
    await tabela();
    fireEvent.click(
      screen.getByRole("button", {
        name: "Revisar proposta: Catálogo de regras de negócio de F-FAT-001",
      }),
    );
    const revisao = await screen.findByRole("region", { name: "Revisão da proposta" });
    expect(within(revisao).getByText("Revisão final obrigatória")).toBeInTheDocument();
    const publicar = within(revisao).getByRole("button", { name: "Publicar a v1.0" });
    expect(publicar).toBeDisabled();
    fireEvent.click(within(revisao).getByRole("button", { name: "Marcar como revisado" }));
    await waitFor(() => expect(publicar).toBeEnabled());
    fireEvent.click(publicar);
    await waitFor(() => expect(api.publicar).toHaveBeenCalledTimes(1));
    expect(
      await screen.findByText("Catálogo de regras de negócio v1.0 publicado."),
    ).toBeInTheDocument();
  });

  it("ao abrir a revisão embutida, leva a vista até ela", async () => {
    const rolar = vi.fn();
    const original = Element.prototype.scrollIntoView;
    Element.prototype.scrollIntoView = rolar;
    try {
      abrir(apiDocumentacaoFalsa().api);
      await tabela();
      fireEvent.click(
        screen.getByRole("button", {
          name: "Revisar proposta: Catálogo de regras de negócio de F-FAT-001",
        }),
      );
      const revisao = await screen.findByRole("region", { name: "Revisão da proposta" });
      await waitFor(() => expect(rolar).toHaveBeenCalledTimes(1));
      expect(rolar.mock.contexts[0]).toContainElement(revisao);
    } finally {
      Element.prototype.scrollIntoView = original;
    }
  });

  it("propõe a atualização: o app remonta o que monta e o agente vai pela demanda", async () => {
    const t = tabelaInicial();
    t.linhas[0].documentos[1] = {
      tipo: "catalogo-regras",
      caminho: "funcionalidades/cobranca/cancelar-fatura/catalogo-regras.md",
      versao: "1.0",
      estado: { estado: "desatualizado", versao: "1.0", arquivos: ["x.sql"] },
      acao: "Propor atualização",
    };
    const { api } = apiDocumentacaoFalsa(t);
    const { aoAbrirFuncionalidade } = abrir(api);
    await tabela();
    fireEvent.click(
      screen.getByRole("button", {
        name: "Propor atualização: Catálogo de regras de negócio de F-FAT-001",
      }),
    );
    await waitFor(() =>
      expect(api.proporAtualizacao).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "catalogo-regras",
        "F-FAT-001",
      ),
    );
    expect(await screen.findByRole("region", { name: "Revisão da proposta" })).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole("button", { name: "Propor atualização: Manual do usuário de F-FAT-001" }),
    );
    expect(aoAbrirFuncionalidade).toHaveBeenCalledWith("F-FAT-001");
    expect(screen.getByText(/a atualização vem da demanda/)).toBeInTheDocument();
  });

  it("tipos sem geração ainda ficam fora das colunas, com o caminho para Configurações", async () => {
    const aoConfigurar = vi.fn();
    abrir(apiDocumentacaoFalsa().api, { aoConfigurar });
    const t = await tabela();
    expect(within(t).queryByRole("columnheader", { name: /Glossário/ })).toBeNull();
    expect(screen.getByText("Glossário · geração em breve")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Configurações" }));
    expect(aoConfigurar).toHaveBeenCalled();
  });

  it("uma mudança noutra aba relê a tabela", async () => {
    const { api } = apiDocumentacaoFalsa();
    abrir(api);
    await tabela();
    const antes = api.tabela.mock.calls.length;
    act(() => {
      window.dispatchEvent(new Event(EVENTO_DOCUMENTACAO));
    });
    await waitFor(() => expect(api.tabela.mock.calls.length).toBe(antes + 1));
  });

  it("no cartão da funcionalidade, mostra só os documentos dela, sem o lote nem a cobertura", async () => {
    const { api } = apiDocumentacaoFalsa();
    render(<DocumentacaoDoMapa projeto="faturamento-exemplo" api={api} funcionalidade="F-FAT-001" />);
    const lista = await screen.findByRole("list", { name: "Documentos vivos de F-FAT-001" });
    const itens = within(lista).getAllByRole("listitem");
    expect(itens.map((i) => i.getAttribute("aria-label"))).toEqual(COLUNAS.map((c) => c.nome));
    expect(itens[0]).toHaveTextContent("v1.3");
    expect(itens[0]).toHaveTextContent("em dia");
    expect(within(itens[1]).getByRole("button")).toHaveTextContent("Revisar proposta");
    expect(screen.queryByRole("table")).toBeNull();
    expect(screen.queryByRole("group", { name: "Geração em lote" })).toBeNull();
    expect(screen.queryByRole("region", { name: "Cobertura por tipo" })).toBeNull();
    expect(screen.queryByText("Calcular multa")).toBeNull();

    fireEvent.click(within(itens[1]).getByRole("button", { name: /Revisar proposta/ }));
    expect(await screen.findByRole("region", { name: "Revisão da proposta" })).toBeInTheDocument();
  });

  it("no cartão, a funcionalidade sem regras leva ao Explicar", async () => {
    const { api } = apiDocumentacaoFalsa(
      tabelaDe([linha("F-FAT-002", "Calcular multa", undefined, 0, false)]),
    );
    const aoExplicar = vi.fn();
    render(
      <DocumentacaoDoMapa
        projeto="faturamento-exemplo"
        api={api}
        funcionalidade="F-FAT-002"
        aoExplicar={aoExplicar}
      />,
    );
    const lista = await screen.findByRole("list", { name: "Documentos vivos de F-FAT-002" });
    const caso = within(lista).getAllByRole("listitem")[0];
    expect(caso).toHaveTextContent("falta explicação: explique a funcionalidade e aprove a análise");
    fireEvent.click(
      within(caso).getByRole("button", {
        name: "Explicar F-FAT-002 para gerar Especificação de caso de uso",
      }),
    );
    expect(aoExplicar).toHaveBeenCalledWith("F-FAT-002");
  });

  it("sem tipo vivo habilitado, explica onde habilitar", async () => {
    const { api } = apiDocumentacaoFalsa({ ...tabelaInicial(), colunas: [], cobertura: [] });
    abrir(api);
    expect(await screen.findByText(/Nenhum documento vivo habilitado/)).toBeInTheDocument();
  });
});
