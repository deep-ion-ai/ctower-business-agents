import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ListaFuncionalidades } from "./ListaFuncionalidades";
import type { ApiFuncionalidades, Catalogo } from "./funcionalidades";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

const CATALOGO: Catalogo = {
  total: 2,
  documentadas: 1,
  documento: "faturamento-exemplo/funcionalidades/catalogo.md",
  origem: "A-0001",
  areas: [
    {
      area: "cancelamento",
      nome: "Cancelamento",
      funcionalidades: [
        {
          id: "F-CAN-001",
          nome: "Cancelar fatura",
          descricao: "Cancela uma fatura aberta.",
          atores: [],
          pontos: ["FrmCancelamento"],
          situacao: "mantida",
          documento:
            "faturamento-exemplo/funcionalidades/cancelamento/cancelar-fatura/negocial.md",
          explicacao: "A-0002",
        },
      ],
    },
    {
      area: "cobranca",
      nome: "Cobrança",
      funcionalidades: [
        {
          id: "F-COB-001",
          nome: "Calcular encargos",
          descricao: "Multa e juros.",
          atores: [],
          pontos: ["PKG_COBRANCA.FN_CALCULAR_MULTA"],
          situacao: "sem-ponto-de-entrada",
          documento: null,
          explicacao: null,
        },
      ],
    },
  ],
};

function api(catalogo: Catalogo = CATALOGO): ApiFuncionalidades {
  return {
    catalogo: vi.fn().mockResolvedValue(catalogo),
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
    completude: vi.fn(),
    criarMapeamento: vi.fn(),
    escolherDestino: vi.fn().mockResolvedValue("/tmp/catalogo.docx"),
    exportar: vi.fn().mockResolvedValue("/tmp/catalogo.docx"),
  };
}

describe("ListaFuncionalidades", () => {
  it("lista por área, com a situação, e abre o documento e o catálogo", async () => {
    const navegar = vi.fn();
    render(<ListaFuncionalidades projeto="faturamento-exemplo" navegar={navegar} api={api()} />);
    const lista = await screen.findByRole("region", { name: "Funcionalidades" });
    await within(lista).findByText("Cancelar fatura");
    expect(within(lista).getByText(/2 funcionalidades · 1 documentada/)).toBeInTheDocument();
    expect(within(lista).getByText("documentada")).toBeInTheDocument();
    expect(within(lista).getByText("só mapeada")).toBeInTheDocument();
    expect(within(lista).getByText("sem ponto de entrada")).toBeInTheDocument();
    fireEvent.click(within(lista).getByRole("button", { name: "documento" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "base",
      documento: "faturamento-exemplo/funcionalidades/cancelamento/cancelar-fatura/negocial.md",
    });
    fireEvent.click(within(lista).getByRole("button", { name: "abrir o catálogo" }));
    expect(navegar).toHaveBeenLastCalledWith({
      tipo: "base",
      documento: "faturamento-exemplo/funcionalidades/catalogo.md",
    });
  });

  it("Explicar abre a análise criada com a funcionalidade", async () => {
    const navegar = vi.fn();
    const falsa = api();
    render(<ListaFuncionalidades projeto="faturamento-exemplo" navegar={navegar} api={falsa} />);
    fireEvent.click(await screen.findByRole("button", { name: "Explicar" }));
    expect(falsa.explicar).toHaveBeenCalledWith("faturamento-exemplo", "F-COB-001");
    await waitFor(() =>
      expect(navegar).toHaveBeenCalledWith({
        tipo: "analise",
        projeto: "faturamento-exemplo",
        aberta: {
          id: "A-0003",
          abertura: { tipo: "nova", promptInicial: "Inicie a análise A-0003" },
        },
      }),
    );
    expect(screen.getByRole("button", { name: "Explicar de novo" })).toBeInTheDocument();
  });

  it("exporta o catálogo em Word", async () => {
    const falsa = api();
    render(<ListaFuncionalidades projeto="faturamento-exemplo" navegar={vi.fn()} api={falsa} />);
    fireEvent.click(await screen.findByRole("button", { name: "Word" }));
    expect(await screen.findByText(/Catálogo exportado em \/tmp\/catalogo.docx/)).toBeInTheDocument();
    expect(falsa.exportar).toHaveBeenCalledWith("faturamento-exemplo", "/tmp/catalogo.docx", "word");
  });

  it("sem catálogo, diz como mapear", async () => {
    render(
      <ListaFuncionalidades
        projeto="faturamento-exemplo"
        navegar={vi.fn()}
        api={api({ areas: [], total: 0, documentadas: 0, documento: null, origem: null })}
      />,
    );
    expect(await screen.findByText(/Nenhuma funcionalidade mapeada ainda/)).toBeInTheDocument();
  });
});
