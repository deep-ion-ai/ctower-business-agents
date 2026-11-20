import { fireEvent, render, screen, within } from "@testing-library/react";
import { CoberturaProjeto } from "./CoberturaProjeto";
import type { Cobertura } from "./cobertura";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const area = {
  idadeAntigaDias: null,
  semConfirmacaoRecente: 0,
  aRevalidar: 0,
  arquivos: 1,
  funcionalidades: 0,
  funcionalidadesSemDocumento: 0,
};

const COBERTURA: Cobertura = {
  areas: [
    {
      ...area,
      id: "cancelamento",
      nome: "Cancelamento",
      regras: 0,
      termos: 0,
      documentos: 0,
      funcionalidades: 1,
      funcionalidadesSemDocumento: 1,
      centralidade: 4,
      idadeRecenteDias: null,
      lacuna: true,
      calor: 1,
    },
    {
      ...area,
      id: "cobranca",
      nome: "Cobrança",
      regras: 2,
      termos: 1,
      documentos: 1,
      aRevalidar: 1,
      semConfirmacaoRecente: 1,
      centralidade: 12,
      idadeRecenteDias: 3,
      lacuna: false,
      calor: 0,
    },
  ],
  pastas: [
    {
      pasta: "db",
      arquivos: 4,
      centralidade: 16,
      regras: 2,
      funcionalidades: 1,
      documentadas: 0,
      lacuna: false,
      calor: 0,
    },
  ],
  sugestao: {
    tipo: "area",
    alvo: "cancelamento",
    nome: "Cancelamento",
    fluxo: "explicar",
    funcionalidade: "F-CAN-001",
    entrada: 'Explicar a funcionalidade "Cancelar fatura" (F-CAN-001)',
    motivo: "A área recebe 4 referências e a base não tem nenhum documento dela.",
  },
  totais: { regras: 2, termos: 1, documentos: 1, aRevalidar: 1 },
};

describe("CoberturaProjeto", () => {
  it("mostra a área sem documentação como lacuna e a sugere", async () => {
    const api = { cobertura: vi.fn(() => Promise.resolve(COBERTURA)) };
    const navegar = vi.fn();
    render(<CoberturaProjeto api={api} projeto="fat" navegar={navegar} />);

    const mapa = await screen.findByRole("list", { name: "Mapa de calor por área" });
    const [primeira, segunda] = within(mapa).getAllByRole("listitem");
    expect(within(primeira).getByText("Cancelamento")).toBeInTheDocument();
    expect(within(primeira).getByText("lacuna")).toBeInTheDocument();
    expect(primeira).toHaveTextContent("0 regras · 0 termos · 0 documentos");
    expect(segunda).toHaveTextContent("2 regras · 1 termo · 1 documento");
    expect(segunda).toHaveTextContent("1 a revalidar");
    expect(segunda).toHaveTextContent("1 regra sem confirmação recente");
    expect(within(segunda).queryByText("lacuna")).toBeNull();

    const sugestao = screen.getByRole("region", { name: "Próxima análise sugerida" });
    expect(sugestao).toHaveTextContent("Explicar a funcionalidade");
    fireEvent.click(within(sugestao).getByRole("button"));
    expect(navegar).toHaveBeenCalledWith({ tipo: "mapa", projeto: "fat" });

    expect(screen.getByText("2 regras · 1 termo · 1 documento negocial · 1 a revalidar")).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Pastas por lacuna" })).toHaveTextContent("db");
  });

  it("sugestão de pasta abre uma pergunta", async () => {
    const api = {
      cobertura: vi.fn(() =>
        Promise.resolve({
          ...COBERTURA,
          sugestao: {
            ...COBERTURA.sugestao!,
            tipo: "pasta" as const,
            alvo: "db",
            funcionalidade: null,
            fluxo: "perguntar" as const,
            entrada: "O que o código da pasta db faz?",
          },
        }),
      ),
    };
    const navegar = vi.fn();
    render(<CoberturaProjeto api={api} projeto="fat" navegar={navegar} />);
    fireEvent.click(await screen.findByRole("button", { name: "Perguntar ao sistema" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: "fat",
      pergunta: "O que o código da pasta db faz?",
    });
  });

  it("mostra a falha do cálculo", async () => {
    const api = { cobertura: vi.fn(() => Promise.reject(new Error("índice indisponível"))) };
    render(<CoberturaProjeto api={api} projeto="fat" navegar={vi.fn()} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("índice indisponível");
  });
});
