import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ExecucaoWorkflow } from "./analise";
import { CartaoAfirmacao } from "./CartaoAfirmacao";
import type { ApiConducao, ConducaoDisponivel } from "./conducao";
import { lembrarConducao, textoDaEstimativa, ultimaConducao } from "./conducao";
import { EscolhaConducao } from "./EscolhaConducao";
import { ProgressoWorkflow } from "./ProgressoWorkflow";
import { analiseFicticia } from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const EXECUCAO: ExecucaoWorkflow = {
  nome: "cba-impacto",
  run_id: "wf_1",
  sessao: "s1",
  situacao: "rodando",
  fases: [
    { titulo: "Preparar", iniciados: 1, concluidos: 1 },
    { titulo: "Investigar", iniciados: 3, concluidos: 1 },
    { titulo: "Verificar", iniciados: 0, concluidos: 0 },
    { titulo: "Consolidar", iniciados: 0, concluidos: 0 },
  ],
  iniciados: 4,
  concluidos: 2,
  iniciado_ms: 1,
  atualizado_ms: 2,
};

const DISPONIVEL: ConducaoDisponivel = {
  temWorkflow: true,
  workflow: "cba-impacto",
  disponivel: true,
  motivo: null,
  prepararDeNovo: false,
  frentes: 6,
  maxAgentes: 14,
  estimativa: null,
};

describe("ProgressoWorkflow (#371)", () => {
  it("mostra as fases com n de m agentes enquanto o workflow roda", () => {
    render(
      <ProgressoWorkflow analise={analiseFicticia({ conducao: "workflow", workflow: EXECUCAO })} />,
    );
    expect(screen.getByText("rodando")).toBeInTheDocument();
    expect(screen.getByText("1 de 3 agentes")).toBeInTheDocument();
    expect(screen.getByText("1 de 1 agente")).toBeInTheDocument();
    expect(screen.getAllByText("aguardando")).toHaveLength(2);
    expect(screen.queryByRole("button", { name: "Continuar sem workflow" })).toBeNull();
  });

  it("avisa a condução sem workflow e oferece continuar sem ele", () => {
    const continuar = vi.fn();
    render(
      <ProgressoWorkflow
        analise={analiseFicticia({
          conducao: "workflow",
          aviso_conducao: "a sessão terminou a resposta sem iniciar o workflow",
        })}
        pedidoSemWorkflow="Continue a análise A-0001 pelo ctower sem workflow"
        aoContinuarSemWorkflow={continuar}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "Conduzida sem workflow: a sessão terminou a resposta sem iniciar o workflow",
    );
    fireEvent.click(screen.getByRole("button", { name: "Continuar sem workflow" }));
    expect(continuar).toHaveBeenCalledWith("Continue a análise A-0001 pelo ctower sem workflow");
  });

  it("não aparece na condução simples", () => {
    const { container } = render(<ProgressoWorkflow analise={analiseFicticia()} />);
    expect(container).toBeEmptyDOMElement();
  });
});

describe("EscolhaConducao (#371)", () => {
  const api = (d: ConducaoDisponivel): ApiConducao => ({
    disponivel: vi.fn().mockResolvedValue(d),
    escolher: vi.fn(),
  });

  it("mostra o número máximo de agentes e deixa escolher a aprofundada", async () => {
    const mudar = vi.fn();
    render(
      <EscolhaConducao api={api(DISPONIVEL)} projeto="p" fluxo="impacto" valor="simples" aoMudar={mudar} />,
    );
    await waitFor(() => expect(screen.getByText(/até 14 agentes \(6 frentes\)/)).toBeInTheDocument());
    fireEvent.click(screen.getByLabelText(/Aprofundada/));
    expect(mudar).toHaveBeenCalledWith("workflow");
  });

  it("desabilita a aprofundada com o motivo e volta à simples", async () => {
    const mudar = vi.fn();
    render(
      <EscolhaConducao
        api={api({ ...DISPONIVEL, disponivel: false, motivo: "o Claude Code 2.1.300 não roda os workflows" })}
        projeto="p"
        fluxo="impacto"
        valor="workflow"
        aoMudar={mudar}
      />,
    );
    await waitFor(() => expect(screen.getByText(/Aprofundada indisponível: o Claude Code 2.1.300/)).toBeInTheDocument());
    expect(screen.getByLabelText(/Aprofundada/)).toBeDisabled();
    expect(mudar).toHaveBeenCalledWith("simples");
  });

  it("some nos tipos sem workflow", async () => {
    const a = api({ ...DISPONIVEL, temWorkflow: false, disponivel: false });
    const { container } = render(
      <EscolhaConducao api={a} projeto="p" fluxo="perguntar" valor="simples" aoMudar={vi.fn()} />,
    );
    await waitFor(() => expect(a.disponivel).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it("estimativa e a última escolha por tipo", () => {
    expect(
      textoDaEstimativa({
        ...DISPONIVEL,
        estimativa: { tokensMinimo: 900_000, tokensMaximo: 2_400_000, razaoSimples: 1.6, minutosMinimo: 6, minutosMaximo: 9 },
      }),
    ).toBe("até 14 agentes (6 frentes); cerca de 900 mil a 2,4 mi tokens e 6 a 9 min, cerca de 1,6× a simples");
    expect(ultimaConducao("impacto")).toBe("simples");
    lembrarConducao("impacto", "workflow");
    expect(ultimaConducao("impacto")).toBe("workflow");
    expect(ultimaConducao("perguntar")).toBe("simples");
  });
});

describe("Contestação no cartão da afirmação (#371)", () => {
  it("mostra o motivo da verificação independente", () => {
    render(
      <CartaoAfirmacao
        afirmacao={{
          numero: 3,
          secao: "como_e_hoje",
          texto: "A multa é de 2%.",
          confianca: "confirmado",
          evidencias: [],
          estado: "a-revalidar",
          contestacao: {
            origem: "verificacao-independente",
            motivo: "O trecho é do cálculo de juros.",
            registrada_ms: 1,
          },
          registrada_ms: 1,
        }}
        aoAbrirEvidencia={vi.fn()}
      />,
    );
    expect(screen.getByText("a revalidar: contestada pela verificação independente")).toBeInTheDocument();
    expect(screen.getByTestId("contestacao-3")).toHaveTextContent("O trecho é do cálculo de juros.");
  });
});
