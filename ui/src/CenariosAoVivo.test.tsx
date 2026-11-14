import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Cenario, TabelaDecisao } from "./analise";
import { CenariosAoVivo } from "./CenariosAoVivo";
import { NovaHistorias } from "./NovaHistorias";
import type { Revisao } from "./AcoesRevisao";
import { apiConhecimentoFalsa } from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

const RN014 = { tipo: "regra" as const, id: "RN-FAT-014", titulo: "Supervisor após 30 dias" };

/** Tabela de decisão fictícia do "Faturamento Exemplo" (cancelamento após 30 dias). */
function tabela(): TabelaDecisao {
  return {
    numero: 1,
    titulo: "Cancelamento de fatura emitida há mais de 30 dias",
    condicoes: [
      {
        texto: "fatura emitida há mais de 30 dias",
        evidencias: [{ arquivo: "delphi/UFrmCancelamento.pas", linhas: "70-70" }],
      },
      { texto: "usuário com perfil SUPERVISOR" },
    ],
    acoes: ["segue para o cancelamento", "recusa: 'Solicite a um supervisor'"],
    colunas: [
      { numero: 1, valores: ["S", "S"], acoes: [1] },
      { numero: 2, valores: ["S", "N"], acoes: [2] },
      { numero: 3, valores: ["N", "-"], impossivel: "exemplo de justificativa" },
    ],
    fundamentos: [RN014],
    situacao: "pendente",
    registrada_ms: 1,
  };
}

function cenarios(): Cenario[] {
  return [
    {
      id: "CT-0001",
      tecnica: "valor_limite",
      titulo: "31 dias sem supervisor é recusado",
      dado: "uma fatura ABERTA emitida há 31 dias",
      quando: "um atendente pede o cancelamento",
      entao: "o cancelamento é recusado",
      valor: "31 dias",
      limite: {
        grandeza: "dias desde a emissão",
        valor: "30",
        evidencias: [{ arquivo: "delphi/UFrmCancelamento.pas", linhas: "41-41" }],
      },
      coluna: { tabela: 1, coluna: 2 },
      historia: "HU-0001",
      fundamentos: [RN014],
      situacao: "pendente",
      registrado_ms: 2,
    },
    {
      id: "CT-0002",
      tecnica: "valor_limite",
      titulo: "7 parcelas são recusadas",
      dado: "uma fatura ABERTA de pessoa jurídica",
      quando: "o cliente pede 7 parcelas",
      entao: "o pedido é recusado",
      valor: "7 parcelas",
      limite: {
        grandeza: "quantidade de parcelas",
        valor: "6",
        evidencias: [{ arquivo: "backend/ParcelamentoService.java", linhas: "34-34" }],
        configuracao: "PARAMETRO.MAX_PARCELAS",
      },
      fundamentos: [{ tipo: "regra", id: "RN-FAT-008", titulo: "Quantidade de parcelas" }],
      pergunta: "A-0001/1",
      situacao: "pendente",
      registrado_ms: 3,
    },
  ];
}

function revisao(): Revisao & { aoDecidir: ReturnType<typeof vi.fn> } {
  return { aoDecidir: vi.fn(() => Promise.resolve()), ocupado: false };
}

describe("CenariosAoVivo", () => {
  it("mostra a tabela de decisão, os limites e os cenários", () => {
    render(<CenariosAoVivo tabelas={[tabela()]} cenarios={cenarios()} origem="A-0001" />);
    expect(screen.getByText(/1 tabela\(s\) de decisão · 2 cenário\(s\) · 1 a confirmar/)).toBeTruthy();
    const td = screen.getByRole("table", { name: "Tabela de decisão TD-1" });
    expect(within(td).getByText("R3")).toBeTruthy();
    expect(within(td).getByText("CT-0001")).toBeTruthy();
    expect(within(td).getByText("impossível")).toBeTruthy();
    expect(within(td).getByText("delphi/UFrmCancelamento.pas:70")).toBeTruthy();
    expect(screen.getByText(/R3/, { selector: "li strong" })).toBeTruthy();
    const limites = screen.getByRole("region", { name: "Valores-limite" });
    expect(within(limites).getByText("30")).toBeTruthy();
    expect(within(limites).getByText("a confirmar")).toBeTruthy();
    expect(screen.getByText("a confirmar: pergunta A-0001/1")).toBeTruthy();
    expect(screen.getByText("TD-1/R2")).toBeTruthy();
    // Sem revisão (aprovada), só leitura.
    expect(screen.queryByRole("button", { name: "Aceitar" })).toBeNull();
  });

  it("decide e edita tabelas e cenários", async () => {
    const r = revisao();
    render(<CenariosAoVivo tabelas={[tabela()]} cenarios={cenarios()} revisao={r} />);
    fireEvent.click(
      within(screen.getByRole("group", { name: "Revisão de TD-1" })).getByRole("button", {
        name: "Aceitar",
      }),
    );
    expect(r.aoDecidir).toHaveBeenCalledWith({ tipo: "tabela", numero: 1, situacao: "aceito" });
    fireEvent.click(
      within(screen.getByRole("group", { name: "Revisão de CT-0002" })).getByRole("button", {
        name: "Descartar",
      }),
    );
    expect(r.aoDecidir).toHaveBeenCalledWith({
      tipo: "cenario",
      cenario: "CT-0002",
      situacao: "descartado",
    });
    fireEvent.click(
      within(screen.getByRole("group", { name: "Revisão de CT-0001" })).getByRole("button", {
        name: "Editar",
      }),
    );
    const form = screen.getByRole("form", { name: "Editar CT-0001" });
    fireEvent.change(within(form).getByLabelText("Título"), {
      target: { value: "31 dias recusado" },
    });
    fireEvent.click(within(form).getByRole("button", { name: "Salvar" }));
    await waitFor(() =>
      expect(r.aoDecidir).toHaveBeenCalledWith(
        expect.objectContaining({ tipo: "editar-cenario", cenario: "CT-0001", titulo: "31 dias recusado" }),
      ),
    );
  });
});

describe("NovaHistorias no fluxo cenarios", () => {
  it("aceita a análise de histórias como origem e cria a análise de cenários", async () => {
    const { api } = apiConhecimentoFalsa();
    const aoCriar = vi.fn();
    render(
      <NovaHistorias
        api={api}
        fluxo="cenarios"
        projetos={[{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }]}
        aoCriar={aoCriar}
        aoCancelar={() => undefined}
      />,
    );
    const origem = await screen.findByLabelText("Análise de origem");
    await waitFor(() => expect((origem as HTMLSelectElement).options.length).toBe(3));
    // A de histórias também serve de origem; e dá para partir só das regras.
    expect(screen.getByRole("option", { name: /A-0002: Histórias · Parcelamento/ })).toBeTruthy();
    expect(screen.getByRole("option", { name: /só as regras do pedido/ })).toBeTruthy();
    fireEvent.change(origem, { target: { value: "A-0002" } });
    const botao = screen.getByRole("button", { name: "Gerar os cenários" });
    await waitFor(() => expect((botao as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(botao);
    await waitFor(() =>
      expect(api.criarAnaliseDeCenarios).toHaveBeenCalledWith("faturamento-exemplo", {
        origem: "A-0002",
        demanda: undefined,
        entrada: undefined,
      }),
    );
    expect(aoCriar).toHaveBeenCalled();
    expect(api.criarAnaliseDeHistorias).not.toHaveBeenCalled();
  });
});
