import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Historia } from "./analise";
import { HistoriasAoVivo, alertasInvest } from "./HistoriasAoVivo";
import { NovaHistorias } from "./NovaHistorias";
import type { Revisao } from "./AcoesRevisao";
import { apiConhecimentoFalsa } from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

/** Histórias fictícias do "Faturamento Exemplo" (parcelamento para pessoa física). */
function historias(): Historia[] {
  return [
    {
      id: "HU-0001",
      titulo: "Parcelar fatura de pessoa física em até 3 vezes",
      como: "atendente do faturamento",
      quero: "parcelar a fatura em aberto de um cliente pessoa física",
      para: "reduzir a inadimplência",
      situacao: "pendente",
      registrada_ms: 1,
      criterios: [
        {
          numero: 1,
          dado: "uma pessoa física com uma fatura em aberto de R$ 300,00",
          quando: "ela pede o parcelamento em 3 vezes",
          entao: "são geradas 3 parcelas de R$ 100,00",
          fundamentos: [
            { tipo: "afirmacao", ref: "A-0001/4", texto: "Pessoa física não parcela hoje." },
            { tipo: "regra", id: "RN-FAT-008", titulo: "Quantidade de parcelas" },
          ],
          situacao: "pendente",
          registrado_ms: 1,
        },
        {
          numero: 2,
          dado: "uma fatura de R$ 250,00",
          quando: "ela pede 3 parcelas",
          entao: "o pedido é recusado",
          fundamentos: [{ tipo: "regra", id: "RN-FAT-009", titulo: "Valor mínimo" }],
          pergunta: "A-0001/2",
          situacao: "pendente",
          registrado_ms: 2,
        },
      ],
    },
    {
      id: "HU-0002",
      titulo: "Mostrar as parcelas no relatório de recebíveis",
      como: "gestor financeiro",
      quero: "ver as parcelas no relatório",
      para: "prever o caixa",
      situacao: "pendente",
      registrada_ms: 3,
      invest: [{ criterio: "pequena", motivo: "junta tela e relatório" }],
      quebra: "Separar o relatório do filtro.",
      criterios: [],
    },
  ];
}

function revisaoFalsa(): Revisao & { aoDecidir: ReturnType<typeof vi.fn> } {
  return { aoDecidir: vi.fn(() => Promise.resolve()), ocupado: false };
}

describe("HistoriasAoVivo", () => {
  it("mostra a narrativa, o INVEST, os critérios Gherkin e as origens", () => {
    render(<HistoriasAoVivo historias={historias()} origem="A-0001" revisao={revisaoFalsa()} />);
    expect(screen.getByText(/2 história\(s\) · 2 critério\(s\) · 1 a confirmar · origem A-0001/));
    const hu1 = screen.getByRole("region", { name: /HU-0001/ });
    expect(hu1).toHaveTextContent("Como atendente do faturamento, quero parcelar");
    expect(within(hu1).getByText("RN-FAT-008")).toHaveAttribute("title", "Quantidade de parcelas");
    expect(within(hu1).getByText("af. A-0001/4")).toBeInTheDocument();
    expect(within(hu1).getByText("a confirmar: pergunta A-0001/2")).toBeInTheDocument();
    const invest = within(hu1).getByLabelText("Verificação INVEST");
    expect(within(invest).getByLabelText("testável: atende")).toBeInTheDocument();

    const hu2 = screen.getByRole("region", { name: /HU-0002/ });
    expect(within(hu2).getByLabelText("pequena: não atende")).toBeInTheDocument();
    // Sem critério, não é testável.
    expect(within(hu2).getByLabelText("testável: não atende")).toBeInTheDocument();
    expect(hu2).toHaveTextContent("Quebra proposta: Separar o relatório do filtro.");
  });

  it("aceita, quebra, edita e descarta histórias e critérios", async () => {
    const revisao = revisaoFalsa();
    render(<HistoriasAoVivo historias={historias()} revisao={revisao} />);
    const hu1 = screen.getByRole("group", { name: "Revisão de HU-0001" });
    fireEvent.click(within(hu1).getByRole("button", { name: "Aceitar" }));
    expect(revisao.aoDecidir).toHaveBeenCalledWith({
      tipo: "historia",
      historia: "HU-0001",
      decisao: "aceitar",
    });

    const ca2 = screen.getByRole("group", { name: "Revisão do CA-2" });
    fireEvent.click(within(ca2).getByRole("button", { name: "Descartar" }));
    expect(revisao.aoDecidir).toHaveBeenCalledWith({
      tipo: "criterio",
      historia: "HU-0001",
      numero: 2,
      situacao: "descartado",
    });

    fireEvent.click(
      within(screen.getByRole("group", { name: "Revisão do CA-1" })).getByRole("button", {
        name: "Editar",
      }),
    );
    const form = screen.getByRole("form", { name: "Editar CA-1" });
    fireEvent.change(within(form).getByLabelText("Então"), {
      target: { value: "são geradas 3 parcelas de R$ 100,00 a cada 30 dias" },
    });
    fireEvent.click(within(form).getByRole("button", { name: "Salvar" }));
    await waitFor(() =>
      expect(revisao.aoDecidir).toHaveBeenCalledWith({
        tipo: "editar-criterio",
        historia: "HU-0001",
        numero: 1,
        dado: "uma pessoa física com uma fatura em aberto de R$ 300,00",
        quando: "ela pede o parcelamento em 3 vezes",
        entao: "são geradas 3 parcelas de R$ 100,00 a cada 30 dias",
      }),
    );

    const hu2 = screen.getByRole("group", { name: "Revisão de HU-0002" });
    fireEvent.click(within(hu2).getByRole("button", { name: "Quebrar" }));
    const quebra = screen.getByRole("form", { name: "Quebrar HU-0002" });
    fireEvent.change(within(quebra).getByLabelText("Como quebrar (opcional)"), {
      target: { value: "relatório primeiro" },
    });
    fireEvent.click(within(quebra).getByRole("button", { name: "Pedir a quebra" }));
    expect(revisao.aoDecidir).toHaveBeenCalledWith({
      tipo: "historia",
      historia: "HU-0002",
      decisao: "quebrar",
      pedido: "relatório primeiro",
    });
  });

  it("sem revisão (aprovada), só leitura", () => {
    render(<HistoriasAoVivo historias={historias()} />);
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("o INVEST testável vem dos critérios", () => {
    const [hu1] = historias();
    expect(alertasInvest(hu1).has("testavel")).toBe(false);
    const soAConfirmar = { ...hu1, criterios: [hu1.criterios[1]] };
    expect(alertasInvest(soAConfirmar).get("testavel")).toMatch(/pergunta em aberto/);
  });
});

describe("NovaHistorias", () => {
  it("lista as análises do código como origem e cria a análise de histórias", async () => {
    const { api } = apiConhecimentoFalsa();
    const aoCriar = vi.fn();
    render(
      <NovaHistorias
        api={api}
        projetos={[{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }]}
        aoCriar={aoCriar}
        aoCancelar={vi.fn()}
      />,
    );
    const origem = await screen.findByLabelText("Análise de origem");
    await waitFor(() => expect(origem).toHaveValue("A-0001"));
    // A própria análise de histórias não é origem.
    expect(within(origem).queryByText(/A-0002/)).toBeNull();
    fireEvent.change(screen.getByLabelText("O que você quer nas histórias (opcional)"), {
      target: { value: "Só a tela." },
    });
    const botao = screen.getByRole("button", { name: "Gerar as histórias" });
    await waitFor(() => expect(botao).toBeEnabled());
    fireEvent.click(botao);
    await waitFor(() => expect(aoCriar).toHaveBeenCalled());
    expect(api.criarAnaliseDeHistorias).toHaveBeenCalledWith("faturamento-exemplo", {
      origem: "A-0001",
      demanda: undefined,
      entrada: "Só a tela.",
    });
  });
});
