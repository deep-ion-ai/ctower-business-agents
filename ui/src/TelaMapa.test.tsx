import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ApiAnalise, AvisoAnalise } from "./analise";
import type { ApiMapa, Funcionalidade, MapaFuncionalidades } from "./funcionalidades";
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

function funcionalidade(f: Partial<Funcionalidade> & Pick<Funcionalidade, "id" | "nome">): Funcionalidade {
  return {
    area: "cobranca",
    areaNome: "Cobrança",
    descricao: `${f.nome}.`,
    atores: [],
    pontos: [
      {
        nome: "PKG_COBRANCA.FN_CALCULAR_MULTA",
        chave: "db/03_pkg_cobranca.sql#PKG_COBRANCA.FN_CALCULAR_MULTA",
        tipo: "rotina_banco",
        arquivo: "db/03_pkg_cobranca.sql",
        linhaInicial: 10,
        linhaFinal: 10,
        trecho: "FUNCTION FN_CALCULAR_MULTA",
        foraDaLista: false,
      },
      {
        nome: "EncargosService.calcular",
        chave: "backend/E.java#EncargosService.calcular",
        tipo: "servico",
        arquivo: "backend/E.java",
        linhaInicial: 5,
        linhaFinal: 5,
        trecho: "public void calcular()",
        foraDaLista: false,
      },
    ],
    dadosLidos: [],
    dadosGravados: [],
    relacionadas: [],
    situacao: "pendente",
    juntadaEm: null,
    editada: false,
    documento: null,
    explicacao: null,
    semPontoDeEntrada: false,
    ...f,
  };
}

const MAPA: MapaFuncionalidades = {
  areas: [
    {
      area: "cancelamento",
      nome: "Cancelamento",
      funcionalidades: [
        funcionalidade({
          id: "F-CAN-001",
          nome: "Cancelar fatura",
          area: "cancelamento",
          areaNome: "Cancelamento",
          situacao: "aceita",
          documento: "faturamento-exemplo/funcionalidades/cancelamento/cancelar-fatura/negocial.md",
          explicacao: {
            id: "A-0002",
            titulo: "F-CAN-001 · Cancelar fatura",
            fluxo: "explicar",
            estado: "aprovada",
            atualizadaMs: 1,
          },
        }),
      ],
    },
    {
      area: "cobranca",
      nome: "Cobrança",
      funcionalidades: [funcionalidade({ id: "F-COB-001", nome: "Calcular encargos" })],
    },
  ],
  fechadas: [],
  total: 2,
  pendentes: 1,
  documentadas: 1,
  justificados: [
    {
      chave: "db/03_pkg_cobranca.sql#PKG_COBRANCA.FN_PARAMETRO",
      tipo: "rotina_banco",
      nome: "PKG_COBRANCA.FN_PARAMETRO",
      arquivo: "db/03_pkg_cobranca.sql",
      motivo: "utilitario",
      justificativa: "Lê parâmetros.",
    },
  ],
  completude: {
    total: 15,
    semTratamento: [
      {
        chave: "db/04_pkg_situacao.sql#PKG_SITUACAO.PRC_SUSPENDER_COBRANCA",
        tipo: "rotina_banco",
        nome: "PKG_SITUACAO.PRC_SUSPENDER_COBRANCA",
        arquivo: "db/04_pkg_situacao.sql",
        linha: 40,
      },
    ],
  },
  erroCompletude: null,
  documento: "faturamento-exemplo/funcionalidades/mapa.md",
  versao: 3,
};

function apiFalsa(mapa: MapaFuncionalidades = MAPA) {
  const api: ApiMapa = {
    mapa: vi.fn().mockResolvedValue(mapa),
    revisar: vi.fn().mockResolvedValue({ ...mapa, pendentes: 0 }),
    pedido: vi.fn().mockResolvedValue("Mapeie as funcionalidades do projeto pelo ctower"),
    explicar: vi.fn().mockResolvedValue({
      analise: {
        id: "A-0003",
        titulo: "F-COB-001 · Calcular encargos",
        fluxo: "explicar",
        estado: "criada",
        atualizadaMs: 1,
      },
      promptInicial: "Inicie a análise A-0003",
    }),
    escolherDestino: vi.fn().mockResolvedValue("/tmp/mapa.md"),
    exportar: vi.fn().mockResolvedValue("/tmp/mapa.md"),
  };
  return api;
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

function abrir(api: ApiMapa, analise = analiseFalsa(), apiTerminal?: ApiTerminal) {
  const navegar = vi.fn();
  render(
    <TelaMapa
      projetos={PROJETOS as never}
      api={api}
      apiAnalise={analise.api}
      apiTerminal={apiTerminal}
      criarTela={() => telaFalsa() as never}
      navegar={navegar}
    />,
  );
  return { navegar, analise };
}

describe("TelaMapa", () => {
  it("mostra o mapa por área, o resumo e os pontos sem tratamento", async () => {
    abrir(apiFalsa());
    expect(await screen.findByRole("region", { name: "F-COB-001: Calcular encargos" })).toBeInTheDocument();
    expect(screen.getByLabelText("Resumo do mapa")).toHaveTextContent(
      "2 funcionalidades · 2 áreas · 1 a revisar · 1 documentadas · 15 pontos de entrada, 1 sem tratamento",
    );
    expect(screen.getByRole("heading", { name: /Cancelamento/ })).toBeInTheDocument();
    const sem = screen.getByRole("list", { name: "Sem tratamento" });
    expect(sem).toHaveTextContent("PKG_SITUACAO.PRC_SUSPENDER_COBRANCA");
    expect(screen.getByRole("list", { name: "Justificados" })).toHaveTextContent("utilitário");
  });

  it("aceita, filtra e desfaz uma justificativa", async () => {
    const api = apiFalsa();
    abrir(api);
    const cartao = await screen.findByRole("region", { name: "F-COB-001: Calcular encargos" });
    fireEvent.click(within(cartao).getByRole("button", { name: "Aceitar" }));
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

    fireEvent.click(screen.getByRole("button", { name: "A revisar" }));
    expect(screen.queryByRole("region", { name: "F-CAN-001: Cancelar fatura" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Todas" }));
    fireEvent.change(screen.getByRole("searchbox", { name: "Buscar funcionalidade" }), {
      target: { value: "cancelar" },
    });
    expect(screen.queryByRole("region", { name: "F-COB-001: Calcular encargos" })).toBeNull();
    expect(screen.getByRole("region", { name: "F-CAN-001: Cancelar fatura" })).toBeInTheDocument();

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
    const cartao = await screen.findByRole("region", { name: "F-COB-001: Calcular encargos" });
    fireEvent.click(within(cartao).getByRole("button", { name: "Dividir" }));
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
    const cartao = await screen.findByRole("region", { name: "F-CAN-001: Cancelar fatura" });
    fireEvent.click(within(cartao).getByRole("button", { name: "Documento negocial" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "base",
      documento: "faturamento-exemplo/funcionalidades/cancelamento/cancelar-fatura/negocial.md",
    });
    // Já explicada: "Explicar" desabilitado e o link para a análise.
    expect(within(cartao).getByRole("button", { name: "Explicar" })).toBeDisabled();
    fireEvent.click(within(cartao).getByRole("button", { name: "Explicação A-0002 (aprovada)" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: "faturamento-exemplo",
      aberta: { id: "A-0002", abertura: { tipo: "reabrir" } },
    });
    const cobranca = screen.getByRole("region", { name: "F-COB-001: Calcular encargos" });
    fireEvent.click(within(cobranca).getByRole("button", { name: "Explicar" }));
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
    await screen.findByRole("region", { name: "F-COB-001: Calcular encargos" });
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
    await screen.findByRole("region", { name: "F-COB-001: Calcular encargos" });
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

  it("mostra as evidências de código só quando o analista pede", async () => {
    abrir(apiFalsa());
    const cartao = await screen.findByRole("region", { name: "F-COB-001: Calcular encargos" });
    const opcao = screen.getByRole("checkbox", { name: "Mostrar evidências de código" });
    expect(opcao).not.toBeChecked();
    // Desligada: nem os pontos de entrada (arquivo e linha) nem os trechos, só a contagem.
    expect(within(cartao).queryByRole("list", { name: "Pontos de entrada de F-COB-001" })).toBeNull();
    expect(cartao).not.toHaveTextContent("db/03_pkg_cobranca.sql");
    expect(cartao).toHaveTextContent("2 pontos de entrada");
    fireEvent.click(opcao);
    const pontos = within(cartao).getByRole("list", { name: "Pontos de entrada de F-COB-001" });
    expect(pontos).toHaveTextContent("PKG_COBRANCA.FN_CALCULAR_MULTA · rotina de banco · db/03_pkg_cobranca.sql:10");
    const ev = within(cartao).getByLabelText("Evidência de PKG_COBRANCA.FN_CALCULAR_MULTA");
    expect(ev).toHaveTextContent("FUNCTION FN_CALCULAR_MULTA");
    fireEvent.click(opcao);
    expect(within(cartao).queryByRole("list", { name: "Pontos de entrada de F-COB-001" })).toBeNull();
  });

  it("árvore lateral abre a página só da funcionalidade escolhida", async () => {
    abrir(apiFalsa());
    const arvore = await screen.findByRole("navigation", { name: "Árvore de funcionalidades" });
    expect(within(arvore).getByRole("button", { name: /Cobrança 1 · 1 a revisar/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "A revisar" }));
    const cancelar = within(arvore).getByRole("button", { name: /F-CAN-001 Cancelar fatura/ });
    expect(cancelar).toHaveClass("fora-do-filtro");

    fireEvent.click(cancelar);
    const pagina = screen.getByRole("article", { name: "Página de F-CAN-001" });
    expect(within(pagina).getByRole("heading", { name: "F-CAN-001 · Cancelar fatura" })).toBeInTheDocument();
    expect(within(pagina).getByRole("region", { name: "F-CAN-001: Cancelar fatura" })).toBeInTheDocument();
    // Só ela: nem as outras, nem a lista, nem os pontos sem tratamento.
    expect(screen.queryByRole("region", { name: "F-COB-001: Calcular encargos" })).toBeNull();
    expect(screen.queryByRole("region", { name: "Funcionalidades" })).toBeNull();
    expect(screen.queryByRole("region", { name: "Pontos de entrada sem funcionalidade" })).toBeNull();
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
    expect(screen.queryByRole("button", { name: "Exportar Markdown" })).toBeNull();
  });
});
