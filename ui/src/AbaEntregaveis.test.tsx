import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { AbaEntregaveis } from "./AbaEntregaveis";
import type { PropsConfigProjeto } from "./ConfigProjeto";
import {
  aplicarMetodologia,
  alternarTipo,
  ajustarTipo,
  escolherPadrao,
  type Entregaveis,
} from "./entregaveis";
import { apiConfiguracoesFalsa, configuracoesFicticias, TEXTOS } from "./test/configuracoesFalso";
import { entregaveisFicticios } from "./test/entregaveisFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

function montar(api = apiConfiguracoesFalsa()) {
  const aoPendencia = vi.fn();
  const props = {
    api,
    cfg: configuracoesFicticias(),
    projeto: configuracoesFicticias().projetos[0],
    textos: TEXTOS,
    estado: { ambiente: undefined },
    aoMudar: vi.fn(),
    aoAbrirTexto: vi.fn(),
    aoEditar: vi.fn(),
    aoReverificarAmbiente: vi.fn(),
    aoConferirProntidao: vi.fn(),
    aoIrParaAcessos: vi.fn(),
    aoPendencia,
  } as unknown as PropsConfigProjeto;
  render(<AbaEntregaveis {...props} />);
  return { api, aoPendencia };
}

const habilitados = () =>
  screen
    .getAllByRole("checkbox")
    .filter((c) => (c as HTMLInputElement).checked)
    .map((c) => c.closest("label")!.textContent!.trim());

describe("regras dos entregáveis", () => {
  const dados = entregaveisFicticios();

  it("RUP aplica a predefinição e Personalizado mantém os tipos", () => {
    const rup = aplicarMetodologia(dados, dados.entregaveis, "rup");
    expect(rup.tipos.map((t) => t.tipo)).toEqual([
      "caso-de-uso",
      "catalogo-regras",
      "especificacao-suplementar",
      "prototipo",
      "glossario",
    ]);
    expect(rup.padrao).toBe("caso-de-uso");
    const pers = aplicarMetodologia(dados, rup, "personalizado");
    expect(pers.tipos).toEqual(rup.tipos);
    expect(pers.metodologia).toBe("personalizado");
  });

  it("habilitar, desabilitar e trocar o padrão passam para Personalizado", () => {
    const rup = aplicarMetodologia(dados, dados.entregaveis, "rup");
    const semUc = alternarTipo(dados.catalogo, rup, "caso-de-uso", false);
    expect(semUc.metodologia).toBe("personalizado");
    expect(semUc.padrao).toBe("catalogo-regras");
    const comManual = alternarTipo(dados.catalogo, rup, "manual", true);
    expect(comManual.tipos[comManual.tipos.length - 1].tipo).toBe("manual");
    expect(comManual.metodologia).toBe("personalizado");
    expect(escolherPadrao(dados.predefinicoes, rup, "glossario").metodologia).toBe(
      "personalizado",
    );
    expect(escolherPadrao(dados.predefinicoes, rup, "caso-de-uso").metodologia).toBe("rup");
  });

  it("template e destino não mudam a metodologia", () => {
    const rup = aplicarMetodologia(dados, dados.entregaveis, "rup");
    const ajustado = ajustarTipo(rup, "caso-de-uso", {
      template: "caso-de-uso-empresa.md",
      destino: "repositorio_docs",
    });
    expect(ajustado.metodologia).toBe("rup");
    // Voltar à predefinição mantém o ajuste do tipo que continua.
    const agil = aplicarMetodologia(dados, ajustado, "agil");
    const rupDeNovo = aplicarMetodologia(dados, { ...agil, tipos: [...agil.tipos] }, "rup");
    expect(rupDeNovo.tipos[0]).toEqual({ tipo: "caso-de-uso", template: null, destino: "exportar" });
    const prototipo = aplicarMetodologia(
      dados,
      ajustarTipo(rup, "prototipo", { destino: "comentario_rastreador" }),
      "agil",
    ).tipos.find((t) => t.tipo === "prototipo");
    expect(prototipo?.destino).toBe("comentario_rastreador");
  });
});

describe("AbaEntregaveis", () => {
  it("projeto antigo abre em Personalizado com os tipos que o app já gerava", async () => {
    montar();
    expect(await screen.findByRole("radio", { name: "Personalizado" })).toBeChecked();
    expect(screen.getByRole("radio", { name: /Orientado a agentes/ })).toBeDisabled();
    expect(screen.getByText(/ainda usa os tipos que o app já gerava/)).toBeInTheDocument();
    expect(habilitados()).toEqual([
      "História de usuário",
      "Casos de teste",
      "Protótipo",
      "Manual do usuário",
      "Documento negocial",
    ]);
    // O ciclo de vida e a geração de cada tipo aparecem no catálogo.
    const uc = screen.getByText("Especificação de caso de uso").closest("li")!;
    expect(within(uc).getByText("documento vivo")).toBeInTheDocument();
    expect(within(uc).getByText("redigido pelo agente")).toBeInTheDocument();
    expect(within(uc).getByText("geração em breve")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Salvar" })).toBeDisabled();
  });

  it("escolher RUP habilita caso de uso (padrão), regras, suplementar, protótipo e glossário", async () => {
    const { api, aoPendencia } = montar();
    fireEvent.click(await screen.findByRole("radio", { name: "RUP" }));
    expect(habilitados()).toEqual([
      "Especificação de caso de uso",
      "Catálogo de regras de negócio",
      "Especificação suplementar",
      "Protótipo",
      "Glossário",
    ]);
    const uc = screen.getByText("Especificação de caso de uso").closest("li")!;
    expect(within(uc).getByRole("radio", { name: "Tipo padrão" })).toBeChecked();
    expect(aoPendencia).toHaveBeenLastCalledWith(true);

    fireEvent.change(screen.getByLabelText("Template de Especificação de caso de uso"), {
      target: { value: "caso-de-uso-empresa.md" },
    });
    fireEvent.change(screen.getByLabelText("Destino de Glossário"), {
      target: { value: "repositorio_docs" },
    });
    // Ajustar template e destino não tira do RUP.
    expect(screen.getByRole("radio", { name: "RUP" })).toBeChecked();

    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
    await waitFor(() => expect(api.salvarEntregaveis).toHaveBeenCalled());
    const [projeto, gravada] = api.salvarEntregaveis.mock.calls[0] as [string, Entregaveis];
    expect(projeto).toBe("faturamento-exemplo");
    expect(gravada.metodologia).toBe("rup");
    expect(gravada.padrao).toBe("caso-de-uso");
    expect(gravada.tipos[0]).toEqual({
      tipo: "caso-de-uso",
      template: "caso-de-uso-empresa.md",
      destino: "exportar",
    });
    expect(gravada.tipos.find((t) => t.tipo === "glossario")?.destino).toBe("repositorio_docs");
    expect(await screen.findByText("Entregáveis salvos (RUP).")).toBeInTheDocument();
    expect(aoPendencia).toHaveBeenLastCalledWith(false);
  });

  it("mudar um item à mão passa para Personalizado, com aviso", async () => {
    montar();
    fireEvent.click(await screen.findByRole("radio", { name: "Ágil" }));
    expect(screen.queryByText(/passou para Personalizado/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox", { name: "Glossário" }));
    expect(screen.getByRole("radio", { name: "Personalizado" })).toBeChecked();
    expect(screen.getByText("A metodologia passou para Personalizado.")).toBeInTheDocument();
    expect(screen.getByText(/predefinição de Ágil/)).toBeInTheDocument();
    // Escolher a metodologia de novo volta à predefinição e tira o aviso.
    fireEvent.click(screen.getByRole("radio", { name: "Ágil" }));
    expect(screen.queryByText(/passou para Personalizado/)).not.toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Glossário" })).not.toBeChecked();
  });

  it("desfaz as alterações e mostra a falha ao salvar", async () => {
    const api = apiConfiguracoesFalsa();
    api.salvarEntregaveis.mockRejectedValueOnce("o template _templates/x.md não existe na base");
    montar(api);
    fireEvent.click(await screen.findByRole("radio", { name: "RUP" }));
    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
    expect(
      await screen.findByText("o template _templates/x.md não existe na base"),
    ).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "RUP" })).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Desfazer as alterações" }));
    expect(screen.getByRole("radio", { name: "Personalizado" })).toBeChecked();
    expect(habilitados()).toHaveLength(5);
  });

  it("o último tipo habilitado não pode ser desabilitado", async () => {
    const api = apiConfiguracoesFalsa();
    api.entregaveisDoProjeto.mockResolvedValueOnce(
      entregaveisFicticios({
        configurada: true,
        entregaveis: {
          metodologia: "personalizado",
          tipos: [{ tipo: "manual", destino: "exportar" }],
          padrao: "manual",
        },
      }),
    );
    montar(api);
    expect(await screen.findByRole("checkbox", { name: "Manual do usuário" })).toBeDisabled();
    expect(screen.queryByText(/ainda usa os tipos/)).not.toBeInTheDocument();
  });
});
