import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MudancasCodigo } from "./MudancasCodigo";
import type { ApiMudancas, PropostaDeRegra, RelatorioDeMudancas } from "./mudancas";
import type { ApiTerminal } from "./terminal";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

const RELATORIO: RelatorioDeMudancas = {
  projeto: "faturamento-exemplo",
  de: {
    referencia: "1111111111111111111111111111111111111111",
    commit: "1111111111111111111111111111111111111111",
    analise: { id: "A-0001", titulo: "Multa por atraso" },
  },
  ate: { referencia: "cópia de trabalho", commit: null },
  commits: [],
  arquivos_alterados: 3,
  com_evidencias: [
    {
      arquivo: "db/03_pkg_cobranca.sql",
      situacao: "modificado",
      regras: ["RN-COB-001"],
      termos: ["Multa por atraso"],
      funcionalidades: [],
      analises: ["A-0001"],
    },
  ],
  regras: [
    {
      id: "RN-COB-001",
      titulo: "Multa por atraso",
      area: "cobranca",
      status: "a-revalidar",
      documento: "regras/RN-COB-001.md",
      classe: "alterado",
      trechos: [
        {
          arquivo: "db/03_pkg_cobranca.sql",
          linhas: "71-72",
          linhas_novas: "71-72",
          classe: "alterado",
          diff: "@@ -70,3 +70,3 @@\n-  RETURN ROUND(V * FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2);\n+  RETURN ROUND(V * FN_PARAMETRO('PERC_MULTA_ATRASO_NOVA') / 100, 4);\n",
        },
      ],
    },
  ],
  termos: [],
  funcionalidades: [],
  analises: [],
  sem_evidencias: [
    {
      area: "cobranca",
      nome: "Cobrança",
      arquivos: [{ arquivo: "db/04_pkg_situacao.sql", situacao: "modificado" }],
    },
    { nome: "relatorios", arquivos: [{ arquivo: "relatorios/inadimplencia.sql", situacao: "adicionado" }] },
  ],
  problemas: [],
  duracao_ms: 12,
};

const PROPOSTA: PropostaDeRegra = {
  numero: 1,
  regra: "RN-COB-001",
  comportamento_mudou: true,
  explicacao: "A multa passou a usar o parâmetro novo.",
  enunciado: "A multa usa o parâmetro PERC_MULTA_ATRASO_NOVA.",
  situacao: "pendente",
};

function apiFalsa(propostas: PropostaDeRegra[] = [PROPOSTA]): ApiMudancas {
  return {
    relatorio: vi.fn().mockResolvedValue(RELATORIO),
    escolherDestino: vi.fn().mockResolvedValue("/tmp/nota.md"),
    exportarNota: vi.fn().mockResolvedValue("/tmp/nota.md"),
    pedido: vi.fn().mockResolvedValue("Explique as mudanças no código"),
    propostas: vi.fn().mockResolvedValue(propostas),
    confirmar: vi.fn().mockResolvedValue({
      id: "RN-COB-001",
      documento: "regras/RN-COB-001.md",
      versoes_anteriores: 1,
      avisos: [],
    }),
    descartar: vi.fn().mockResolvedValue(undefined),
  };
}

function telaFalsa() {
  return {
    abrirEm: vi.fn(),
    escrever: vi.fn(),
    aoDigitar: vi.fn(),
    ajustar: vi.fn(() => ({ linhas: 24, colunas: 80 })),
    limpar: vi.fn(),
    focar: vi.fn(),
    descartar: vi.fn(),
  };
}

function abrir(api: ApiMudancas, apiTerminal?: ApiTerminal) {
  const aoFechar = vi.fn();
  render(
    <MudancasCodigo
      api={api}
      projeto="faturamento-exemplo"
      aoFechar={aoFechar}
      apiTerminal={apiTerminal}
      criarTela={() => telaFalsa() as never}
    />,
  );
  return aoFechar;
}

describe("MudancasCodigo", () => {
  it("mostra o relatório desde a última análise aprovada, com o diff e os arquivos sem evidência", async () => {
    const api = apiFalsa([]);
    abrir(api);
    const regra = await screen.findByRole("article", { name: "RN-COB-001 · Multa por atraso" });
    expect(api.relatorio).toHaveBeenCalledWith("faturamento-exemplo", null, null);
    expect(within(regra).getAllByText(/trecho alterado/).length).toBeGreaterThan(0);
    expect(regra.querySelector("pre.diff")?.textContent).toContain("PERC_MULTA_ATRASO_NOVA");
    expect(screen.getByText(/última análise aprovada, A-0001/)).toBeInTheDocument();

    const citados = screen.getByRole("region", { name: "Arquivos alterados citados pela base" });
    expect(citados).toHaveTextContent("RN-COB-001, termo Multa por atraso, análise A-0001");
    const sem = screen.getByRole("region", { name: "Arquivos sem evidência" });
    expect(sem).toHaveTextContent("Área Cobrança");
    expect(sem).toHaveTextContent("Pasta relatorios");
    expect(sem).toHaveTextContent("relatorios/inadimplencia.sql (adicionado)");
    expect(screen.getByText("Nenhuma proposta pendente.")).toBeInTheDocument();
  });

  it("compara o intervalo pedido e exporta a nota dele", async () => {
    const api = apiFalsa([]);
    abrir(api);
    await screen.findByRole("article", { name: /RN-COB-001/ });
    const form = screen.getByRole("form", { name: "Intervalo" });
    fireEvent.change(within(form).getByLabelText("De (commit, tag ou branch)"), {
      target: { value: " v1 " },
    });
    fireEvent.change(within(form).getByLabelText("Até"), { target: { value: "v2" } });
    fireEvent.click(within(form).getByRole("button", { name: "Comparar" }));
    await waitFor(() => expect(api.relatorio).toHaveBeenCalledWith("faturamento-exemplo", "v1", "v2"));
    await screen.findByRole("article", { name: /RN-COB-001/ });
    fireEvent.click(screen.getByRole("button", { name: "Exportar a nota de mudanças" }));
    await waitFor(() =>
      expect(api.exportarNota).toHaveBeenCalledWith("faturamento-exemplo", "v1", "v2", "/tmp/nota.md"),
    );
    expect(await screen.findByText("Nota de mudanças gravada em /tmp/nota.md.")).toBeInTheDocument();
  });

  it("não exporta quando o analista cancela o diálogo", async () => {
    const api = apiFalsa([]);
    (api.escolherDestino as ReturnType<typeof vi.fn>).mockResolvedValue(null);
    abrir(api);
    await screen.findByRole("article", { name: /RN-COB-001/ });
    fireEvent.click(screen.getByRole("button", { name: "Exportar a nota de mudanças" }));
    await waitFor(() => expect(api.escolherDestino).toHaveBeenCalled());
    expect(api.exportarNota).not.toHaveBeenCalled();
  });

  it("confirma a proposta com o texto editado e descarta outra", async () => {
    const api = apiFalsa();
    abrir(api);
    const proposta = await screen.findByRole("article", { name: "Proposta para RN-COB-001" });
    expect(proposta).toHaveTextContent("o comportamento mudou");
    const texto = within(proposta).getByLabelText(/Texto da regra/);
    expect(texto).toHaveValue("A multa usa o parâmetro PERC_MULTA_ATRASO_NOVA.");
    fireEvent.change(texto, { target: { value: "Texto ajustado pelo analista." } });
    fireEvent.click(within(proposta).getByRole("button", { name: "Confirmar" }));
    await waitFor(() =>
      expect(api.confirmar).toHaveBeenCalledWith(
        "faturamento-exemplo",
        1,
        null,
        "Texto ajustado pelo analista.",
      ),
    );
    expect(await screen.findByText(/RN-COB-001 atualizada; a versão anterior/)).toBeInTheDocument();
    await waitFor(() => expect(api.propostas).toHaveBeenCalledTimes(2));

    fireEvent.click(within(proposta).getByRole("button", { name: "Descartar" }));
    await waitFor(() => expect(api.descartar).toHaveBeenCalledWith("faturamento-exemplo", 1));
  });

  it("inicia a explicação das mudanças com o Claude", async () => {
    const api = apiFalsa([]);
    const abrirTerminal = vi.fn().mockResolvedValue(7);
    const apiTerminal = {
      abrir: abrirTerminal,
      escrever: vi.fn().mockResolvedValue(undefined),
      redimensionar: vi.fn().mockResolvedValue(undefined),
      encerrar: vi.fn().mockResolvedValue(undefined),
      enviarPrompt: vi.fn().mockResolvedValue(undefined),
    } as unknown as ApiTerminal;
    abrir(api, apiTerminal);
    await screen.findByRole("article", { name: /RN-COB-001/ });
    fireEvent.click(screen.getByRole("button", { name: "Explicar mudanças com o Claude" }));
    await waitFor(() => expect(api.pedido).toHaveBeenCalledWith(null, null));
    await waitFor(() => expect(abrirTerminal).toHaveBeenCalled());
    expect(abrirTerminal.mock.calls[0][0]).toBe("faturamento-exemplo");
    expect(abrirTerminal.mock.calls[0][1]).toMatchObject({
      promptInicial: "Explique as mudanças no código",
    });
    expect(screen.queryByRole("button", { name: "Explicar mudanças com o Claude" })).toBeNull();
  });

  it("mostra a recusa do relatório e fecha", async () => {
    const api = apiFalsa([]);
    (api.relatorio as ReturnType<typeof vi.fn>).mockRejectedValue(
      "nenhuma análise aprovada tem o commit gravado nas evidências; informe `de`",
    );
    const aoFechar = abrir(api);
    expect(await screen.findByRole("alert")).toHaveTextContent("nenhuma análise aprovada");
    fireEvent.click(screen.getByRole("button", { name: "Fechar" }));
    expect(aoFechar).toHaveBeenCalled();
  });
});
