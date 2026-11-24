import { fireEvent, render, screen, within } from "@testing-library/react";
import { seloAtividades } from "./atividades";
import type { Atividade, ResumoProjeto } from "./resumoProjeto";
import { TelaAtividades } from "./TelaAtividades";
import { PROJETO, resumoFicticio } from "./test/conhecimentoFalso";
import type { AmbienteClaude } from "./analise";

const VAZIA: Omit<Atividade, "tipo" | "ordem"> = {
  analise: null,
  demanda: null,
  titulo: null,
  decididas: null,
  total: null,
  regras: null,
  atualizadaMs: null,
};

const revisar = (n: number, decididas: number, total: number): Atividade => ({
  ...VAZIA,
  tipo: "revisar",
  ordem: n,
  analise: `A-000${n}`,
  titulo: `Análise ${n}`,
  decididas,
  total,
});

/** O cenário da §1 da revisão de UX: cinco análises a revisar e o ambiente por preparar. */
function cenarioDaSecao1(): ResumoProjeto {
  return resumoFicticio({
    atividades: [revisar(3, 3, 9), revisar(4, 0, 21), revisar(5, 0, 14), revisar(6, 0, 4), revisar(7, 0, 2)],
    aguardandoRevisao: [
      {
        id: "A-0003",
        titulo: "Análise 3",
        fluxo: "historias",
        atualizadaMs: 1,
        afirmacoesDecididas: 3,
        afirmacoesTotal: 9,
        pendencias: 6,
      },
    ],
  });
}

const NAO_PRONTO: AmbienteClaude = { pronto: false, pendencias: ["servidor MCP"] };

function montar(resumo: ResumoProjeto | null, ambiente: AmbienteClaude | null | undefined = { pronto: true, pendencias: [] }) {
  const navegar = vi.fn();
  render(
    <TelaAtividades
      projeto={PROJETO}
      nomeProjeto="Faturamento Exemplo"
      resumo={resumo}
      falha={null}
      ambiente={ambiente}
      navegar={navegar}
    />,
  );
  return { navegar };
}

const grupo = (nome: string) => screen.getByRole("region", { name: nome });

describe("TelaAtividades", () => {
  it("com cinco revisões pendentes e o ambiente por preparar, mostra os dois juntos (§1)", () => {
    montar(cenarioDaSecao1(), NAO_PRONTO);
    const revisao = grupo("Revisar e aprovar");
    expect(within(revisao).getAllByRole("listitem")).toHaveLength(5);
    expect(within(revisao).getByText("5")).toBeInTheDocument();
    expect(within(revisao).getByText("Histórias e critérios · 3 de 9 decididas")).toBeInTheDocument();
    expect(within(revisao).getByRole("button", { name: "Continuar a revisão: A-0003" })).toBeInTheDocument();
    expect(within(revisao).getByRole("button", { name: "Revisar: A-0004" })).toBeInTheDocument();

    const ambiente = grupo("Ambiente a preparar");
    expect(within(ambiente).getByRole("button", { name: "Preparar" })).toBeInTheDocument();
    expect(seloAtividades(cenarioDaSecao1(), NAO_PRONTO)).toBe(6);
  });

  it("mostra os grupos na ordem, e o vazio com a frase que ensina", () => {
    montar(resumoFicticio({ atividades: [] }));
    const titulos = screen
      .getAllByRole("region")
      .filter((r) => r.classList.contains("atividades-grupo"))
      .map((r) => r.getAttribute("aria-label"));
    expect(titulos).toEqual([
      "O agente aguarda você",
      "Revisar e aprovar",
      "Regras a revalidar",
      "Ambiente a preparar",
      "Análises paradas",
    ]);
    expect(
      within(grupo("Regras a revalidar")).getByText(
        "Quando o código citado por uma regra mudar, ela aparece aqui para você conferir.",
      ),
    ).toBeInTheDocument();
    expect(within(grupo("O agente aguarda você")).queryByRole("list")).not.toBeInTheDocument();
  });

  it("o grupo das demandas novas fica oculto até a #322", () => {
    const resumo = resumoFicticio({
      atividades: [{ ...VAZIA, tipo: "demanda-sem-analise", ordem: 0, demanda: "D-0001", titulo: "Pix" }],
    });
    montar(resumo);
    expect(screen.queryByRole("region", { name: "Demandas novas sem análise" })).not.toBeInTheDocument();
    expect(screen.queryByText("Pix")).not.toBeInTheDocument();
    expect(seloAtividades(resumo, { pronto: true, pendencias: [] })).toBe(0);
  });

  it("a ação de cada item abre o objeto certo", () => {
    const { navegar } = montar(
      resumoFicticio({
        atividades: [
          { ...VAZIA, tipo: "responder", ordem: 0, analise: "A-0008", titulo: "Multa" },
          revisar(3, 0, 9),
          { ...VAZIA, tipo: "revalidar", ordem: 2, regras: 2 },
          { ...VAZIA, tipo: "retomar", ordem: 3, analise: "A-0009", titulo: "Juros" },
        ],
      }),
      NAO_PRONTO,
    );
    const reabrir = (id: string) => ({
      tipo: "analise",
      projeto: PROJETO,
      aberta: { id, abertura: { tipo: "reabrir" } },
    });

    fireEvent.click(screen.getByRole("button", { name: "Responder ao agente: A-0008" }));
    expect(navegar).toHaveBeenLastCalledWith(reabrir("A-0008"));
    fireEvent.click(screen.getByRole("button", { name: "Revisar: A-0003" }));
    expect(navegar).toHaveBeenLastCalledWith(reabrir("A-0003"));
    fireEvent.click(screen.getByRole("button", { name: "Retomar: A-0009" }));
    expect(navegar).toHaveBeenLastCalledWith(reabrir("A-0009"));

    expect(within(grupo("Regras a revalidar")).getByText("2 regras a revalidar")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Ver as regras" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "base" });
    fireEvent.click(screen.getByRole("button", { name: "Preparar" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "configurar", projeto: PROJETO });
  });

  it("num projeto sem análise, convida à primeira", () => {
    const { navegar } = montar(
      resumoFicticio({ atividades: [{ ...VAZIA, tipo: "primeira-analise", ordem: 0 }] }),
    );
    fireEvent.click(within(grupo("Primeira análise")).getByRole("button", { name: "Nova análise" }));
    expect(navegar).toHaveBeenCalledWith({ tipo: "analise", projeto: PROJETO });
  });

  it("diz quando o ambiente ainda está sendo conferido ou a conferência falhou", () => {
    const { unmount } = render(
      <TelaAtividades
        projeto={PROJETO}
        nomeProjeto="Faturamento Exemplo"
        resumo={resumoFicticio()}
        falha={null}
        ambiente={undefined}
        navegar={vi.fn()}
      />,
    );
    expect(within(grupo("Ambiente a preparar")).getByText("Conferindo o ambiente do Claude…")).toBeInTheDocument();
    unmount();
    montar(resumoFicticio(), null);
    expect(within(grupo("Ambiente a preparar")).getByText(/Não foi possível conferir/)).toBeInTheDocument();
  });

  it("mostra a falha ao ler o resumo", () => {
    render(
      <TelaAtividades
        projeto={PROJETO}
        nomeProjeto="Faturamento Exemplo"
        resumo={null}
        falha="base inacessível"
        ambiente={undefined}
        navegar={vi.fn()}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("base inacessível");
  });
});
