import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { NovaPrototipo, pedidoDaEntrada } from "./NovaPrototipo";
import { PainelAnalise } from "./PainelAnalise";
import { PainelDemanda } from "./PainelDemanda";
import { TelaAnalise } from "./TelaAnalise";
import type { AnaliseResumo } from "./analise";
import { PROJETO, apiConhecimentoFalsa } from "./test/conhecimentoFalso";
import {
  apiAnaliseFalsa,
  apiConversaFalsa,
  apiTerminalFalsa,
  telaFalsa,
  vistaFicticia,
} from "./test/falsos";
import { apiPrototipoFalsa } from "./test/prototipoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

const CRIADA = {
  analise: expect.objectContaining({ id: "A-0009", fluxo: "prototipo" }),
  promptInicial: "Inicie a análise A-0009 pelo ctower",
};

/** Origens fictícias do "Faturamento Exemplo": aprovadas, não aprovadas e uma que não serve. */
function analises(): AnaliseResumo[] {
  return [
    { id: "A-0008", titulo: "Cenários do parcelamento", fluxo: "cenarios", estado: "concluida", atualizadaMs: 8 },
    { id: "A-0007", titulo: "Histórias do parcelamento", fluxo: "historias", estado: "concluida", atualizadaMs: 7, demanda: "D-0001" },
    { id: "A-0003", titulo: "Parcelamento para pessoa jurídica", fluxo: "impacto", estado: "aprovada", atualizadaMs: 3 },
    { id: "A-0001", titulo: "Multa por atraso", fluxo: "explicar", estado: "aprovada", atualizadaMs: 1 },
  ];
}

describe("NovaPrototipo: o pedido do protótipo de telas (#379)", () => {
  it("pedidoDaEntrada: uma tela por linha, sem repetidas, e o que fica de fora aparado", () => {
    expect(
      pedidoDaEntrada({
        telas: "Emissão da fatura\n\n emissão da fatura ;Parcelamento",
        fidelidade: "alta_fidelidade",
        fora: "  ",
      }),
    ).toEqual({
      telas: ["Emissão da fatura", "Parcelamento"],
      fidelidade: "alta_fidelidade",
      fora: undefined,
    });
  });

  it("Nova análise: origem obrigatória, aprovadas em destaque e o pedido com telas, fidelidade e o que fica de fora", async () => {
    const conhecimento = apiConhecimentoFalsa();
    conhecimento.api.listarAnalises.mockResolvedValue(analises() as never);
    const aoAbrirAnalise = vi.fn();
    render(
      <TelaAnalise
        api={apiAnaliseFalsa().api}
        apiConhecimento={conhecimento.api}
        projetoInicial={PROJETO}
        aoAbrirAnalise={aoAbrirAnalise}
        novaInicial
      />,
    );
    const painel = await screen.findByRole("dialog", { name: "Nova análise" });
    fireEvent.click(within(painel).getByRole("button", { name: /Protótipo de telas/ }));
    const form = within(painel).getByRole("form", { name: "Protótipo de telas" });

    const origem = within(form).getByLabelText("Análise de origem") as HTMLSelectElement;
    await waitFor(() => expect(origem.options.length).toBe(3));
    // Cenários não servem de origem; as aprovadas vêm primeiro, num grupo à parte.
    const grupos = within(origem).getAllByRole("group");
    expect(grupos.map((g) => g.getAttribute("label"))).toEqual(["Aprovadas", "Ainda não aprovadas"]);
    expect(within(grupos[0]).getAllByRole("option").map((o) => o.textContent)).toEqual([
      "A-0003: Parcelamento para pessoa jurídica (aprovada)",
      "A-0001: Multa por atraso (aprovada)",
    ]);
    expect(within(origem).queryByRole("option", { name: /A-0008/ })).toBeNull();
    expect(origem.value).toBe("A-0003");
    expect(within(form).queryByText(/ainda não foi aprovada/)).toBeNull();

    // A origem não aprovada só gera aviso.
    fireEvent.change(origem, { target: { value: "A-0007" } });
    expect(within(form).getByText(/A análise A-0007 ainda não foi aprovada/)).toBeInTheDocument();
    fireEvent.change(origem, { target: { value: "A-0003" } });

    fireEvent.change(within(form).getByLabelText("Telas"), {
      target: { value: "Emissão da fatura\nParcelamento da fatura" },
    });
    const fidelidade = within(form).getByRole("radiogroup", { name: "Fidelidade" });
    expect(within(fidelidade).getByRole("radio", { name: "Wireframe" })).toBeChecked();
    fireEvent.click(within(fidelidade).getByRole("radio", { name: "Alta fidelidade" }));
    fireEvent.change(within(form).getByLabelText("O que fica de fora"), {
      target: { value: "Relatório de recebíveis" },
    });
    // A identidade visual está no pedido, sem bloqueá-lo.
    expect(within(form).getByRole("region", { name: "Identidade visual" })).toBeInTheDocument();

    const botao = within(form).getByRole("button", { name: "Prototipar as telas" });
    await waitFor(() => expect(botao).toBeEnabled());
    fireEvent.click(botao);
    await waitFor(() =>
      expect(conhecimento.api.prototipo.criarAnaliseDePrototipo).toHaveBeenCalledWith(PROJETO, {
        origem: "A-0003",
        demanda: undefined,
        pedido: {
          telas: ["Emissão da fatura", "Parcelamento da fatura"],
          fidelidade: "alta_fidelidade",
          fora: "Relatório de recebíveis",
        },
      }),
    );
    await waitFor(() =>
      expect(aoAbrirAnalise).toHaveBeenCalledWith(PROJETO, "A-0009", {
        tipo: "nova",
        promptInicial: "Inicie a análise A-0009 pelo ctower",
      }),
    );
  });

  it("sem análise que sirva de origem, não deixa pedir", async () => {
    const conhecimento = apiConhecimentoFalsa();
    conhecimento.api.listarAnalises.mockResolvedValue([analises()[0]] as never);
    render(
      <NovaPrototipo
        api={conhecimento.api}
        apiPrototipo={conhecimento.api.prototipo}
        projetos={[{ id: PROJETO, nome: "Faturamento Exemplo" }]}
        aoCriar={vi.fn()}
        aoCancelar={vi.fn()}
      />,
    );
    expect(await screen.findByText(/Faça antes a análise de impacto/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Prototipar as telas" })).toBeDisabled();
  });

  it("painel da demanda: as análises da demanda primeiro e o protótipo ligado a ela", async () => {
    const conhecimento = apiConhecimentoFalsa();
    conhecimento.api.listarAnalises.mockResolvedValue(analises() as never);
    const navegar = vi.fn();
    render(
      <PainelDemanda api={conhecimento.api} projeto={PROJETO} demanda="D-0001" navegar={navegar} />,
    );
    const secao = await screen.findByRole("region", { name: "Nova análise da demanda" });
    fireEvent.click(within(secao).getByRole("button", { name: "Protótipo de telas" }));
    const form = within(secao).getByRole("form", { name: "Protótipo de telas" });
    expect(within(form).getByText("O protótipo fica ligado à demanda D-0001.")).toBeInTheDocument();
    const origem = within(form).getByLabelText("Análise de origem") as HTMLSelectElement;
    // A-0007 é da D-0001: vem escolhida, mesmo sem aprovação (com o aviso).
    await waitFor(() => expect(origem.value).toBe("A-0007"));
    expect(within(form).getByText(/A análise A-0007 ainda não foi aprovada/)).toBeInTheDocument();
    const botao = within(form).getByRole("button", { name: "Prototipar as telas" });
    await waitFor(() => expect(botao).toBeEnabled());
    fireEvent.click(botao);
    await waitFor(() =>
      expect(conhecimento.api.prototipo.criarAnaliseDePrototipo).toHaveBeenCalledWith(PROJETO, {
        origem: "A-0007",
        demanda: "D-0001",
        pedido: { telas: [], fidelidade: "wireframe", fora: undefined },
      }),
    );
    await waitFor(() =>
      expect(navegar).toHaveBeenCalledWith({
        tipo: "analise",
        projeto: PROJETO,
        aberta: {
          id: "A-0009",
          abertura: { tipo: "nova", promptInicial: "Inicie a análise A-0009 pelo ctower" },
        },
      }),
    );
  });

  describe("na análise: Prototipar telas", () => {
    function montar(estado: "aprovada" | "concluida", fluxo: AnaliseResumo["fluxo"] = "impacto") {
      const vista = vistaFicticia({}, { estado, fluxo, titulo: "Parcelamento para pessoa jurídica" });
      const analise = apiAnaliseFalsa(vista);
      const prototipo = apiPrototipoFalsa();
      const aoAbrirAnalise = vi.fn();
      render(
        <PainelAnalise
          api={analise.api}
          projeto={PROJETO}
          analise="A-0001"
          abertura={{ tipo: "reabrir" }}
          aoVoltar={vi.fn()}
          apiTerminal={apiTerminalFalsa().api}
          apiConversa={apiConversaFalsa()}
          criarTela={telaFalsa}
          apiPrototipo={prototipo.api}
          aoAbrirAnalise={aoAbrirAnalise}
          apiPublicacao={{
            previa: () => new Promise(() => {}),
            publicar: () => new Promise(() => {}),
          }}
        />,
      );
      return { analise, prototipo, aoAbrirAnalise };
    }

    it("a análise aprovada pede o protótipo com ela de origem e abre a nova", async () => {
      const { prototipo, aoAbrirAnalise, analise } = montar("aprovada");
      fireEvent.click(await screen.findByRole("button", { name: "Prototipar telas" }));
      const dialogo = screen.getByRole("dialog", { name: "Prototipar telas" });
      expect(
        within(dialogo).getByText("A-0001: Parcelamento para pessoa jurídica"),
      ).toBeInTheDocument();
      // A origem já é esta análise: nada de escolher outra.
      expect(within(dialogo).queryByLabelText("Análise de origem")).toBeNull();
      expect(analise.api.listarAnalises).not.toHaveBeenCalled();
      fireEvent.click(within(dialogo).getByRole("radio", { name: "Alta fidelidade" }));
      const botao = within(dialogo).getByRole("button", { name: "Prototipar as telas" });
      await waitFor(() => expect(botao).toBeEnabled());
      fireEvent.click(botao);
      await waitFor(() =>
        expect(prototipo.api.criarAnaliseDePrototipo).toHaveBeenCalledWith(PROJETO, {
          origem: "A-0001",
          demanda: undefined,
          pedido: { telas: [], fidelidade: "alta_fidelidade", fora: undefined },
        }),
      );
      await waitFor(() =>
        expect(aoAbrirAnalise).toHaveBeenCalledWith("A-0009", {
          tipo: "nova",
          promptInicial: CRIADA.promptInicial,
        }),
      );
      expect(screen.queryByRole("dialog", { name: "Prototipar telas" })).toBeNull();
    });

    it("sem aprovação, ou num fluxo que não é origem, não oferece a ação", async () => {
      montar("concluida");
      expect(await screen.findByRole("button", { name: "Exportar" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Prototipar telas" })).toBeNull();
    });

    it("um protótipo aprovado não vira origem de outro", async () => {
      montar("aprovada", "prototipo");
      expect(await screen.findByRole("button", { name: "Exportar" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Prototipar telas" })).toBeNull();
    });
  });
});
