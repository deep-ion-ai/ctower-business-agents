import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ApiConhecimento } from "./conhecimento";
import type { Funcionalidade } from "./funcionalidades";
import { PainelFuncionalidade } from "./PainelFuncionalidade";
import { apiFalsa, MAPA, painelDe } from "./test/mapaFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const CANCELAR = MAPA.areas[0].funcionalidades[0];
const COBRANCA = MAPA.areas[1].funcionalidades[0];

function abrir(
  f: Funcionalidade = CANCELAR,
  evidencias = false,
  painel: typeof painelDe = painelDe,
) {
  const api = apiFalsa(MAPA, painel);
  const navegar = vi.fn();
  const aoEscolher = vi.fn();
  const aoRevisar = vi.fn().mockResolvedValue(undefined);
  const revalidar = vi.fn().mockResolvedValue({ evidencias: 5, desatualizadas: 1 });
  render(
    <PainelFuncionalidade
      projeto="faturamento-exemplo"
      f={f}
      outras={[COBRANCA]}
      recarga={1}
      api={api}
      apiConhecimento={{ revalidar } as unknown as ApiConhecimento}
      revisao={{ ocupado: false, aoRevisar }}
      evidencias={evidencias}
      navegar={navegar}
      aoVoltar={vi.fn()}
      aoEscolher={aoEscolher}
    />,
  );
  return { api, navegar, aoEscolher, aoRevisar, revalidar };
}

describe("PainelFuncionalidade", () => {
  it("visão geral: indicadores, alerta, documento, análises, demandas, regras e contexto", async () => {
    const { navegar, aoEscolher, revalidar } = abrir();
    const indicadores = await screen.findByRole("region", { name: "Indicadores" });
    expect(indicadores).toHaveTextContent("Análises2");
    expect(indicadores).toHaveTextContent("1 em andamento · 1 a revisar");
    expect(indicadores).toHaveTextContent("1 a revalidar");
    expect(screen.getByRole("status")).toHaveTextContent("RN-CAN-003");
    fireEvent.click(within(screen.getByRole("status")).getByRole("button", { name: "Revalidar" }));
    await waitFor(() => expect(revalidar).toHaveBeenCalledWith("faturamento-exemplo"));

    const doc = screen.getByRole("region", { name: "Documento negocial" });
    fireEvent.click(within(doc).getByRole("button", { name: "Abrir na base" }));
    expect(navegar).toHaveBeenCalledWith({ tipo: "base", documento: CANCELAR.documento });

    const analises = screen.getByRole("region", { name: "Análises" });
    fireEvent.click(within(analises).getByRole("button", { name: /A-0019/ }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: "faturamento-exemplo",
      aberta: { id: "A-0019", abertura: { tipo: "reabrir" } },
    });
    expect(screen.getByRole("region", { name: "Pendências" })).toHaveTextContent(
      "A-0008 concluída, aguardando a sua revisão",
    );
    // Pontos de entrada só com a opção de evidências ligada.
    expect(screen.getByRole("region", { name: "Pontos de entrada" })).toHaveTextContent(
      "2 pontos de entrada",
    );
    expect(screen.getByRole("region", { name: "Pontos de entrada" })).not.toHaveTextContent(
      "db/03_pkg_cobranca.sql",
    );
    fireEvent.click(
      within(screen.getByRole("region", { name: "Relacionadas" })).getByRole("button", {
        name: "F-COB-001 · Calcular encargos",
      }),
    );
    expect(aoEscolher).toHaveBeenCalledWith("F-COB-001");
    // O enunciado completo da regra, não o título.
    expect(screen.getByRole("list", { name: "Regras" })).toHaveTextContent(
      "Só fatura com situação aberta pode ser cancelada pelo atendente.",
    );
    // Markdown do enunciado renderizado (negrito e lista), sem os asteriscos.
    const regras = screen.getByRole("list", { name: "Regras" });
    expect(within(regras).getByText("ainda não pagas").tagName).toBe("STRONG");
    // Linha 1: ID e título; linha 2: enunciado.
    expect(within(regras).getByText("Só fatura aberta")).toHaveClass("titulo-regra");
    expect(regras).not.toHaveTextContent("**");
    expect(within(regras).getByText("as vencidas;")).toBeInTheDocument();
    fireEvent.click(within(regras).getByRole("button", { name: "RN-CAN-003" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "base",
      documento: "faturamento-exemplo/regras/RN-CAN-003.md",
    });
    // Aceita e documentada, com demanda: sem próximos passos.
    expect(screen.queryByRole("region", { name: "Próximos passos" })).toBeNull();
  });

  it("aba Análises: nova pergunta com a funcionalidade e tabela com a ligação", async () => {
    const { api, navegar } = abrir();
    await screen.findByRole("region", { name: "Indicadores" });
    fireEvent.click(screen.getByRole("button", { name: "Análises (2)" }));
    const nova = screen.getByRole("region", { name: "Nova análise para esta funcionalidade" });
    // O título do formulário e o das outras seções são o mesmo título de seção (#360, #410).
    expect(
      within(nova)
        .getByRole("heading", { name: "Nova análise para esta funcionalidade" })
        .closest(".titulo-secao"),
    ).not.toBeNull();
    expect(screen.getByRole("heading", { name: "Análises" }).closest(".titulo-secao")).not.toBeNull();
    // Já explicada: Explicar desabilitado.
    expect(within(nova).getByRole("button", { name: /Explicar/ })).toBeDisabled();
    fireEvent.click(within(nova).getByRole("button", { name: /Perguntar/ }));
    const form = within(nova).getByRole("form", { name: "Criar Perguntar" });
    const criar = within(form).getByRole("button", { name: "Criar e abrir" });
    expect(criar).toBeDisabled();
    fireEvent.change(within(form).getByLabelText("Pergunta"), {
      target: { value: "Supervisor também cancela?" },
    });
    fireEvent.click(criar);
    await waitFor(() =>
      expect(api.criarAnalise).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "F-CAN-001",
        "perguntar",
        "Supervisor também cancela?",
        null,
      ),
    );
    await waitFor(() =>
      expect(navegar).toHaveBeenCalledWith({
        tipo: "analise",
        projeto: "faturamento-exemplo",
        aberta: { id: "A-0009", abertura: { tipo: "nova", promptInicial: "Inicie a análise A-0009" } },
      }),
    );
    const tabela = screen.getByRole("table");
    expect(tabela).toHaveTextContent("pelo código");
    expect(tabela).toHaveTextContent("aberta aqui");
  });

  it("manual do usuário: gerar a partir da funcionalidade aceita", async () => {
    const { api, navegar } = abrir();
    const manual = await screen.findByRole("region", { name: "Manual do usuário" });
    fireEvent.click(within(manual).getByRole("button", { name: "Gerar manual do usuário" }));
    await waitFor(() =>
      expect(api.criarAnalise).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "F-CAN-001",
        "manual",
        null,
        null,
      ),
    );
    await waitFor(() => expect(navegar).toHaveBeenCalled());
  });

  it("manual do usuário: em andamento com passos sem print e aprovado a revisar", async () => {
    abrir(CANCELAR, false, (id) => ({
      ...painelDe(id),
      manual: {
        situacao: "emAndamento",
        analise: "A-0031",
        estado: "concluida",
        passos: 3,
        semPrint: 1,
      },
    }));
    const manual = await screen.findByRole("region", { name: "Manual do usuário" });
    expect(manual).toHaveTextContent("3 passos · 1 passo sem print");
    expect(within(manual).getByRole("button", { name: "Abrir o manual A-0031" })).toBeEnabled();
    expect(within(manual).queryByRole("button", { name: "Gerar manual do usuário" })).toBeNull();
  });

  it("manual do usuário aprovado fica a revisar quando a funcionalidade muda", async () => {
    const { api, navegar } = abrir(CANCELAR, false, (id) => ({
      ...painelDe(id),
      manual: {
        situacao: "aprovado",
        analise: "A-0031",
        documento: "faturamento-exemplo/funcionalidades/cancelamento/cancelar-fatura/manual.md",
        aRevisar: ["o mapeamento da funcionalidade mudou depois da aprovação"],
      },
    }));
    const manual = await screen.findByRole("region", { name: "Manual do usuário" });
    expect(within(manual).getByText("a revisar")).toBeInTheDocument();
    expect(manual).toHaveTextContent("os prints foram mantidos");
    expect(manual).toHaveTextContent("o mapeamento da funcionalidade mudou");
    fireEvent.click(within(manual).getByRole("button", { name: "Abrir na base" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "base",
      documento: "faturamento-exemplo/funcionalidades/cancelamento/cancelar-fatura/manual.md",
    });
    fireEvent.click(within(manual).getByRole("button", { name: "Revisar o manual" }));
    await waitFor(() =>
      expect(api.criarAnalise).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "F-CAN-001",
        "manual",
        null,
        null,
      ),
    );
  });

  it("aba Demandas: etapas, ligar, desligar e analisar impacto", async () => {
    const { api, aoRevisar } = abrir();
    await screen.findByRole("region", { name: "Indicadores" });
    fireEvent.click(screen.getByRole("button", { name: "Demandas (1)" }));
    const cartao = screen.getByRole("article", { name: "D-0004: Permitir cancelamento parcial" });
    expect(within(cartao).getByRole("list", { name: "Etapas de D-0004" })).toHaveTextContent(
      "ImpactoA-0019 · em andamento",
    );
    expect(within(cartao).getByRole("link", { name: "#41" })).toHaveAttribute(
      "href",
      "https://github.com/acme/faturamento/issues/41",
    );
    expect(cartao).toHaveTextContent("O estorno parcial gera nova fatura?");
    fireEvent.click(within(cartao).getByRole("button", { name: "Desligar" }));
    await waitFor(() =>
      expect(aoRevisar).toHaveBeenCalledWith({
        tipo: "ligar-demanda",
        funcionalidade: "F-CAN-001",
        demanda: "D-0004",
        ligar: false,
      }),
    );
    const ligar = screen.getByRole("form", { name: "Ligar demanda" });
    fireEvent.change(within(ligar).getByRole("combobox"), { target: { value: "D-0007" } });
    fireEvent.click(within(ligar).getByRole("button", { name: "Ligar" }));
    await waitFor(() =>
      expect(aoRevisar).toHaveBeenLastCalledWith({
        tipo: "ligar-demanda",
        funcionalidade: "F-CAN-001",
        demanda: "D-0007",
        ligar: true,
      }),
    );
    expect(api.criarAnalise).not.toHaveBeenCalled();
  });

  it("recém-mapeada: aceitar, próximos passos e o menu de revisão", async () => {
    const { aoRevisar } = abrir(COBRANCA);
    const passos = await screen.findByRole("region", { name: "Próximos passos" });
    expect(passos).toHaveTextContent("Revisar o mapeamentoConfira");
    fireEvent.click(screen.getByRole("button", { name: "Aceitar" }));
    await waitFor(() =>
      expect(aoRevisar).toHaveBeenCalledWith({
        tipo: "decidir",
        funcionalidade: "F-COB-001",
        decisao: "aceitar",
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Revisar mapeamento" }));
    const menu = screen.getByRole("menu", { name: "Revisar mapeamento" });
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Dividir" }));
    expect(screen.getByRole("form", { name: "Dividir F-COB-001" })).toBeInTheDocument();
  });

  it("aba Código mostra os pontos com o trecho", async () => {
    abrir(CANCELAR, true);
    await screen.findByRole("region", { name: "Indicadores" });
    expect(screen.getByRole("region", { name: "Pontos de entrada" })).toHaveTextContent(
      "db/03_pkg_cobranca.sql:10",
    );
    fireEvent.click(screen.getByRole("button", { name: "Código" }));
    const codigo = screen.getByRole("region", { name: "Código da funcionalidade" });
    expect(within(codigo).getByLabelText("Evidência de PKG_COBRANCA.FN_CALCULAR_MULTA")).toHaveTextContent(
      "FUNCTION FN_CALCULAR_MULTA",
    );
  });
});
