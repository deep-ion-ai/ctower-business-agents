import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { EntregaveisDemanda } from "./EntregaveisDemanda";
import {
  acaoDoDestino,
  type ApiEntregaveisDemanda,
  type EntregavelVisto,
  type VistaEntregaveis,
} from "./entregaveisDemanda";
import type { ApiTerminal } from "./terminal";
import { telaFalsa } from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

const PROJETO = "faturamento-exemplo";
const DEMANDA = "D-0001";

const BASE = {
  analise: "A-0001",
  titulo: "Parcelamento para pessoa física",
  fluxo: "impacto" as const,
  regras: [
    { id: "RN-FAT-001", titulo: "Pessoa física não parcela" },
    { id: "RN-FAT-002", titulo: "Parcela mínima por parâmetro" },
  ],
  decisoes: [
    { numero: 1, texto: "A parcela mínima continua a mesma?", resposta: "Subir para R$ 100,00" },
  ],
  perguntas: [{ numero: 2, texto: "O limite de 3 parcelas vale para todo cliente PF?", decisao: false }],
  funcionalidades: ["F-FAT-001"],
};

const TIPOS: VistaEntregaveis["tipos"] = [
  {
    id: "historia-usuario",
    nome: "História de usuário",
    descricao: "Como, quero, para.",
    ciclo: "por_demanda",
    geracao: "agente",
    destino: "exportar",
    padrao: true,
  },
  {
    id: "caso-de-uso",
    nome: "Especificação de caso de uso",
    descricao: "Atores e fluxos.",
    ciclo: "vivo",
    geracao: "agente",
    destino: "exportar",
    padrao: false,
  },
  {
    id: "catalogo-regras",
    nome: "Catálogo de regras de negócio",
    descricao: "As regras da funcionalidade.",
    ciclo: "vivo",
    geracao: "app",
    destino: "exportar",
    padrao: false,
    indisponivel: "Montado pelo app, sem IA: a montagem na demanda chega numa próxima versão.",
  },
];

function historia(parcial: Partial<EntregavelVisto> = {}): EntregavelVisto {
  return {
    numero: 1,
    tipo: "historia-usuario",
    nome: "História de usuário",
    ciclo: "por_demanda",
    analise: "A-0001",
    finalidade: "demanda",
    destino: "exportar",
    situacao: "rascunho",
    versao: 1,
    versoes: [1],
    texto: "## Parcelar fatura de pessoa física\n\n**Como (papel):** atendente",
    editado: false,
    itens: [
      {
        numero: 1,
        texto: "## Parcelar fatura de pessoa física",
        regras: [
          {
            id: "RN-FAT-001",
            titulo: "Pessoa física não parcela",
            evidencias: [
              {
                arquivo: "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java",
                inicio: 30,
                fim: 32,
                trecho: "if (fatura.tipoCliente() == TipoCliente.PF) {",
              },
            ],
          },
        ],
      },
    ],
    semCobertura: ["RN-FAT-002"],
    avisoCobertura: "RN-FAT-002 sem história",
    conferir: ["Cada item cita as regras certas.", "Cobertura: RN-FAT-002 sem história."],
    envios: [],
    prompt: "Gere o entregável E-1 da demanda D-0001 pelo ctower",
    ...parcial,
  };
}

function vista(parcial: Partial<VistaEntregaveis> = {}): VistaEntregaveis {
  return {
    demanda: DEMANDA,
    aberta: true,
    analises: [BASE],
    tipos: TIPOS,
    entregaveis: [],
    funcionalidades: [
      {
        id: "F-FAT-001",
        nome: "Parcelar fatura",
        daAnalise: true,
        documentos: [{ tipo: "caso-de-uso", estado: "sem documento" }],
      },
    ],
    ...parcial,
  };
}

function apiFalsa(inicial: VistaEntregaveis) {
  let atual = inicial;
  const api = {
    vista: vi.fn(async () => atual),
    gerar: vi.fn(async () => ({ numero: 1, versao: 1, prompt: "Gere o entregável E-1 da demanda D-0001 pelo ctower" })),
    editar: vi.fn(async () => undefined),
    marcarRevisado: vi.fn(async () => undefined),
    escolherDestino: vi.fn(async (nome: string) => `/tmp/${nome}.md`),
    exportar: vi.fn(async (_p: string, _d: string, _n: number, destino: string) => destino),
    publicar: vi.fn(async () => "faturamento-exemplo/demandas/D-0001/entregaveis/E-1-historia-usuario.md"),
    comentario: vi.fn(async () => ({ texto: "## História", link: "https://rastreador.exemplo.invalid/FAT-12", chave: "FAT-12" })),
    copiar: vi.fn(async () => undefined),
  } satisfies ApiEntregaveisDemanda;
  return {
    api,
    trocar: (v: VistaEntregaveis) => {
      atual = v;
    },
  };
}

function terminalFalso(): ApiTerminal {
  return {
    abrir: vi.fn().mockResolvedValue(7),
    escrever: vi.fn().mockResolvedValue(undefined),
    redimensionar: vi.fn().mockResolvedValue(undefined),
    encerrar: vi.fn().mockResolvedValue(undefined),
    enviarPrompt: vi.fn().mockResolvedValue(undefined),
  } as unknown as ApiTerminal;
}

function abrir(api: ApiEntregaveisDemanda, extra: { abrirLink?: () => Promise<void>; apiTerminal?: ApiTerminal } = {}) {
  return render(
    <EntregaveisDemanda
      projeto={PROJETO}
      demanda={DEMANDA}
      api={api}
      abrirLink={extra.abrirLink}
      apiTerminal={extra.apiTerminal ?? terminalFalso()}
      criarTela={() => telaFalsa() as never}
    />,
  );
}

describe("EntregaveisDemanda", () => {
  it("sem análise validada, a seção não oferece a geração", async () => {
    const { api } = apiFalsa(vista({ analises: [] }));
    abrir(api);
    expect(await screen.findByText(/Sem análise validada/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Gerar/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Gerar entregável" })).not.toBeInTheDocument();
  });

  it("mostra a base e só os tipos habilitados, com o padrão marcado", async () => {
    const { api } = apiFalsa(vista());
    abrir(api);
    const base = await screen.findByRole("region", { name: "Análise validada de base" });
    expect(within(base).getByText("RN-FAT-001")).toBeInTheDocument();
    expect(within(base).getByText(/Subir para R\$ 100,00/)).toBeInTheDocument();
    expect(within(base).getByText(/O limite de 3 parcelas/)).toBeInTheDocument();

    const tipos = screen.getByRole("region", { name: "Gerar entregável" });
    const linhas = within(tipos).getAllByRole("listitem");
    expect(linhas).toHaveLength(3);
    expect(within(linhas[0]).getByText("História de usuário")).toBeInTheDocument();
    expect(within(linhas[0]).getByText("padrão")).toBeInTheDocument();
    // O montado pelo app fica desabilitado, com o motivo.
    expect(screen.getByRole("button", { name: "Gerar a v1.0 de Catálogo de regras de negócio" })).toBeDisabled();
    expect(screen.getAllByText(/Montado pelo app/).length).toBeGreaterThan(0);
  });

  it("gerar abre o rascunho e a sessão do agente com o prompt", async () => {
    const { api } = apiFalsa(vista());
    const apiTerminal = terminalFalso();
    abrir(api, { apiTerminal });
    fireEvent.click(await screen.findByRole("button", { name: "Gerar História de usuário" }));
    await waitFor(() =>
      expect(api.gerar).toHaveBeenCalledWith(PROJETO, DEMANDA, {
        tipo: "historia-usuario",
        analise: "A-0001",
        funcionalidade: null,
        versaoInicial: false,
      }),
    );
    expect(await screen.findByRole("region", { name: "Sessão do agente" })).toBeInTheDocument();
    await waitFor(() => expect(apiTerminal.abrir).toHaveBeenCalled());
    expect((apiTerminal.abrir as ReturnType<typeof vi.fn>).mock.calls[0][1]).toMatchObject({
      promptInicial: "Gere o entregável E-1 da demanda D-0001 pelo ctower",
    });
  });

  it("o tipo vivo sem documento oferece gerar a v1.0 antes", async () => {
    const { api } = apiFalsa(vista());
    abrir(api);
    const botao = await screen.findByRole("button", { name: "Gerar a v1.0 de Especificação de caso de uso" });
    expect(screen.getByText(/Sem documento: gere a v1.0/)).toBeInTheDocument();
    fireEvent.click(botao);
    await waitFor(() =>
      expect(api.gerar).toHaveBeenCalledWith(PROJETO, DEMANDA, {
        tipo: "caso-de-uso",
        analise: "A-0001",
        funcionalidade: "F-FAT-001",
        versaoInicial: true,
      }),
    );
  });

  it("o tipo vivo com documento gera a proposta de versão", async () => {
    const { api } = apiFalsa(
      vista({
        funcionalidades: [
          {
            id: "F-FAT-001",
            nome: "Parcelar fatura",
            daAnalise: true,
            documentos: [{ tipo: "caso-de-uso", versao: "1.3", estado: "em dia" }],
          },
        ],
      }),
    );
    abrir(api);
    expect(await screen.findByText("Especificação de caso de uso v1.3, em dia.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Gerar a proposta de versão de Especificação de caso de uso" }));
    await waitFor(() =>
      expect(api.gerar).toHaveBeenCalledWith(PROJETO, DEMANDA, expect.objectContaining({ versaoInicial: false })),
    );
  });

  it("rascunho com a faixa de revisão, o aviso de cobertura e o envio bloqueado até a revisão", async () => {
    const { api, trocar } = apiFalsa(vista({ entregaveis: [historia()] }));
    abrir(api);
    const cartao = await screen.findByRole("listitem", { name: "E-1 · História de usuário" });
    const faixa = within(cartao).getByRole("note", { name: "Revisão final obrigatória" });
    expect(within(faixa).getByText("Cada item cita as regras certas.")).toBeInTheDocument();
    expect(within(cartao).getAllByText("RN-FAT-002 sem história").length).toBeGreaterThan(0);
    expect(within(cartao).getByText("rascunho")).toBeInTheDocument();
    const exportar = within(cartao).getByRole("button", { name: "Exportar" });
    expect(exportar).toBeDisabled();

    trocar(vista({ entregaveis: [historia({ situacao: "revisado", revisadoPor: "ana", conferir: [] })] }));
    fireEvent.click(within(cartao).getByRole("button", { name: "Marcar como revisado" }));
    await waitFor(() => expect(api.marcarRevisado).toHaveBeenCalledWith(PROJETO, DEMANDA, 1));
    const revisado = await screen.findByRole("listitem", { name: "E-1 · História de usuário" });
    await waitFor(() => expect(within(revisado).getByRole("button", { name: "Exportar" })).toBeEnabled());
    expect(within(revisado).queryByRole("note", { name: "Revisão final obrigatória" })).not.toBeInTheDocument();
    expect(within(revisado).getByRole("button", { name: "Marcar como revisado" })).toBeDisabled();

    fireEvent.click(within(revisado).getByRole("button", { name: "Exportar" }));
    await waitFor(() =>
      expect(api.exportar).toHaveBeenCalledWith(
        PROJETO,
        DEMANDA,
        1,
        "/tmp/D-0001-E-1-historia-usuario.md",
      ),
    );
    expect(await screen.findByText(/exportado em/)).toBeInTheDocument();
  });

  it("uma edição depois da revisão volta o entregável para rascunho", async () => {
    const { api, trocar } = apiFalsa(
      vista({ entregaveis: [historia({ situacao: "revisado", revisadoPor: "ana" })] }),
    );
    abrir(api);
    const cartao = await screen.findByRole("listitem", { name: "E-1 · História de usuário" });
    await waitFor(() => expect(within(cartao).getByRole("button", { name: "Exportar" })).toBeEnabled());
    fireEvent.click(within(cartao).getByRole("button", { name: "Editar" }));
    const form = within(cartao).getByRole("form", { name: "Editar E-1 · História de usuário" });
    fireEvent.change(within(form).getByLabelText("Texto do entregável"), {
      target: { value: "## Parcelar fatura\n\nTexto revisto." },
    });
    trocar(vista({ entregaveis: [historia({ situacao: "rascunho", editado: true })] }));
    fireEvent.click(within(form).getByRole("button", { name: "Salvar a edição" }));
    await waitFor(() =>
      expect(api.editar).toHaveBeenCalledWith(PROJETO, DEMANDA, 1, "## Parcelar fatura\n\nTexto revisto."),
    );
    await waitFor(() => expect(within(cartao).getByRole("button", { name: "Exportar" })).toBeDisabled());
    expect(within(cartao).getByRole("note", { name: "Revisão final obrigatória" })).toBeInTheDocument();
  });

  it("gerar de novo pede a versão seguinte do mesmo entregável", async () => {
    const { api } = apiFalsa(vista({ entregaveis: [historia({ versao: 2, versoes: [1, 2] })] }));
    abrir(api);
    const cartao = await screen.findByRole("listitem", { name: "E-1 · História de usuário" });
    expect(within(cartao).getByText(/anteriores guardadas: v1/)).toBeInTheDocument();
    fireEvent.click(within(cartao).getByRole("button", { name: "Gerar de novo" }));
    await waitFor(() =>
      expect(api.gerar).toHaveBeenCalledWith(PROJETO, DEMANDA, {
        tipo: "historia-usuario",
        analise: "A-0001",
        funcionalidade: null,
        versaoInicial: false,
      }),
    );
  });

  it("mostrar evidências exibe o trecho de cada regra citada", async () => {
    const { api } = apiFalsa(vista({ entregaveis: [historia()] }));
    abrir(api);
    const cartao = await screen.findByRole("listitem", { name: "E-1 · História de usuário" });
    expect(within(cartao).queryByRole("list", { name: "Evidências" })).not.toBeInTheDocument();
    fireEvent.click(within(cartao).getByRole("button", { name: "Mostrar evidências" }));
    const lista = within(cartao).getByRole("list", { name: "Evidências" });
    expect(within(lista).getByText(/ParcelamentoService.java:30-32/)).toBeInTheDocument();
    expect(within(lista).getByText("if (fatura.tipoCliente() == TipoCliente.PF) {")).toBeInTheDocument();
  });

  it("a proposta do vivo mostra o texto anterior riscado e publica a versão", async () => {
    const vivo = historia({
      numero: 2,
      tipo: "caso-de-uso",
      nome: "Especificação de caso de uso",
      ciclo: "vivo",
      funcionalidade: "F-FAT-001",
      situacao: "revisado",
      avisoCobertura: null,
      semCobertura: [],
      proposta: {
        caminho: "funcionalidades/cobranca/parcelar-fatura/caso-de-uso.md",
        versao: "1.4",
        base: "1.3",
        mudancas: "Pessoa física passa a parcelar.",
        trechos: [
          { marca: "mantido", texto: "# Parcelar fatura" },
          {
            marca: "alterado",
            texto: "- O sistema parcela em até 3 vezes.",
            anterior: "- O sistema recusa pessoa física.",
          },
        ],
        historico: [
          { versao: "1.3", data: "2026-11-01", demanda: "D-0000", revisor: "ana", mudancas: "Ajuste" },
        ],
      },
    });
    const { api } = apiFalsa(vista({ entregaveis: [vivo] }));
    abrir(api);
    const cartao = await screen.findByRole("listitem", { name: "E-2 · Especificação de caso de uso" });
    expect(within(cartao).getByLabelText("Texto anterior")).toHaveTextContent("O sistema recusa pessoa física.");
    expect(within(cartao).getByLabelText("Alterado")).toHaveTextContent("O sistema parcela em até 3 vezes.");
    expect(within(cartao).getByText("Histórico de revisões (1)")).toBeInTheDocument();
    fireEvent.click(within(cartao).getByRole("button", { name: "Publicar a versão 1.4" }));
    await waitFor(() => expect(api.publicar).toHaveBeenCalledWith(PROJETO, DEMANDA, 2));
  });

  it("comentário no rastreador copia o texto e abre a issue", async () => {
    const { api } = apiFalsa(
      vista({ entregaveis: [historia({ situacao: "revisado", destino: "comentario_rastreador" })] }),
    );
    const abrirLink = vi.fn(async () => undefined);
    abrir(api, { abrirLink });
    const cartao = await screen.findByRole("listitem", { name: "E-1 · História de usuário" });
    fireEvent.click(within(cartao).getByRole("button", { name: "Enviar ao rastreador" }));
    await waitFor(() => expect(api.copiar).toHaveBeenCalledWith("## História"));
    expect(abrirLink).toHaveBeenCalledWith(PROJETO, "https://rastreador.exemplo.invalid/FAT-12");
    expect(await screen.findByText(/cole como comentário na issue FAT-12/)).toBeInTheDocument();
  });

  it("o botão do destino segue o configurado para o tipo", () => {
    expect(acaoDoDestino({ ciclo: "por_demanda", destino: "exportar", proposta: null }).texto).toBe("Exportar");
    expect(acaoDoDestino({ ciclo: "por_demanda", destino: "repositorio_docs", proposta: null }).texto).toBe(
      "Publicar",
    );
    expect(acaoDoDestino({ ciclo: "vivo", destino: "exportar", proposta: null }).tipo).toBe("publicar");
  });
});
