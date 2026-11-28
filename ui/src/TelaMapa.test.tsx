import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ApiAnalise, AvisoAnalise } from "./analise";
import type { ApiMapa } from "./funcionalidades";
import { apiDocumentacaoFalsa, linha, tabelaDe } from "./test/documentacaoFalsa";
import { apiFalsa, MAPA } from "./test/mapaFalso";
import { TelaMapa } from "./TelaMapa";
import type { ApiTerminal } from "./terminal";

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

function analiseFalsa() {
  let ouvinte: ((a: AvisoAnalise) => void) | null = null;
  const api = {
    vigiar: vi.fn((_projeto: string, aoAviso: (a: AvisoAnalise) => void) => {
      ouvinte = aoAviso;
      return Promise.resolve(() => undefined);
    }),
  } as unknown as ApiAnalise;
  return { api, avisar: (a: AvisoAnalise) => ouvinte?.(a) };
}

const PROJETOS = [{ id: "faturamento-exemplo", nome: "Faturamento Exemplo", pastas: [] }];

function abrir(
  api: ApiMapa,
  analise = analiseFalsa(),
  apiTerminal?: ApiTerminal,
  aoMudarConversa?: (estado: "nenhuma" | "aberta" | "trabalhando") => void,
) {
  const navegar = vi.fn();
  render(
    <TelaMapa
      projetos={PROJETOS as never}
      api={api}
      apiAnalise={analise.api}
      apiTerminal={apiTerminal}
      criarTela={() => telaFalsa() as never}
      navegar={navegar}
      aoMudarConversa={aoMudarConversa}
    />,
  );
  return { navegar, analise };
}

describe("TelaMapa", () => {
  it("a aba Documentação mostra a tabela dos documentos vivos e a cobertura por tipo (#462)", async () => {
    const { api: apiDocumentacao } = apiDocumentacaoFalsa(
      tabelaDe([linha("F-CAN-001", "Cancelar fatura"), linha("F-COB-001", "Calcular encargos")]),
    );
    render(
      <TelaMapa
        projetos={PROJETOS as never}
        api={apiFalsa()}
        apiAnalise={analiseFalsa().api}
        apiDocumentacao={apiDocumentacao}
        criarTela={() => telaFalsa() as never}
        navegar={vi.fn()}
      />,
    );
    const aba = await screen.findByRole("tab", { name: "Documentação" });
    expect(screen.getByRole("tab", { name: "Funcionalidades" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.queryByRole("table", { name: "Documentação por funcionalidade" })).toBeNull();
    fireEvent.click(aba);
    expect(aba).toHaveAttribute("aria-selected", "true");
    expect(
      await screen.findByRole("table", { name: "Documentação por funcionalidade" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Cobertura de Especificação de caso de uso")).toHaveTextContent(
      "0 de 2 com documento",
    );
    expect(apiDocumentacao.tabela).toHaveBeenCalledWith("faturamento-exemplo");
    // O nome da funcionalidade abre o painel dela.
    fireEvent.click(screen.getByRole("button", { name: "Cancelar fatura" }));
    expect(
      await screen.findByRole("button", { name: "← Todas as funcionalidades" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("tablist", { name: "Visão" })).toBeNull();
  });

  it("o vínculo das demandas vive na área Demandas: o Mapa não tem mais o botão (#323)", async () => {
    const api = apiFalsa();
    abrir(api);
    expect(await screen.findByRole("button", { name: /Exportar Markdown|Mapear/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Vincular demandas" })).toBeNull();
  });

  it("religa os documentos aprovados antes do vínculo e mostra o resumo (#287)", async () => {
    const api = apiFalsa();
    api.religar = vi.fn().mockResolvedValue({
      analises: ["analises/A-0001.md"],
      regras: ["regras/RN-CAN-001.md"],
      termos: [],
      negociais: [],
      regrasSemVinculo: 0,
      avisos: [],
    });
    abrir(api);
    fireEvent.click(await screen.findByRole("button", { name: "Mais" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Religar documentos" }));
    expect(
      await screen.findByText("Religados às funcionalidades: 1 análise, 1 regra."),
    ).toBeInTheDocument();
    expect(api.religar).toHaveBeenCalledWith("faturamento-exemplo");
  });

  it("mostra o mapa por área, o resumo e os pontos sem tratamento", async () => {
    abrir(apiFalsa());
    expect(await screen.findByRole("listitem", { name: "F-COB-001: Calcular encargos" })).toBeInTheDocument();
    expect(screen.getByLabelText("Resumo do mapa")).toHaveTextContent(
      "2 funcionalidades · 2 áreas · 1 a revisar · 1 documentadas · 15 pontos de entrada, 1 ainda sem funcionalidade",
    );
    expect(screen.getByRole("heading", { name: /Cancelamento/ })).toBeInTheDocument();
    const lacunas = screen.getByRole("list", { name: "Lacunas da cobertura" });
    expect(lacunas).toHaveTextContent("db");
    expect(screen.queryByText("PKG_SITUACAO.PRC_SUSPENDER_COBRANCA")).toBeNull();
    expect(screen.getByRole("list", { name: "Justificados" })).toHaveTextContent("utilitário");
  });

  it("aceita, filtra e desfaz uma justificativa", async () => {
    const api = apiFalsa();
    abrir(api);
    const cartao = await screen.findByRole("listitem", { name: "F-COB-001: Calcular encargos" });
    fireEvent.click(within(cartao).getByRole("button", { name: "Aceitar F-COB-001" }));
    await waitFor(() =>
      expect(api.revisar).toHaveBeenCalledWith("faturamento-exemplo", {
        tipo: "decidir",
        funcionalidade: "F-COB-001",
        decisao: "aceitar",
      }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText("Resumo do mapa")).toHaveTextContent("0 a revisar"),
    );

    fireEvent.click(screen.getByRole("button", { name: /^A revisar/ }));
    expect(screen.queryByRole("listitem", { name: "F-CAN-001: Cancelar fatura" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /^Todas \(/ }));
    fireEvent.change(screen.getByRole("searchbox", { name: "Buscar funcionalidade" }), {
      target: { value: "cancelar" },
    });
    expect(screen.queryByRole("listitem", { name: "F-COB-001: Calcular encargos" })).toBeNull();
    expect(screen.getByRole("listitem", { name: "F-CAN-001: Cancelar fatura" })).toBeInTheDocument();

    fireEvent.click(
      within(screen.getByRole("list", { name: "Justificados" })).getByRole("button", {
        name: "Desfazer",
      }),
    );
    await waitFor(() =>
      expect(api.revisar).toHaveBeenLastCalledWith("faturamento-exemplo", {
        tipo: "desfazer-justificativa",
        ponto: "db/03_pkg_cobranca.sql#PKG_COBRANCA.FN_PARAMETRO",
      }),
    );
  });

  it("divide uma funcionalidade escolhendo os pontos", async () => {
    const api = apiFalsa();
    abrir(api);
    const cartao = await screen.findByRole("listitem", { name: "F-COB-001: Calcular encargos" });
    fireEvent.click(within(cartao).getByRole("button", { name: "Mais ações para F-COB-001" }));
    fireEvent.click(within(cartao).getByRole("menuitem", { name: "Dividir" }));
    const form = within(cartao).getByRole("form", { name: "Dividir F-COB-001" });
    fireEvent.click(within(form).getByLabelText("EncargosService.calcular"));
    fireEvent.change(within(form).getByLabelText("Nome da nova"), {
      target: { value: "Calcular pelo serviço" },
    });
    fireEvent.change(within(form).getByLabelText("Descrição da nova"), {
      target: { value: "Pelo serviço." },
    });
    fireEvent.click(within(form).getByRole("button", { name: "Dividir" }));
    await waitFor(() =>
      expect(api.revisar).toHaveBeenCalledWith("faturamento-exemplo", {
        tipo: "dividir",
        funcionalidade: "F-COB-001",
        pontos: [1],
        nome: "Calcular pelo serviço",
        descricao: "Pelo serviço.",
      }),
    );
  });

  it("explica uma funcionalidade numa análise e abre o documento negocial", async () => {
    const api = apiFalsa();
    const { navegar } = abrir(api);
    const cartao = await screen.findByRole("listitem", { name: "F-CAN-001: Cancelar fatura" });
    fireEvent.click(within(cartao).getByRole("button", { name: "Mais ações para F-CAN-001" }));
    fireEvent.click(within(cartao).getByRole("menuitem", { name: "Documento negocial" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "base",
      documento: "faturamento-exemplo/funcionalidades/cancelamento/cancelar-fatura/negocial.md",
    });
    // Já explicada: sem "Explicar" e com o link para a análise.
    expect(within(cartao).queryByRole("button", { name: "Explicar F-CAN-001" })).toBeNull();
    fireEvent.click(within(cartao).getByRole("button", { name: "Mais ações para F-CAN-001" }));
    expect(within(cartao).queryByRole("menuitem", { name: "Explicar" })).toBeNull();
    fireEvent.click(within(cartao).getByRole("menuitem", { name: "Explicação A-0002 (aprovada)" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: "faturamento-exemplo",
      aberta: { id: "A-0002", abertura: { tipo: "reabrir" } },
    });
    const cobranca = screen.getByRole("listitem", { name: "F-COB-001: Calcular encargos" });
    fireEvent.click(within(cobranca).getByRole("button", { name: "Mais ações para F-COB-001" }));
    fireEvent.click(within(cobranca).getByRole("menuitem", { name: "Explicar" }));
    await waitFor(() =>
      expect(navegar).toHaveBeenCalledWith({
        tipo: "analise",
        projeto: "faturamento-exemplo",
        aberta: { id: "A-0003", abertura: { tipo: "nova", promptInicial: "Inicie a análise A-0003" } },
      }),
    );
  });

  it("relê o mapa quando o agente registra algo", async () => {
    const api = apiFalsa();
    const { analise } = abrir(api);
    await screen.findByRole("listitem", { name: "F-COB-001: Calcular encargos" });
    expect(api.mapa).toHaveBeenCalledTimes(1);
    analise.avisar({ tipo: "mudanca", analise: "A-0001", mudanca: "afirmacao", versao: 2 });
    analise.avisar({ tipo: "mudanca", analise: "mapa", mudanca: "funcionalidade", versao: 4 });
    await waitFor(() => expect(api.mapa).toHaveBeenCalledTimes(2));
    analise.avisar({ tipo: "mudanca", analise: "A-0003", mudanca: "criada", versao: 1 });
    await waitFor(() => expect(api.mapa).toHaveBeenCalledTimes(3));
  });

  it("começa a sessão de mapeamento com o recorte e a orientação", async () => {
    const api = apiFalsa();
    const abrirTerminal = vi.fn().mockResolvedValue(7);
    const apiTerminal = {
      abrir: abrirTerminal,
      escrever: vi.fn().mockResolvedValue(undefined),
      redimensionar: vi.fn().mockResolvedValue(undefined),
      encerrar: vi.fn().mockResolvedValue(undefined),
      enviarPrompt: vi.fn().mockResolvedValue(undefined),
    } as unknown as ApiTerminal;
    abrir(api, analiseFalsa(), apiTerminal);
    await screen.findByRole("listitem", { name: "F-COB-001: Calcular encargos" });
    fireEvent.click(screen.getByRole("button", { name: "Mapear com o Claude" }));
    const form = screen.getByRole("form", { name: "Começar o mapeamento" });
    fireEvent.change(within(form).getByLabelText("Pasta (opcional)"), { target: { value: "db" } });
    fireEvent.change(within(form).getByLabelText("Orientação ao agente (opcional)"), {
      target: { value: "Comece pelo banco." },
    });
    fireEvent.click(within(form).getByRole("button", { name: "Começar" }));
    await waitFor(() => expect(api.pedido).toHaveBeenCalledWith("db", null, "Comece pelo banco."));
    await waitFor(() => expect(abrirTerminal).toHaveBeenCalled());
    expect(abrirTerminal.mock.calls[0][0]).toBe("faturamento-exemplo");
    expect(abrirTerminal.mock.calls[0][1]).toMatchObject({
      promptInicial: "Mapeie as funcionalidades do projeto pelo ctower",
    });
    expect(screen.getByRole("button", { name: "Encerrar a sessão" })).toBeInTheDocument();
  });

  it("informa o estado da sessão de mapeamento para a aba pedir confirmação ao fechar (#257)", async () => {
    const api = apiFalsa();
    const apiTerminal = {
      abrir: vi.fn().mockResolvedValue(7),
      escrever: vi.fn().mockResolvedValue(undefined),
      redimensionar: vi.fn().mockResolvedValue(undefined),
      encerrar: vi.fn().mockResolvedValue(undefined),
      enviarPrompt: vi.fn().mockResolvedValue(undefined),
    } as unknown as ApiTerminal;
    const aoMudarConversa = vi.fn();
    abrir(api, analiseFalsa(), apiTerminal, aoMudarConversa);
    await screen.findByRole("listitem", { name: "F-COB-001: Calcular encargos" });
    // Sem sessão, nada de conversa viva.
    expect(aoMudarConversa).not.toHaveBeenCalledWith("aberta");
    fireEvent.click(screen.getByRole("button", { name: "Mapear com o Claude" }));
    fireEvent.click(screen.getByRole("button", { name: "Começar" }));
    await waitFor(() => expect(aoMudarConversa).toHaveBeenLastCalledWith("aberta"));
    // Encerrar a sessão devolve a aba ao estado sem conversa.
    fireEvent.click(screen.getByRole("button", { name: "Encerrar a sessão" }));
    await waitFor(() => expect(aoMudarConversa).toHaveBeenLastCalledWith("nenhuma"));
  });

  it("árvore lateral abre a página só da funcionalidade escolhida", async () => {
    abrir(apiFalsa());
    const arvore = await screen.findByRole("navigation", { name: "Árvore de funcionalidades" });
    expect(within(arvore).getByRole("button", { name: /Cobrança 1 · 1 a revisar/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^A revisar/ }));
    const cancelar = within(arvore).getByRole("button", { name: /F-CAN-001 Cancelar fatura/ });
    expect(cancelar).toHaveClass("fora-do-filtro");

    fireEvent.click(cancelar);
    const pagina = screen.getByRole("article", { name: "Página de F-CAN-001" });
    expect(within(pagina).getByRole("heading", { name: "Cancelar fatura" })).toBeInTheDocument();
    expect(await within(pagina).findByRole("region", { name: "Indicadores" })).toBeInTheDocument();
    // Só ela: nem as outras, nem a lista, nem os pontos sem tratamento.
    expect(screen.queryByRole("listitem", { name: "F-COB-001: Calcular encargos" })).toBeNull();
    expect(screen.queryByRole("region", { name: "Funcionalidades" })).toBeNull();
    expect(screen.queryByRole("region", { name: "Ainda sem mapa" })).toBeNull();
    expect(cancelar).toHaveAttribute("aria-current", "true");

    // A árvore troca de página; "Todas as funcionalidades" volta à lista.
    fireEvent.click(within(arvore).getByRole("button", { name: /F-COB-001 Calcular encargos/ }));
    expect(screen.getByRole("article", { name: "Página de F-COB-001" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "← Todas as funcionalidades" }));
    expect(screen.queryByRole("article")).toBeNull();
    expect(screen.getByRole("region", { name: "Funcionalidades" })).toBeInTheDocument();

    // Recolher a área esconde as funcionalidades dela na árvore.
    const area = within(arvore).getByRole("button", { name: /Cancelamento/ });
    fireEvent.click(area);
    expect(area).toHaveAttribute("aria-expanded", "false");
    expect(within(arvore).queryByRole("button", { name: /F-CAN-001/ })).toBeNull();
  });

  it("mapa vazio convida a mapear", async () => {
    abrir(
      apiFalsa({
        ...MAPA,
        areas: [],
        total: 0,
        pendentes: 0,
        documentadas: 0,
        justificados: [],
        documento: null,
      }),
    );
    expect(await screen.findByText(/O mapa ainda está vazio/)).toBeInTheDocument();
    // Os pontos de entrada ficam atrás de "Ver os pontos de entrada".
    expect(screen.getByText("Ver os pontos de entrada")).toBeInTheDocument();
    expect(screen.getByText("PKG_SITUACAO.PRC_SUSPENDER_COBRANCA")).not.toBeVisible();
    expect(screen.queryByRole("button", { name: "Exportar Markdown" })).toBeNull();
  });

  it("conta os manuais aprovados quando o mapa informa (#327)", async () => {
    const mapa = {
      ...MAPA,
      areas: MAPA.areas.map((a) => ({
        ...a,
        funcionalidades: a.funcionalidades.map((f) => ({ ...f, comManual: f.id === "F-CAN-001" })),
      })),
    };
    abrir(apiFalsa(mapa));
    const progresso = await screen.findByRole("region", { name: "Progresso do mapa" });
    const etapas = within(progresso).getAllByRole("listitem").map((e) => e.textContent);
    expect(etapas[3]).toBe("1 de 2 com manual");
  });

  it("mostra o progresso do mapa em quatro etapas e o código já mapeado", async () => {
    abrir(apiFalsa());
    const progresso = await screen.findByRole("region", { name: "Progresso do mapa" });
    const etapas = within(progresso).getAllByRole("listitem").map((e) => e.textContent);
    expect(etapas).toEqual(["2 de 2 mapeadas", "1 de 2 aceitas", "1 de 2 explicadas", "— com manual"]);
    expect(within(progresso).getByRole("progressbar", { name: "Código já mapeado" })).toHaveAttribute(
      "aria-valuenow",
      "93",
    );
  });

  it("filtros mostram a contagem", async () => {
    abrir(apiFalsa());
    expect(await screen.findByRole("button", { name: "Todas (2)" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "A revisar (1)" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Aceitas (1)" })).toBeInTheDocument();
  });

  it("Mapear numa lacuna abre a sessão com a pasta preenchida e Código lista todas", async () => {
    const { navegar } = abrir(apiFalsa());
    fireEvent.click(await screen.findByRole("button", { name: "Mapear db" }));
    const form = screen.getByRole("form", { name: "Começar o mapeamento" });
    expect(within(form).getByLabelText("Pasta (opcional)")).toHaveValue("db");
    fireEvent.click(screen.getByRole("button", { name: "Ver todas as pastas em Código" }));
    expect(navegar).toHaveBeenCalledWith({ tipo: "codigo", projeto: "faturamento-exemplo" });
  });
});
