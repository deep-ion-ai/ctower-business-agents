import { fireEvent, render, screen, within } from "@testing-library/react";
import type { Afirmacao } from "./analise";
import { CartaoAfirmacao } from "./CartaoAfirmacao";
import { DocumentoAoVivo } from "./DocumentoAoVivo";
import { AFIRMACAO_MULTA, vistaFicticia } from "./test/falsos";

const EVIDENCIAS: Afirmacao["evidencias"] = [
  { arquivo: "src/ParcelamentoService.java", linhas: "88-120", trecho: "parcelar()", hash: "a", estado: "valida" },
  { arquivo: "src/FaturaRepository.java", linhas: "15-22", trecho: "buscar()", hash: "b", estado: "valida" },
];

function afirmacao(texto: string, parcial: Partial<Afirmacao> = {}): Afirmacao {
  return { ...AFIRMACAO_MULTA, secao: "demanda_como_entendida", texto, evidencias: EVIDENCIAS, ...parcial } as Afirmacao;
}

function cartao(a: Afirmacao) {
  const aoAbrirEvidencia = vi.fn();
  render(
    <ol>
      <CartaoAfirmacao afirmacao={a} aoAbrirEvidencia={aoAbrirEvidencia} />
    </ol>,
  );
  return { aoAbrirEvidencia, li: screen.getByTestId(`afirmacao-${a.numero}`) };
}

beforeEach(() => {
  localStorage.clear();
});

describe("Cartão de afirmação: premissa e verificação (#404)", () => {
  it.each([
    ["não confere", "erro", "✕"],
    ["confere em parte", "atencao", "–"],
    ["confere", "concluido", "✓"],
  ])("veredito %s: dois blocos e o selo com tom %s e ícone %s", (veredito, tom, icone) => {
    const { li } = cartao(
      afirmacao(
        `Premissa do solicitante: hoje o sistema não parcela fatura nenhuma. Verificação: ${veredito}. O parcelamento existe para PJ.`,
      ),
    );
    const premissa = within(li).getByTestId("premissa");
    expect(within(premissa).getByText("Premissa do solicitante")).toBeInTheDocument();
    expect(within(premissa).getByText("hoje o sistema não parcela fatura nenhuma.")).toBeInTheDocument();
    const verificacao = within(li).getByTestId("verificacao");
    expect(within(verificacao).getByText("Verificação")).toBeInTheDocument();
    const selo = verificacao.querySelector(".selo")!;
    expect(selo).toHaveAttribute("data-tom", tom);
    expect(selo).toHaveTextContent(`${icone} ${veredito}`);
    expect(within(verificacao).getByText("O parcelamento existe para PJ.")).toBeInTheDocument();
    // O veredito não se repete no texto da verificação.
    expect(verificacao.querySelector(".texto")).not.toHaveTextContent(veredito);
  });

  it("preserva os parágrafos do que o código mostra", () => {
    const { li } = cartao(
      afirmacao(
        "Premissa do solicitante: x.\nVerificação: não confere. Existe para **pessoa jurídica**.\n\nPara pessoa física não há opção.",
      ),
    );
    const texto = within(li).getByTestId("verificacao").querySelector(".texto")!;
    expect(texto.querySelectorAll("p")).toHaveLength(2);
    expect(within(li).getByText("pessoa jurídica").tagName).toBe("STRONG");
  });

  it("fora do formato, o texto aparece como sempre, sem blocos nem selo", () => {
    const { li } = cartao(afirmacao(AFIRMACAO_MULTA.texto));
    expect(within(li).getByText(AFIRMACAO_MULTA.texto)).toBeInTheDocument();
    expect(within(li).queryByTestId("premissa")).not.toBeInTheDocument();
    // Só o selo de confiança: nenhum selo de veredito.
    expect(li.querySelector(".selo-icone")).toBeNull();
  });
});

describe("Cartão de afirmação: evidências de código (#404)", () => {
  it("ficam ocultas por padrão; o </> N abre as do cartão sob o título, e o chip abre o código", () => {
    const { li, aoAbrirEvidencia } = cartao(afirmacao(AFIRMACAO_MULTA.texto));
    expect(within(li).queryByRole("button", { name: "ParcelamentoService.java:88-120" })).toBeNull();
    const botao = within(li).getByRole("button", { name: "Ver as 2 evidências no código" });
    expect(botao).toHaveAttribute("aria-expanded", "false");
    expect(botao).toHaveTextContent("2");

    fireEvent.click(botao);
    expect(within(li).getByText("Evidências no código")).toBeInTheDocument();
    const chip = within(li).getByRole("button", { name: "ParcelamentoService.java:88-120" });
    fireEvent.click(chip);
    expect(aoAbrirEvidencia).toHaveBeenCalledWith(EVIDENCIAS[0]);

    fireEvent.click(within(li).getByRole("button", { name: "Ocultar as 2 evidências no código" }));
    expect(within(li).queryByText("Evidências no código")).toBeNull();
  });

  it("sem evidência, não há botão de código", () => {
    const { li } = cartao(afirmacao(AFIRMACAO_MULTA.texto, { evidencias: [] }));
    expect(within(li).queryByRole("button", { name: /no código$/ })).toBeNull();
  });
});

describe("Seletor global das evidências (#404)", () => {
  const AFIRMACOES = [
    afirmacao("Premissa do solicitante: a. Verificação: confere. b.", { numero: 1 }),
    afirmacao("Premissa do solicitante: c. Verificação: não confere. d.", { numero: 2 }),
  ];

  function documento() {
    return render(
      <DocumentoAoVivo
        vista={vistaFicticia({}, { afirmacoes: AFIRMACOES })}
        semCabecalho
        aoAbrirEvidencia={vi.fn()}
      />,
    );
  }

  const chips = () => screen.queryAllByRole("button", { name: /\.java:/ });

  it("desligado por padrão; mostra e esconde todas e lembra a escolha ao reabrir", () => {
    const { unmount } = documento();
    const seletor = screen.getByRole("switch", { name: "Mostrar evidências de código" });
    expect(seletor).toHaveAttribute("aria-checked", "false");
    expect(chips()).toHaveLength(0);

    fireEvent.click(seletor);
    expect(seletor).toHaveAttribute("aria-checked", "true");
    expect(chips()).toHaveLength(4);
    expect(localStorage.getItem("cba.evidenciasVisiveis")).toBe("1");

    // Reaberta, a análise mostra as evidências de novo.
    unmount();
    documento();
    expect(screen.getByRole("switch", { name: "Mostrar evidências de código" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    expect(chips()).toHaveLength(4);

    // O </> de um cartão esconde só as dele.
    fireEvent.click(
      within(screen.getByTestId("afirmacao-1")).getByRole("button", {
        name: "Ocultar as 2 evidências no código",
      }),
    );
    expect(within(screen.getByTestId("afirmacao-1")).queryAllByRole("button", { name: /\.java:/ })).toHaveLength(0);
    expect(within(screen.getByTestId("afirmacao-2")).queryAllByRole("button", { name: /\.java:/ })).toHaveLength(2);

    fireEvent.click(screen.getByRole("switch", { name: "Mostrar evidências de código" }));
    expect(chips()).toHaveLength(0);
    expect(localStorage.getItem("cba.evidenciasVisiveis")).toBe("0");
  });

  it("com o seletor desligado, o </> abre só as evidências daquele cartão", () => {
    documento();
    fireEvent.click(
      within(screen.getByTestId("afirmacao-2")).getByRole("button", {
        name: "Ver as 2 evidências no código",
      }),
    );
    expect(within(screen.getByTestId("afirmacao-1")).queryAllByRole("button", { name: /\.java:/ })).toHaveLength(0);
    expect(within(screen.getByTestId("afirmacao-2")).queryAllByRole("button", { name: /\.java:/ })).toHaveLength(2);
  });
});
