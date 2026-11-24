import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaInicio } from "./TelaInicio";
import { PROJETO, apiConhecimentoFalsa, inicioFicticio } from "./test/conhecimentoFalso";
import { apiConfiguracoesFalsa } from "./test/configuracoesFalso";
import { clientesProntos } from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("./clientes", () => ({ estadoClientes: vi.fn(() => Promise.resolve(clientesProntos())) }));

function montar(falsa = apiConhecimentoFalsa()) {
  const navegar = vi.fn();
  const aoPrimeiroUso = vi.fn();
  render(<TelaInicio api={falsa.api} navegar={navegar} aoPrimeiroUso={aoPrimeiroUso} />);
  return { ...falsa, navegar, aoPrimeiroUso };
}

describe("TelaInicio", () => {
  const concluida = {
    ...inicioFicticio().recentes[1],
    id: "A-0003",
    titulo: "Juros de mora",
    estado: "concluida" as const,
    atualizadaMs: 3,
  };

  it("sugere revisar e aprovar a análise concluída, que sai de “Em andamento” (#227)", async () => {
    const falsa = apiConhecimentoFalsa();
    const base = inicioFicticio();
    const andamento = { ...base.emAndamento[0], sessao: null };
    falsa.api.inicio.mockResolvedValue(
      inicioFicticio({
        projetos: [{ ...base.projetos[0], aRevalidar: 0 }],
        emAndamento: [concluida, andamento],
        recentes: [concluida, andamento],
      }),
    );
    const { navegar } = montar(falsa);
    const passo = await screen.findByRole("region", { name: "Próximo passo" });
    expect(within(passo).getByText("Revisar e aprovar a A-0003")).toBeInTheDocument();
    fireEvent.click(within(passo).getByRole("button", { name: "Revisar a análise" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      aberta: { id: "A-0003", abertura: { tipo: "reabrir" } },
    });

    const andamentoLista = screen.getByRole("region", { name: "Em andamento" });
    expect(within(andamentoLista).getAllByRole("listitem")).toHaveLength(1);
    expect(within(andamentoLista).getByText(/A-0002/)).toBeInTheDocument();
    expect(within(andamentoLista).queryByText(/A-0003/)).not.toBeInTheDocument();
    const revisao = screen.getByRole("region", { name: "Aguardando revisão" });
    expect(within(revisao).getByText(/A-0003: Juros de mora/)).toBeInTheDocument();
  });

  it("sugere responder ao agente que aguarda, antes de qualquer outro passo (#227)", async () => {
    montar();
    const passo = await screen.findByRole("region", { name: "Próximo passo" });
    expect(within(passo).getByText("O agente aguarda você na A-0002")).toBeInTheDocument();
  });

  it("sugere preparar o ambiente quando o Claude do projeto não está preparado (#227)", async () => {
    const falsa = apiConhecimentoFalsa();
    const base = inicioFicticio();
    falsa.api.inicio.mockResolvedValue(
      inicioFicticio({
        projetos: [{ ...base.projetos[0], aRevalidar: 0 }],
        emAndamento: [],
        recentes: [base.recentes[1]],
      }),
    );
    falsa.api.verificarAmbiente.mockResolvedValue({ pronto: false, pendencias: ["mcp"] });
    const { navegar } = montar(falsa);
    const passo = await screen.findByRole("region", { name: "Próximo passo" });
    fireEvent.click(await within(passo).findByRole("button", { name: "Preparar" }));
    expect(navegar).toHaveBeenCalledWith({ tipo: "configurar", projeto: base.projetos[0].id });
  });

  it("num projeto sem análise, sugere a primeira análise e abre a pergunta (#227)", async () => {
    const falsa = apiConhecimentoFalsa();
    const base = inicioFicticio();
    falsa.api.inicio.mockResolvedValue(
      inicioFicticio({
        projetos: [{ ...base.projetos[0], aRevalidar: 0, analises: 0 }],
        emAndamento: [],
        recentes: [],
      }),
    );
    montar(falsa);
    const passo = await screen.findByRole("region", { name: "Próximo passo" });
    fireEvent.click(within(passo).getByRole("button", { name: "Fazer uma pergunta" }));
    expect(await screen.findByRole("form", { name: "Perguntar ao sistema" })).toBeInTheDocument();
  });

  it("“Novo projeto” abre o assistente numa modal; Esc e Cancelar fecham", async () => {
    const falsa = apiConhecimentoFalsa();
    const apiProjeto = apiConfiguracoesFalsa();
    render(
      <TelaInicio api={falsa.api} navegar={vi.fn()} apiProjeto={apiProjeto} aoMudarProjetos={vi.fn()} />,
    );
    const projetos = await screen.findByRole("region", { name: "Projetos" });
    const botao = within(projetos).getByRole("button", { name: "+ Novo projeto" });
    botao.focus();
    fireEvent.click(botao);
    const modal = screen.getByRole("dialog", { name: "Novo projeto" });
    expect(modal).toHaveAttribute("aria-modal", "true");
    expect(within(modal).getByRole("form", { name: "Cadastrar projeto" })).toBeInTheDocument();
    fireEvent.keyDown(modal, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(botao).toHaveFocus();
    fireEvent.click(botao);
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancelar" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });


  it("mostra os projetos, as análises em andamento e recentes e os clientes de IA", async () => {
    montar();
    const andamento = await screen.findByRole("region", { name: "Em andamento" });
    expect(
      within(andamento).getByRole("button", { name: "A-0002: Parcelamento para pessoa jurídica" }),
    ).toBeInTheDocument();
    expect(within(andamento).getByText(/Aguardando você/)).toBeInTheDocument();
    const recentes = screen.getByRole("region", { name: "Recentes" });
    expect(within(recentes).getAllByRole("listitem")).toHaveLength(2);
    expect(within(recentes).getByText(/aprovada/)).toBeInTheDocument();

    const projetos = screen.getByRole("region", { name: "Projetos" });
    expect(within(projetos).getByText(/1 regra · 1 termo/)).toBeInTheDocument();
    expect(within(projetos).getByText("1 a revalidar")).toHaveAttribute("data-tone", "attention");

    const clientes = screen.getByRole("region", { name: "Clientes de IA" });
    expect(await within(clientes).findByText("Instalado, versão 2.1.0")).toBeInTheDocument();
    expect(within(clientes).getByText("Logado")).toBeInTheDocument();
  });

  it("abre a análise da lista e o projeto", async () => {
    const { navegar } = montar();
    const recentes = await screen.findByRole("region", { name: "Recentes" });
    fireEvent.click(within(recentes).getByRole("button", { name: "A-0001: Multa por atraso" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      aberta: { id: "A-0001", abertura: { tipo: "reabrir" } },
    });
    fireEvent.click(screen.getByRole("button", { name: "Faturamento Exemplo" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "projeto", projeto: PROJETO });
  });

  it("os botões dos fluxos criam a análise e abrem a tela da análise", async () => {
    const { api, navegar } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Analisar impacto" }));
    const form = screen.getByRole("form", { name: "Analisar impacto" });
    const iniciar = within(form).getByRole("button", { name: "Iniciar a análise" });
    expect(iniciar).toBeDisabled();
    fireEvent.change(within(form).getByLabelText("Texto da demanda"), {
      target: { value: "Permitir parcelamento em até 3 vezes para pessoa jurídica." },
    });
    await waitFor(() => expect(iniciar).toBeEnabled());
    fireEvent.click(iniciar);
    await waitFor(() =>
      expect(navegar).toHaveBeenCalledWith({
        tipo: "analise",
        projeto: PROJETO,
        aberta: {
          id: "A-0003",
          abertura: { tipo: "nova", promptInicial: "Inicie a análise A-0003 pelo ctower" },
        },
      }),
    );
    expect(api.criarAnalise).toHaveBeenCalledWith(
      PROJETO,
      "impacto",
      "Permitir parcelamento em até 3 vezes para pessoa jurídica.",
      undefined,
    );
  });

  it("liga a análise a uma demanda aberta, se o analista escolher", async () => {
    const { api } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Perguntar ao sistema" }));
    const form = screen.getByRole("form", { name: "Perguntar ao sistema" });
    const select = await within(form).findByLabelText("Demanda (opcional)");
    // Só as abertas.
    expect(within(select).getAllByRole("option").map((o) => o.textContent)).toEqual([
      "Sem demanda",
      "D-0001: Parcelamento PJ",
    ]);
    fireEvent.change(select, { target: { value: "D-0001" } });
    fireEvent.change(within(form).getByLabelText("Pergunta"), {
      target: { value: "Como funciona o parcelamento?" },
    });
    const iniciar = within(form).getByRole("button", { name: "Iniciar a análise" });
    await waitFor(() => expect(iniciar).toBeEnabled());
    fireEvent.click(iniciar);
    await waitFor(() =>
      expect(api.criarAnalise).toHaveBeenCalledWith(
        PROJETO,
        "perguntar",
        "Como funciona o parcelamento?",
        undefined,
        "D-0001",
      ),
    );
  });

  it("inicia a análise a partir de uma issue do projeto", async () => {
    const { api } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Analisar impacto" }));
    const iniciar = await screen.findByRole("button", { name: "Iniciar a análise" });
    expect(iniciar).toBeDisabled();
    fireEvent.change(screen.getByLabelText(/Issue do projeto/), { target: { value: "PROJ-123" } });
    await waitFor(() => expect(iniciar).toBeEnabled());
    fireEvent.click(iniciar);
    await waitFor(() =>
      expect(api.criarAnaliseDeDemanda).toHaveBeenCalledWith(PROJETO, "impacto", "PROJ-123"),
    );
    expect(api.criarAnalise).not.toHaveBeenCalled();
  });

  it("revisa um requisito pela chave no rastreador ou pelo texto colado", async () => {
    const { api } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Revisar requisito" }));
    const form = await screen.findByRole("form", { name: "Revisar requisito" });
    const iniciar = within(form).getByRole("button", { name: "Iniciar a análise" });
    fireEvent.change(within(form).getByLabelText(/Chave no rastreador/), {
      target: { value: "PROJ-482" },
    });
    await waitFor(() => expect(iniciar).toBeEnabled());
    fireEvent.click(iniciar);
    await waitFor(() =>
      expect(api.criarAnaliseDeDemanda).toHaveBeenCalledWith(PROJETO, "revisar", "PROJ-482"),
    );
    expect(within(form).getByLabelText(/Requisito ou história a revisar/)).toBeInTheDocument();
  });

  it("pede o preparo do ambiente antes de criar a análise", async () => {
    const falsa = apiConhecimentoFalsa();
    falsa.api.verificarAmbiente.mockResolvedValue({
      pronto: false,
      pendencias: ["o servidor MCP do app não está registrado para esta pasta"],
    });
    montar(falsa);
    fireEvent.click(await screen.findByRole("button", { name: "Explicar funcionalidade" }));
    expect(
      await screen.findByText(/Ligue o agente a este projeto antes de iniciar/),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Funcionalidade")).toBeInTheDocument();
  });

  it("sem projeto, leva ao primeiro uso e desliga os fluxos", async () => {
    const falsa = apiConhecimentoFalsa();
    falsa.api.inicio.mockResolvedValueOnce(
      inicioFicticio({ projetos: [], emAndamento: [], recentes: [] }),
    );
    const { aoPrimeiroUso } = montar(falsa);
    fireEvent.click(await screen.findByRole("button", { name: "Abrir o primeiro uso" }));
    expect(aoPrimeiroUso).toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Perguntar ao sistema" })).toBeDisabled();
  });
});
