import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { DocumentacaoDoMapa, INTERVALO_DO_LOTE_MS } from "./DocumentacaoDoMapa";
import type { ApiDocumentacao } from "./documentacaoMapa";
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

function abrir(api: ApiDocumentacao, terminal = terminalFalso()) {
  const aoAbrirFuncionalidade = vi.fn();
  render(
    <DocumentacaoDoMapa
      projeto="faturamento-exemplo"
      api={api}
      apiTerminal={terminal}
      criarTela={() => telaFalsa() as never}
      aoAbrirFuncionalidade={aoAbrirFuncionalidade}
    />,
  );
  return { terminal, aoAbrirFuncionalidade };
}

const tabela = () => screen.findByRole("table", { name: "Documentação por funcionalidade" });

describe("DocumentacaoDoMapa", () => {
  afterEach(() => vi.useRealTimers());

  it("no RUP, mostra as colunas vivas com o estado e a ação de cada célula", async () => {
    abrir(apiDocumentacaoFalsa().api);
    const t = await tabela();
    const cabecalhos = within(t)
      .getAllByRole("columnheader")
      .map((c) => c.textContent);
    expect(cabecalhos.slice(2)).toEqual([
      "Especificação de caso de uso",
      "Catálogo de regras de negócio",
      "Especificação suplementar",
      "Manual do usuário",
    ]);
    const f1 = within(t).getByRole("row", { name: "F-FAT-001: Cancelar fatura" });
    const celulas = within(f1).getAllByRole("cell").slice(1);
    expect(celulas[0]).toHaveTextContent("v1.3");
    expect(celulas[0]).toHaveTextContent("em dia");
    expect(within(celulas[0]).queryByRole("button")).toBeNull();
    expect(celulas[1]).toHaveTextContent("rascunho v1.0");
    expect(celulas[1]).toHaveTextContent("proposta pendente");
    expect(within(celulas[1]).getByRole("button")).toHaveTextContent("Revisar proposta");
    expect(celulas[2]).toHaveTextContent("sem documento");
    expect(within(celulas[2]).getByRole("button")).toHaveTextContent("Gerar versão inicial");
    expect(celulas[3]).toHaveTextContent("desatualizado");
    expect(celulas[3]).toHaveTextContent("mudou: db/04_pkg_situacao.sql");
    expect(within(celulas[3]).getByRole("button")).toHaveTextContent("Propor atualização");
  });

  it("a cobertura por tipo bate com as células da tabela", async () => {
    abrir(apiDocumentacaoFalsa().api);
    const t = await tabela();
    const linhas = within(t).getAllByRole("row").slice(1);
    COLUNAS.forEach((c, i) => {
      const comDocumento = linhas.filter(
        (l) => !within(l).getAllByRole("cell")[i + 1].textContent?.includes("sem documento"),
      ).length;
      const cartao = screen.getByLabelText(`Cobertura de ${c.nome}`);
      expect(cartao).toHaveTextContent(`${comDocumento} de 3 funcionalidades`);
    });
    expect(screen.getByLabelText("Cobertura de Catálogo de regras de negócio")).toHaveTextContent(
      "1 proposta pendente",
    );
    expect(screen.getByLabelText("Cobertura de Manual do usuário")).toHaveTextContent(
      "1 desatualizado",
    );
  });

  it("gera em lote os casos de uso: uma sessão por funcionalidade, com progresso, e nada publicado", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { api, estado } = apiDocumentacaoFalsa(
      tabelaDe([
        linha("F-FAT-001", "Cancelar fatura"),
        linha("F-FAT-002", "Calcular multa"),
        linha("F-FAT-003", "Atualizar contato"),
      ]),
    );
    const ids = ["F-FAT-001", "F-FAT-002", "F-FAT-003"];
    api.gerar.mockResolvedValue({
      tipo: "caso-de-uso",
      geracao: "agente",
      itens: ids.map((f) => ({
        resultado: "sessao",
        funcionalidade: f,
        pedido: `Gere a versão inicial de ${f}: chame iniciar_versao_inicial`,
      })),
    });
    const { terminal } = abrir(api);
    await tabela();
    fireEvent.click(screen.getByRole("checkbox", { name: "Selecionar todas" }));
    expect(screen.getByRole("group", { name: "Geração em lote" })).toHaveTextContent(
      "3 funcionalidades selecionadas",
    );
    fireEvent.change(screen.getByRole("combobox", { name: "Tipo a gerar" }), {
      target: { value: "caso-de-uso" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Gerar a versão inicial" }));
    await waitFor(() =>
      expect(api.gerar).toHaveBeenCalledWith("faturamento-exemplo", "caso-de-uso", ids),
    );
    expect(await screen.findByLabelText("Progresso do lote")).toHaveTextContent(
      "Sessão 1 de 3: F-FAT-001",
    );
    await waitFor(() => expect(terminal.abrir).toHaveBeenCalledTimes(1));
    expect(terminal.abrir.mock.calls[0][1]).toEqual({
      promptInicial: expect.stringContaining("F-FAT-001"),
    });

    // O agente registra o rascunho de cada uma: o lote passa para a próxima.
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
    const nomes = ["Cancelar fatura", "Calcular multa", "Atualizar contato"];
    for (let i = 0; i < 3; i++) {
      estado.tabela = tabelaDe(
        ids.map((f, j) => (j <= i ? rascunho(f, nomes[j]) : linha(f, nomes[j]))),
      );
      await act(async () => {
        vi.advanceTimersByTime(INTERVALO_DO_LOTE_MS);
      });
      if (i < 2)
        await waitFor(() =>
          expect(screen.getByLabelText("Progresso do lote")).toHaveTextContent(
            `Sessão ${i + 2} de 3: ${ids[i + 1]}`,
          ),
        );
    }
    await waitFor(() =>
      expect(screen.getByLabelText("Progresso do lote")).toHaveTextContent(
        "Lote concluído: 3 rascunhos gerados",
      ),
    );
    expect(terminal.abrir).toHaveBeenCalledTimes(3);
    expect(screen.getByLabelText("Cobertura de Especificação de caso de uso")).toHaveTextContent(
      "3 de 3 funcionalidades",
    );
    expect(screen.getByLabelText("Cobertura de Especificação de caso de uso")).toHaveTextContent(
      "3 propostas pendentes",
    );
    expect(api.publicar).not.toHaveBeenCalled();
    expect(api.marcarRevisada).not.toHaveBeenCalled();
  });

  it("os tipos do app saem na hora, como rascunho", async () => {
    const { api } = apiDocumentacaoFalsa();
    api.gerar.mockResolvedValue({
      tipo: "especificacao-suplementar",
      geracao: "app",
      itens: [
        { resultado: "gerada", funcionalidade: "F-FAT-002", caminho: "a.md" },
        { resultado: "gerada", funcionalidade: "F-FAT-003", caminho: "b.md" },
        {
          resultado: "pulada",
          funcionalidade: "F-FAT-001",
          motivo: "já tem o documento (em dia)",
        },
      ],
    });
    abrir(api);
    await tabela();
    for (const f of ["F-FAT-001", "F-FAT-002", "F-FAT-003"])
      fireEvent.click(screen.getByRole("checkbox", { name: `Selecionar ${f}` }));
    fireEvent.change(screen.getByRole("combobox", { name: "Tipo a gerar" }), {
      target: { value: "especificacao-suplementar" },
    });
    expect(screen.getByText("Montado pelo app na hora, sem IA.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Gerar a versão inicial" }));
    expect(
      await screen.findByText(
        /2 rascunhos gerados \(v1\.0\), à espera da revisão final\. Ficaram de fora: F-FAT-001/,
      ),
    ).toBeInTheDocument();
    expect(screen.queryByLabelText("Sessões do lote")).toBeNull();
    expect(api.tabela.mock.calls.length).toBeGreaterThan(1);
  });

  it("a proposta só é publicada depois da revisão final", async () => {
    const { api } = apiDocumentacaoFalsa();
    abrir(api);
    await tabela();
    fireEvent.click(
      screen.getByRole("button", {
        name: "Revisar proposta: Catálogo de regras de negócio de F-FAT-001",
      }),
    );
    const revisao = await screen.findByRole("region", { name: "Revisão da proposta" });
    expect(api.proposta).toHaveBeenCalledWith(
      "faturamento-exemplo",
      "funcionalidades/cobranca/cancelar-fatura/catalogo-regras.md",
    );
    expect(within(revisao).getByText("Revisão final obrigatória")).toBeInTheDocument();
    expect(within(revisao).getByText(/Versão inicial, gerada por engenharia reversa/)).toBeInTheDocument();
    const publicar = within(revisao).getByRole("button", { name: "Publicar a v1.0" });
    expect(publicar).toBeDisabled();
    fireEvent.click(within(revisao).getByRole("button", { name: "Marcar como revisado" }));
    await waitFor(() => expect(publicar).toBeEnabled());
    expect(within(revisao).getByText(/Revisada por ana/)).toBeInTheDocument();
    fireEvent.click(publicar);
    await waitFor(() => expect(api.publicar).toHaveBeenCalledTimes(1));
    expect(
      await screen.findByText("Catálogo de regras de negócio v1.0 publicado."),
    ).toBeInTheDocument();
  });

  it("propõe a atualização: o app remonta o que monta e o agente vai pela demanda", async () => {
    const t = tabelaInicial();
    // O catálogo de F-FAT-001 desatualizado (montado pelo app).
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

  it("sem regras na funcionalidade, o caso de uso avisa e leva ao Explicar em vez de gerar", async () => {
    const { api } = apiDocumentacaoFalsa(
      tabelaDe([linha("F-FAT-002", "Calcular multa", undefined, 0)]),
    );
    const { aoAbrirFuncionalidade } = abrir(api);
    const t = await tabela();
    const f2 = within(t).getByRole("row", { name: "F-FAT-002: Calcular multa" });
    const [caso, catalogo] = within(f2).getAllByRole("cell").slice(1);
    expect(caso).toHaveTextContent("precisa de regras: explique a funcionalidade e aprove a análise");
    expect(within(caso).queryByRole("button", { name: /Gerar versão inicial/ })).toBeNull();
    fireEvent.click(
      within(caso).getByRole("button", {
        name: "Explicar F-FAT-002 para gerar Especificação de caso de uso",
      }),
    );
    expect(aoAbrirFuncionalidade).toHaveBeenCalledWith("F-FAT-002");
    // Os tipos montados pelo app não dependem das regras.
    expect(within(catalogo).getByRole("button")).toHaveTextContent("Gerar versão inicial");
    expect(api.gerar).not.toHaveBeenCalled();
  });

  it("ao abrir o lote, leva a vista até as sessões", async () => {
    const rolar = vi.fn();
    const original = Element.prototype.scrollIntoView;
    Element.prototype.scrollIntoView = rolar;
    try {
      const { api } = apiDocumentacaoFalsa(tabelaDe([linha("F-FAT-001", "Cancelar fatura")]));
      api.gerar.mockResolvedValue({
        tipo: "caso-de-uso",
        geracao: "agente",
        itens: [{ resultado: "sessao", funcionalidade: "F-FAT-001", pedido: "Gere F-FAT-001" }],
      });
      abrir(api);
      await tabela();
      fireEvent.click(
        screen.getByRole("button", {
          name: "Gerar versão inicial: Especificação de caso de uso de F-FAT-001",
        }),
      );
      const sessoes = await screen.findByRole("region", { name: "Sessões do lote" });
      await waitFor(() => expect(rolar).toHaveBeenCalledTimes(1));
      expect(rolar.mock.contexts[0]).toContainElement(sessoes);
    } finally {
      Element.prototype.scrollIntoView = original;
    }
  });

  it("ao abrir a revisão de uma proposta, leva a vista até ela", async () => {
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

  it("no cartão da funcionalidade, mostra só os documentos dela, sem o lote nem a cobertura", async () => {
    const { api } = apiDocumentacaoFalsa();
    const aoExplicar = vi.fn();
    render(
      <DocumentacaoDoMapa
        projeto="faturamento-exemplo"
        api={api}
        funcionalidade="F-FAT-001"
        aoExplicar={aoExplicar}
      />,
    );
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

  it("sem tipo vivo habilitado, explica onde habilitar", async () => {
    const { api } = apiDocumentacaoFalsa({ ...tabelaInicial(), colunas: [], cobertura: [] });
    abrir(api);
    expect(await screen.findByText(/Nenhum documento vivo habilitado/)).toBeInTheDocument();
  });
});
