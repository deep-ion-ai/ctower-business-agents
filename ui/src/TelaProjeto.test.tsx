import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaProjeto } from "./TelaProjeto";
import { ANALISE, REGRA, PROJETO, apiConhecimentoFalsa } from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

function montar(falsa = apiConhecimentoFalsa()) {
  const navegar = vi.fn();
  render(
    <TelaProjeto
      api={falsa.api}
      projetos={[{ id: PROJETO, nome: "Faturamento Exemplo" }]}
      projetoInicial={PROJETO}
      navegar={navegar}
    />,
  );
  return { ...falsa, navegar };
}

describe("TelaProjeto", () => {
  it("revalida ao abrir e destaca as regras a revalidar", async () => {
    const { api } = montar();
    const regras = await screen.findByRole("region", { name: "Regras" });
    expect(api.revalidar).toHaveBeenCalledWith(PROJETO);
    // A revalidação mudou algo: a lista é relida.
    await waitFor(() => expect(api.visaoProjeto).toHaveBeenCalledTimes(2));

    const itens = within(regras).getAllByRole("listitem");
    // O estado vem por texto; o âmbar (tom "attention") só reforça.
    expect(itens[0]).toHaveAttribute("data-status", "a-revalidar");
    expect(within(itens[0]).getByText("a revalidar")).toHaveAttribute("data-tone", "attention");
    expect(itens[1]).toHaveAttribute("data-status", "confirmada");
    expect(within(itens[1]).getByText("confirmada")).not.toHaveAttribute("data-tone", "attention");

    expect(screen.getByRole("alert")).toHaveTextContent("1 regra está a revalidar");
    expect(await screen.findByText(/passaram a revalidar: RN-COB-002/)).toBeInTheDocument();
  });

  it("mostra o glossário e as áreas, e filtra as regras pela área", async () => {
    montar();
    const glossario = await screen.findByRole("region", { name: "Glossário" });
    expect(within(glossario).getByRole("button", { name: "Carência" })).toBeInTheDocument();
    expect(within(glossario).getByText(/prazo de tolerância/)).toBeInTheDocument();

    const areas = screen.getByRole("region", { name: "Áreas" });
    expect(within(areas).getByText("1 a revalidar")).toBeInTheDocument();
    expect(within(areas).getByText(/fora do projeto.md/)).toBeInTheDocument();
    fireEvent.click(within(areas).getByRole("button", { name: "Parcelamento" }));
    const regras = screen.getByRole("region", { name: "Regras" });
    expect(within(regras).getAllByRole("listitem")).toHaveLength(1);
    expect(within(regras).getByText(/RN-PAR-001/)).toBeInTheDocument();
  });

  it("abre a regra e a análise de origem na base", async () => {
    const { navegar } = montar();
    const regras = await screen.findByRole("region", { name: "Regras" });
    fireEvent.click(within(regras).getByRole("button", { name: /RN-COB-001/ }));
    expect(navegar).toHaveBeenCalledWith({ tipo: "base", documento: REGRA });
    fireEvent.click(within(regras).getAllByRole("button", { name: "origem" })[0]);
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "base", documento: ANALISE });
  });

  it("confere de novo sob demanda", async () => {
    const { api } = montar();
    const botao = await screen.findByRole("button", { name: "Conferir de novo" });
    await waitFor(() => expect(botao).toBeEnabled());
    fireEvent.click(botao);
    await waitFor(() => expect(api.revalidar).toHaveBeenCalledTimes(2));
  });

  it("navega pelo mapa e mapeia quando ainda não há mapa", async () => {
    const falsa = apiConhecimentoFalsa();
    falsa.api.mapa.mockResolvedValueOnce(null);
    montar(falsa);
    const mapa = await screen.findByRole("region", { name: "Mapa do projeto" });
    expect(await within(mapa).findByText(/ainda não foi gerado/)).toBeInTheDocument();
    fireEvent.click(within(mapa).getByRole("button", { name: "Mapear agora" }));
    expect(await within(mapa).findByText(/14 arquivo\(s\): SQL 3/)).toBeInTheDocument();
    expect(falsa.api.mapear).toHaveBeenCalledWith(PROJETO);
    expect(within(mapa).getByText(/backend → db/)).toBeInTheDocument();

    fireEvent.click(within(mapa).getByRole("button", { name: "db/" }));
    expect(await within(mapa).findByText("03_pkg_cobranca.sql")).toBeInTheDocument();
    expect(falsa.api.mapa).toHaveBeenLastCalledWith(PROJETO, "db");
    fireEvent.click(within(mapa).getByRole("button", { name: "(raiz)" }));
    await waitFor(() => expect(falsa.api.mapa).toHaveBeenLastCalledWith(PROJETO, ""));
  });
});
