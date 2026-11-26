import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Analise, VistaAnalise } from "./analise";
import { DocumentoAoVivo, semTituloRepetido } from "./DocumentoAoVivo";
import { PainelAnalise } from "./PainelAnalise";
import {
  apiAnaliseFalsa,
  apiConversaFalsa,
  apiTerminalFalsa,
  telaFalsa,
  vistaFicticia,
} from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

const ISSUE = "https://jira.example.com/browse/PROJ-123";

/** A entrada de uma análise que nasceu de uma issue: o título e a descrição em Markdown. */
const ENTRADA = `Multa por atraso no boleto

**Título:** Multa por atraso no boleto

**Descrição:** O cálculo da multa precisa considerar:

* juros de mora de 1% ao mês;
* multa fixa de 2%.

Detalhes em [outra issue](https://jira.example.com/browse/PROJ-9), no [site do banco](https://banco.example.com/regras) e na figura ![tela de cobrança](https://banco.example.com/tela.png).`;

function mostrar(analise: Partial<Analise>, vista: Partial<VistaAnalise> = {}, aoAbrirLink = vi.fn()) {
  render(
    <DocumentoAoVivo
      vista={vistaFicticia(vista, analise)}
      aoAbrirEvidencia={vi.fn()}
      aoAbrirLink={aoAbrirLink}
    />,
  );
  return aoAbrirLink;
}

describe("entrada da análise em Markdown (#403)", () => {
  it.each([
    ["perguntar", "Pergunta"],
    ["historias", "Pedido"],
    ["manual", "Funcionalidade"],
  ] as const)("no fluxo %s, a seção %s mostra negrito, listas e parágrafos sem os marcadores", (fluxo, rotulo) => {
    mostrar({ fluxo, entrada: ENTRADA });
    const secao = screen.getByRole("region", { name: rotulo });
    expect(within(secao).getByRole("heading", { level: 3, name: rotulo })).toBeInTheDocument();
    expect(within(secao).getByText("Descrição:").tagName).toBe("STRONG");
    const itens = within(secao).getAllByRole("listitem");
    expect(itens.map((i) => i.textContent)).toEqual(["juros de mora de 1% ao mês;", "multa fixa de 2%."]);
    expect(secao.querySelectorAll("p").length).toBeGreaterThanOrEqual(3);
    expect(secao.textContent).not.toMatch(/\*\*|^\* |\n\* |!\[|\]\(/);
  });

  it("vale também na revisão e no documento aprovado", () => {
    for (const estado of ["concluida", "aprovada"] as const) {
      const { unmount } = render(
        <DocumentoAoVivo
          vista={vistaFicticia({}, { estado, entrada: "**Prazo:** 30 dias\n\n- item um\n- item dois" })}
          aoAbrirEvidencia={vi.fn()}
          revisao={{
            ocupado: false,
            respostas: { disponivel: false, aoEnviar: vi.fn() },
            aoDecidir: () => Promise.resolve(),
          }}
        />,
      );
      const secao = screen.getByRole("region", { name: "Pergunta" });
      expect(within(secao).getByText("Prazo:").tagName).toBe("STRONG");
      expect(within(secao).getAllByRole("listitem")).toHaveLength(2);
      unmount();
    }
  });

  it("a pergunta digitada, sem Markdown, aparece como antes", () => {
    mostrar({ entrada: "Como é calculada a multa por atraso de pagamento?" });
    const secao = screen.getByRole("region", { name: "Pergunta" });
    expect(within(secao).getByText("Como é calculada a multa por atraso de pagamento?")).toBeInTheDocument();
  });

  it("o título repetido no começo da descrição aparece uma vez só", () => {
    mostrar({ entrada: ENTRADA });
    const secao = screen.getByRole("region", { name: "Pergunta" });
    expect(within(secao).getAllByText(/Multa por atraso no boleto/)).toHaveLength(1);
    expect(within(secao).getByText("Título:").tagName).toBe("STRONG");
  });

  it("tira o título repetido só quando a descrição começa por ele", () => {
    expect(semTituloRepetido("Multa\n\n# Multa\n\nTexto.")).toBe("# Multa\n\nTexto.");
    expect(semTituloRepetido("Multa por atraso\n\n**Título:** multa  por atraso\nTexto.")).toBe(
      "**Título:** multa  por atraso\nTexto.",
    );
    // Sem repetição, ou com o título mais adiante, fica como veio.
    expect(semTituloRepetido("Multa\n\nTexto que cita a Multa.")).toBe("Multa\n\nTexto que cita a Multa.");
    expect(semTituloRepetido("Multa\n\nTexto.\n\nMulta")).toBe("Multa\n\nTexto.\n\nMulta");
    expect(semTituloRepetido("  Só o título  ")).toBe("Só o título");
  });
});

describe("links da entrada, pela regra da descrição da issue (#398, #403)", () => {
  it("o link do host da issue abre no navegador; o de outro host vira texto; a imagem não carrega", () => {
    const aoAbrirLink = mostrar({ entrada: ENTRADA }, { link: ISSUE });
    const secao = screen.getByRole("region", { name: "Pergunta" });

    fireEvent.click(within(secao).getByRole("link", { name: "outra issue" }));
    expect(aoAbrirLink).toHaveBeenCalledWith("https://jira.example.com/browse/PROJ-9");

    expect(within(secao).queryByRole("link", { name: "site do banco" })).toBeNull();
    expect(within(secao).getByText("site do banco")).toHaveAttribute(
      "title",
      "https://banco.example.com/regras",
    );
    expect(secao.querySelector("img")).toBeNull();
    expect(within(secao).getByText("tela de cobrança")).toHaveAttribute(
      "title",
      "https://banco.example.com/tela.png",
    );
    expect(within(secao).getAllByRole("link")).toHaveLength(1);
  });

  it("sem issue de origem, nenhum link abre: todos aparecem como texto", () => {
    const aoAbrirLink = mostrar({ entrada: ENTRADA });
    const secao = screen.getByRole("region", { name: "Pergunta" });
    expect(within(secao).queryAllByRole("link")).toHaveLength(0);
    fireEvent.click(within(secao).getByText("outra issue"));
    expect(aoAbrirLink).not.toHaveBeenCalled();
  });

  it("na tela da análise, o link do host da issue abre pelo comando do projeto", async () => {
    const abrirUrl = vi.fn(() => Promise.resolve());
    const { api } = apiAnaliseFalsa(vistaFicticia({ link: ISSUE }, { entrada: ENTRADA }));
    render(
      <PainelAnalise
        api={api}
        projeto="faturamento-exemplo"
        analise="A-0001"
        abertura={{ tipo: "reabrir" }}
        aoVoltar={vi.fn()}
        apiTerminal={apiTerminalFalsa().api}
        apiConversa={apiConversaFalsa()}
        criarTela={() => telaFalsa()}
        abrirUrl={abrirUrl}
      />,
    );
    const secao = await screen.findByRole("region", { name: "Pergunta" });
    fireEvent.click(within(secao).getByRole("link", { name: "outra issue" }));
    await waitFor(() =>
      expect(abrirUrl).toHaveBeenCalledWith("https://jira.example.com/browse/PROJ-9"),
    );
  });
});
