import { premissaDaAfirmacao } from "./premissaAfirmacao";

describe("premissaDaAfirmacao (#404, mesma regra do Rust)", () => {
  it("separa a premissa, o veredito e o que o código mostra", () => {
    expect(
      premissaDaAfirmacao(
        "Premissa do solicitante: hoje o projeto não parcela fatura nenhuma. " +
          "Verificação: não confere. O parcelamento existe para PJ.",
      ),
    ).toEqual({
      premissa: "hoje o projeto não parcela fatura nenhuma.",
      verificacao: { veredito: "nao-confere", texto: "O parcelamento existe para PJ." },
    });
  });

  it("reconhece os três vereditos, com e sem acento", () => {
    const veredito = (v: string) =>
      premissaDaAfirmacao(`Premissa do solicitante: x. Verificação: ${v}`)?.verificacao?.veredito;
    expect(veredito("não confere. y")).toBe("nao-confere");
    expect(veredito("Nao confere: y")).toBe("nao-confere");
    expect(veredito("confere em parte.")).toBe("confere-em-parte");
    expect(veredito("confere.")).toBe("confere");
    expect(veredito("CONFERE — o código mostra")).toBe("confere");
    // Fora dos três: a verificação fica, sem selo.
    expect(veredito("parcialmente. y")).toBeNull();
    expect(veredito("conferem os dados")).toBeNull();
  });

  it("preserva os parágrafos da verificação", () => {
    const r = premissaDaAfirmacao(
      "Premissa do solicitante: x.\n\nVerificação: confere em parte. Primeiro.\n\nSegundo parágrafo.",
    );
    expect(r?.premissa).toBe("x.");
    expect(r?.verificacao?.texto).toBe("Primeiro.\n\nSegundo parágrafo.");
  });

  it("sem verificação, só a premissa; fora do formato, nada", () => {
    expect(premissaDaAfirmacao("Premissa do solicitante: x.")).toEqual({ premissa: "x.", verificacao: null });
    expect(premissaDaAfirmacao("Demanda como entendida: limitar a 3.")).toBeNull();
    expect(premissaDaAfirmacao("Premissa sem dois pontos")).toBeNull();
    expect(premissaDaAfirmacao("Premissa do solicitante:   Verificação: confere.")).toBeNull();
    expect(premissaDaAfirmacao("A multa é de 2%. Verificação: confere.")).toBeNull();
  });
});
