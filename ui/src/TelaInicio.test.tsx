import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaInicio } from "./TelaInicio";
import { PROJETO, apiConhecimentoFalsa, inicioFicticio } from "./test/conhecimentoFalso";
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
    expect(within(projetos).getByText(/1 regra\(s\) · 1 termo\(s\)/)).toBeInTheDocument();
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

  it("pede o preparo do ambiente antes de criar a análise", async () => {
    const falsa = apiConhecimentoFalsa();
    falsa.api.verificarAmbiente.mockResolvedValueOnce({
      pronto: false,
      pendencias: ["o servidor MCP do app não está registrado para esta pasta"],
    });
    montar(falsa);
    fireEvent.click(await screen.findByRole("button", { name: "Explicar funcionalidade" }));
    expect(
      await screen.findByText(/Prepare o ambiente do Claude antes de iniciar/),
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
