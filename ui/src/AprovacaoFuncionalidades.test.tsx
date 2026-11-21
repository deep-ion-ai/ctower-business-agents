import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { AprovacaoAnalise } from "./AprovacaoAnalise";
import { resumoReligacao } from "./funcionalidades";
import type { ApiRevisao, AprovacaoFeita, Prontidao } from "./revisao";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(), open: vi.fn() }));

const AREAS = [
  { id: "cobranca", nome: "Cobrança", sigla: "COB" },
  { id: "cancelamento", nome: "Cancelamento", sigla: "CAN" },
];

const ENCARGOS = {
  id: "F-COB-001",
  nome: "Calcular encargos por atraso",
  area: "cobranca",
  areaNome: "Cobrança",
};
const CANCELAR = { id: "F-CAN-001", nome: "Cancelar fatura", area: "cancelamento", areaNome: "Cancelamento" };

/** Pergunta concluída e revisada, pronta para aprovar, com uma regra prevista. */
function pronta(extra: Partial<Prontidao> = {}): Prontidao {
  return {
    estado: "concluida",
    podeAprovar: true,
    pendencias: [],
    avisos: [],
    areas: AREAS,
    areaObrigatoria: true,
    regras: [{ afirmacao: 1, titulo: "A multa é calculada no pacote", texto: "A multa…" }],
    incorretas: [],
    termosAceitos: [],
    ...extra,
  };
}

function api(prontidao: Prontidao, feita: Partial<AprovacaoFeita> = {}) {
  return {
    revisar: vi.fn(),
    prontidao: vi.fn(() => Promise.resolve(prontidao)),
    aprovar: vi.fn(() =>
      Promise.resolve<AprovacaoFeita>({
        jaAprovada: false,
        regras: [],
        termos: [],
        avisos: [],
        ...feita,
      }),
    ),
    escolherDestino: vi.fn(),
    exportar: vi.fn(),
  } satisfies ApiRevisao;
}

function montar(a: ReturnType<typeof api>) {
  render(<AprovacaoAnalise api={a} projeto="faturamento-exemplo" analise="A-0002" versao={1} />);
}

describe("Aprovação com as funcionalidades do mapa (#287)", () => {
  it("propõe as sugeridas pelas evidências, o analista troca e a área acompanha", async () => {
    const a = api(
      pronta({
        funcionalidades: [ENCARGOS],
        origemDasFuncionalidades: "evidencias",
        funcionalidadesDoMapa: [CANCELAR, ENCARGOS],
        areaSugerida: "cobranca",
      }),
      { funcionalidades: ["F-CAN-001"] },
    );
    montar(a);
    const grupo = await screen.findByRole("group", { name: "Funcionalidades do mapa" });
    expect(grupo).toHaveTextContent(/Sugeridas pelas evidências/);
    const chips = within(grupo).getByRole("list", { name: "Funcionalidades vinculadas" });
    expect(chips).toHaveTextContent("F-COB-001 · Calcular encargos por atraso");
    // A área vem da funcionalidade, sem precisar escolher.
    expect(screen.getByLabelText("Área das regras")).toHaveValue("cobranca");

    fireEvent.click(within(grupo).getByRole("button", { name: "Tirar a funcionalidade F-COB-001" }));
    expect(within(grupo).queryByRole("list", { name: "Funcionalidades vinculadas" })).toBeNull();
    expect(screen.getByLabelText("Área das regras")).toHaveValue("");
    fireEvent.change(within(grupo).getByLabelText("Acrescentar"), { target: { value: "F-CAN-001" } });
    expect(within(grupo).getByRole("list", { name: "Funcionalidades vinculadas" })).toHaveTextContent(
      "F-CAN-001 · Cancelar fatura",
    );
    expect(screen.getByLabelText("Área das regras")).toHaveValue("cancelamento");

    fireEvent.click(screen.getByRole("button", { name: "Aprovar e gravar na base" }));
    await waitFor(() =>
      expect(a.aprovar).toHaveBeenCalledWith("faturamento-exemplo", "A-0002", {
        area: "cancelamento",
        afirmacoes: {},
        regras: [1],
        funcionalidades: ["F-CAN-001"],
      }),
    );
    const resultado = await screen.findByRole("status", { name: "Resultado da aprovação" });
    expect(resultado).toHaveTextContent("Funcionalidades do mapa: F-CAN-001");
  });

  it("a funcionalidade da sessão vem marcada e pode ser tirada", async () => {
    const a = api(
      pronta({
        areaObrigatoria: false,
        areas: [],
        funcionalidades: [CANCELAR],
        origemDasFuncionalidades: "sessao",
        funcionalidadesDoMapa: [CANCELAR, ENCARGOS],
        areaSugerida: "cancelamento",
      }),
    );
    montar(a);
    const grupo = await screen.findByRole("group", { name: "Funcionalidades do mapa" });
    expect(grupo).toHaveTextContent(/A funcionalidade desta análise/);
    fireEvent.click(within(grupo).getByRole("button", { name: "Tirar a funcionalidade F-CAN-001" }));
    fireEvent.click(screen.getByRole("button", { name: "Aprovar e gravar na base" }));
    await waitFor(() =>
      expect(a.aprovar).toHaveBeenCalledWith("faturamento-exemplo", "A-0002", {
        area: null,
        afirmacoes: {},
        regras: [1],
        funcionalidades: [],
      }),
    );
  });

  it("sem a proposta do app, a aprovação não manda funcionalidades", async () => {
    const a = api(pronta({ areaObrigatoria: false, areas: [] }));
    montar(a);
    await screen.findByLabelText("Área das regras");
    expect(screen.queryByRole("group", { name: "Funcionalidades do mapa" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Aprovar e gravar na base" }));
    await waitFor(() =>
      expect(a.aprovar).toHaveBeenCalledWith("faturamento-exemplo", "A-0002", {
        area: null,
        afirmacoes: {},
        regras: [1],
      }),
    );
  });
});

describe("resumoReligacao", () => {
  it("conta o que foi religado e as regras que ficaram sem funcionalidade", () => {
    expect(
      resumoReligacao({
        analises: ["analises/A-0001.md"],
        regras: ["regras/RN-CAN-001.md", "regras/RN-CAN-002.md"],
        termos: [],
        negociais: ["funcionalidades/cancelamento/cancelar-fatura/negocial.md"],
        regrasSemVinculo: 1,
        avisos: [],
      }),
    ).toBe(
      "Religados às funcionalidades: 1 análise, 2 regras, 1 documento negocial. " +
        "1 regra continua sem funcionalidade (a análise de origem não tem uma).",
    );
    expect(
      resumoReligacao({
        analises: [],
        regras: [],
        termos: [],
        negociais: [],
        regrasSemVinculo: 0,
        avisos: [],
      }),
    ).toBe("Nenhum documento a religar.");
  });
});
