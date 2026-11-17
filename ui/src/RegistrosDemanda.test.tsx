import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { RegistrosDemanda } from "./RegistrosDemanda";
import type { ApiRegistros, RegistroVista, RegistrosDaDemanda } from "./registros";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const PROJETO = "faturamento-exemplo";
const PASTA = "faturamento-exemplo/demandas/D-0001-parcelamento-pj/registros";

function registro(r: Partial<RegistroVista> & Pick<RegistroVista, "id" | "categoria" | "titulo">) {
  return {
    detalhe: null,
    quem: null,
    pessoa: null,
    origem: null,
    referencia: null,
    criado: "2026-11-10",
    status: null,
    validacao: null,
    decididoEm: null,
    alternativas: [],
    probabilidade: null,
    impacto: null,
    resposta: null,
    restricao: null,
    historico: [],
    documento: `${PASTA}/${r.id}.md`,
    ...r,
  } satisfies RegistroVista;
}

function abaFicticia(): RegistrosDaDemanda {
  return {
    pasta: PASTA,
    pessoas: [{ slug: "maria-silva", nome: "Maria Silva", papel: "PO" }],
    registros: [
      registro({
        id: "PR-0001",
        categoria: "premissa",
        titulo: "Clientes PJ já têm limite de crédito cadastrado.",
        status: "nao-validada",
        quem: "Ana Exemplo",
        referencia: "A-0001/1",
        origem: "faturamento-exemplo/analises/A-0001.md",
      }),
      registro({
        id: "DE-0001",
        categoria: "decisao",
        titulo: "Máximo de 3 parcelas.",
        quem: "Maria Silva",
        pessoa: "maria-silva",
        decididoEm: "2026-11-10",
        referencia: "R-0001/D-1",
        alternativas: ["6 parcelas", "sem limite"],
      }),
      registro({
        id: "RI-0001",
        categoria: "risco",
        titulo: "Integração bancária rejeita remessa com parcelas.",
        probabilidade: "alto",
        impacto: "alto",
      }),
      registro({
        id: "RE-0001",
        categoria: "restricao",
        titulo: "Sem alteração no layout CNAB.",
        restricao: "tecnica",
      }),
    ],
    propostas: [
      {
        referencia: "A-0001/4",
        categoria: "risco",
        fonte: { tipo: "analise", id: "A-0001", titulo: "Parcelamento PJ" },
        dados: { titulo: "Faturas antigas sem coluna de parcela.", probabilidade: "medio", impacto: "medio" },
      },
      {
        referencia: "R-0001/D-2",
        categoria: "decisao",
        fonte: { tipo: "ata", id: "R-0001", data: "2026-11-10" },
        dados: { titulo: "Relatório de recebíveis fica para a fase 2.", quem: "Maria Silva", pessoa: "maria-silva" },
      },
    ],
  };
}

function apiFalsa(aba = abaFicticia()) {
  const api: ApiRegistros = {
    registros: vi.fn(async () => aba),
    criar: vi.fn(async (_p, _d, categoria, campos) =>
      registro({ id: "PR-0002", categoria, titulo: campos.titulo }),
    ),
    editar: vi.fn(async (_p, id, campos) => registro({ id, categoria: "premissa", titulo: campos.titulo })),
    validarPremissa: vi.fn(async (_p, id) => registro({ id, categoria: "premissa", titulo: "x" })),
    aceitarProposta: vi.fn(async () => registro({ id: "RI-0002", categoria: "risco", titulo: "x" })),
    recusarProposta: vi.fn(async () => undefined),
  };
  return api;
}

describe("RegistrosDemanda", () => {
  it("mostra as quatro seções com destaques, a matriz e a faixa das propostas", async () => {
    const api = apiFalsa();
    const abrir = vi.fn();
    render(<RegistrosDemanda projeto={PROJETO} demanda="D-0001" api={api} abrirDocumento={abrir} />);
    const premissas = await screen.findByRole("region", { name: "Premissas" });
    expect(api.registros).toHaveBeenCalledWith(PROJETO, "D-0001");
    expect(within(premissas).getByText("1 não validada")).toBeInTheDocument();
    const pr = premissas.querySelector('[data-registro="PR-0001"]')!;
    expect(pr).toHaveAttribute("data-destaque", "sim");
    expect(pr).toHaveTextContent("assumida por Ana Exemplo");
    expect(pr).toHaveTextContent("análise A-0001");

    const decisoes = screen.getByRole("region", { name: "Decisões" });
    expect(decisoes).toHaveTextContent("decidiu Maria Silva");
    expect(decisoes).toHaveTextContent("ata R-0001, item D-1");
    expect(decisoes).toHaveTextContent("alternativas: 6 parcelas, sem limite");

    const riscos = screen.getByRole("region", { name: "Riscos" });
    expect(within(riscos).getByText("1 alto")).toBeInTheDocument();
    const matriz = within(riscos).getByRole("table", { name: /probabilidade × impacto/ });
    expect(matriz).toHaveTextContent("RI-0001");

    expect(screen.getByRole("region", { name: "Restrições" })).toHaveTextContent("técnica");

    const faixa = screen.getByRole("status");
    expect(faixa).toHaveTextContent("2 propostas do agente");
    expect(faixa).toHaveTextContent("De A-0001 (impacto): 1 risco.");
    expect(faixa).toHaveTextContent("De ata R-0001 (reunião de 2026-11-10): 1 decisão.");
    expect(screen.getByText(new RegExp(PASTA))).toBeInTheDocument();

    fireEvent.click(within(pr as HTMLElement).getByRole("button", { name: "abrir a origem" }));
    expect(abrir).toHaveBeenCalledWith("faturamento-exemplo/analises/A-0001.md");
  });

  it("aceita, ajusta e recusa propostas", async () => {
    const api = apiFalsa();
    render(<RegistrosDemanda projeto={PROJETO} demanda="D-0001" api={api} />);
    fireEvent.click(await screen.findByRole("button", { name: "Aceitar R-0001/D-2" }));
    await waitFor(() =>
      expect(api.aceitarProposta).toHaveBeenCalledWith(PROJETO, "D-0001", "R-0001/D-2", undefined),
    );
    fireEvent.click(screen.getByRole("button", { name: "Recusar R-0001/D-2" }));
    await waitFor(() =>
      expect(api.recusarProposta).toHaveBeenCalledWith(PROJETO, "D-0001", "R-0001/D-2"),
    );

    fireEvent.click(screen.getByRole("button", { name: "Ajustar A-0001/4" }));
    const form = screen.getByRole("form", { name: "Ajustar a proposta A-0001/4" });
    fireEvent.change(within(form).getByLabelText("Probabilidade"), { target: { value: "alto" } });
    fireEvent.click(within(form).getByRole("button", { name: "Aceitar" }));
    await waitFor(() =>
      expect(api.aceitarProposta).toHaveBeenLastCalledWith(
        PROJETO,
        "D-0001",
        "A-0001/4",
        expect.objectContaining({ probabilidade: "alto", impacto: "medio" }),
      ),
    );
  });

  it("cria à mão e valida uma premissa dizendo como", async () => {
    const api = apiFalsa();
    render(<RegistrosDemanda projeto={PROJETO} demanda="D-0001" api={api} />);
    fireEvent.click(await screen.findByRole("button", { name: "Nova premissa" }));
    const form = screen.getByRole("form", { name: "Nova premissa" });
    fireEvent.change(within(form).getByLabelText("Enunciado da premissa"), {
      target: { value: "O banco aceita vencimentos distintos." },
    });
    fireEvent.change(within(form).getByLabelText(/Quem assumiu \(catálogo/), {
      target: { value: "maria-silva" },
    });
    fireEvent.click(within(form).getByRole("button", { name: "Salvar" }));
    await waitFor(() =>
      expect(api.criar).toHaveBeenCalledWith(
        PROJETO,
        "D-0001",
        "premissa",
        expect.objectContaining({
          titulo: "O banco aceita vencimentos distintos.",
          pessoa: "maria-silva",
          quem: "Maria Silva",
          status: "nao-validada",
        }),
      ),
    );

    const premissas = screen.getByRole("region", { name: "Premissas" });
    fireEvent.click(within(premissas).getByRole("button", { name: "Validar" }));
    const validar = screen.getByRole("form", { name: "Validar PR-0001" });
    fireEvent.change(within(validar).getByLabelText("Como foi validada"), {
      target: { value: "Confirmado pelo financeiro" },
    });
    fireEvent.click(within(validar).getByRole("button", { name: "Validar" }));
    await waitFor(() =>
      expect(api.validarPremissa).toHaveBeenCalledWith(PROJETO, "PR-0001", {
        status: "validada",
        como: "Confirmado pelo financeiro",
      }),
    );
  });
});
