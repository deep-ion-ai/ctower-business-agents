import { fireEvent, render, screen, within } from "@testing-library/react";
import type { Achado, Parecer } from "./analise";
import { ParecerAoVivo, marcarTrechos } from "./ParecerAoVivo";
import { analiseFicticia } from "./test/falsos";

// História fictícia com a contradição plantada contra a RN-FAT-012 (#116).
const HISTORIA =
  "Como atendente, quero cancelar uma fatura.\n" +
  "- O atendente pode cancelar fatura aberta, inclusive a que já tem pagamento parcial.\n" +
  "- O cancelamento deve ser rápido.";

const EVIDENCIA = {
  arquivo: "db/03_pkg_cobranca.sql",
  linhas: "115-117",
  trecho: "IF V_VL_PAGO > 0 THEN",
  hash: "h",
  estado: "valida" as const,
};

function achado(parcial: Partial<Achado> & Pick<Achado, "numero">): Achado {
  return {
    caracteristica: "ambiguidade",
    severidade: "media",
    trecho: "rápido",
    problema: "Não diz o tempo.",
    evidencias: [],
    registrada_ms: 1,
    estado: "registrado",
    ...parcial,
  } as Achado;
}

const PARECER: Parecer = {
  achados: [
    achado({
      numero: 1,
      caracteristica: "correcao",
      severidade: "alta",
      trecho: "inclusive a que já tem pagamento parcial",
      problema: "O sistema só cancela fatura sem nenhum valor pago.",
      reescrita: "O atendente pode cancelar fatura aberta e sem pagamento.",
      regras: ["RN-FAT-012"],
      evidencias: [EVIDENCIA],
    }),
    achado({
      numero: 2,
      reescrita: "O cancelamento conclui em até 2 segundos.",
    }),
    achado({
      numero: 3,
      caracteristica: "completude",
      severidade: "baixa",
      trecho: "",
      problema: "Falta o motivo mínimo.",
      estado: "descartado",
      motivo: "Já está na tela.",
    }),
  ],
  invest: [
    {
      criterio: "testavel",
      resultado: "nao_atende",
      justificativa: '"Rápido" não tem medida.',
    },
  ],
  veredito: {
    veredito: "precisa_de_retrabalho",
    justificativa: "Contradiz a regra de cancelamento.",
    registrada_ms: 2,
  },
};

describe("marcarTrechos", () => {
  it("marca contradições e ambiguidades sem diferenciar maiúsculas", () => {
    const partes = marcarTrechos(HISTORIA, PARECER.achados);
    const marcadas = partes.filter((p) => p.marca);
    expect(marcadas).toEqual([
      {
        texto: "inclusive a que já tem pagamento parcial",
        marca: "contradicao",
      },
      { texto: "rápido", marca: "ambiguidade" },
    ]);
    expect(partes.map((p) => p.texto).join("")).toBe(HISTORIA);
    // Trecho que não está no texto não marca nada.
    expect(marcarTrechos("Outro texto", PARECER.achados)).toEqual([{ texto: "Outro texto" }]);
  });
});

describe("ParecerAoVivo", () => {
  it("mostra o veredito, os achados por severidade e a INVEST, e descarta um achado", () => {
    const aoDecidir = vi.fn(() => Promise.resolve());
    render(
      <ParecerAoVivo
        analise={analiseFicticia({
          fluxo: "revisar",
          entrada: HISTORIA,
          parecer: PARECER,
          estado: "concluida",
        })}
        aoDecidir={aoDecidir}
      />,
    );
    const veredito = screen.getByRole("region", { name: "Veredito" });
    expect(veredito).toHaveTextContent("Precisa de retrabalho");
    expect(veredito).toHaveTextContent("1 contradição");
    expect(veredito).toHaveTextContent("1 ambiguidade");

    const alta = screen.getByRole("list", { name: "Severidade alta" });
    expect(alta).toHaveTextContent("Correção");
    expect(alta).toHaveTextContent("RN-FAT-012");
    expect(alta).toHaveTextContent("db/03_pkg_cobranca.sql:115-117");
    expect(alta).toHaveTextContent(
      "Reescrita sugerida: O atendente pode cancelar fatura aberta e sem pagamento.",
    );
    expect(screen.getByRole("list", { name: "Severidade média" })).toHaveTextContent("Ambiguidade");
    expect(screen.queryByRole("list", { name: "Severidade baixa" })).toBeNull();
    expect(screen.getByText("Descartados (1)")).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Verificação INVEST" })).toHaveTextContent(
      "não atende",
    );

    fireEvent.click(within(alta).getByRole("button", { name: "Descartar" }));
    expect(aoDecidir).toHaveBeenCalledWith({
      tipo: "achado",
      numero: 1,
      estado: "descartado",
    });
    fireEvent.click(screen.getByRole("button", { name: "Restaurar" }));
    expect(aoDecidir).toHaveBeenCalledWith({
      tipo: "achado",
      numero: 3,
      estado: "registrado",
    });
  });

  it("aprovada ou sem parecer, só leitura", () => {
    render(
      <ParecerAoVivo
        analise={analiseFicticia({
          fluxo: "revisar",
          entrada: HISTORIA,
          parecer: PARECER,
          estado: "aprovada",
        })}
        aoDecidir={vi.fn()}
      />,
    );
    expect(screen.queryByRole("button", { name: "Descartar" })).toBeNull();
  });

  it("sem registros ainda", () => {
    render(<ParecerAoVivo analise={analiseFicticia({ fluxo: "revisar", entrada: HISTORIA })} />);
    expect(screen.getByText("Veredito ainda não registrado.")).toBeInTheDocument();
    expect(screen.getByText("Nenhum achado registrado.")).toBeInTheDocument();
  });
});
