import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ListaDemandas } from "./ListaDemandas";
import { PROJETO, apiConhecimentoFalsa } from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

function montar(falsa = apiConhecimentoFalsa()) {
  const navegar = vi.fn();
  render(<ListaDemandas api={falsa.api} projeto={PROJETO} navegar={navegar} />);
  return { ...falsa, navegar };
}

describe("ListaDemandas", () => {
  it("mostra as demandas, as abertas primeiro, e abre o painel", async () => {
    const { api, navegar } = montar();
    const lista = await screen.findByRole("region", { name: "Demandas" });
    const itens = await within(lista).findAllByRole("listitem");
    expect(api.listarDemandas).toHaveBeenCalledWith(PROJETO);
    expect(itens[0]).toHaveAttribute("data-status", "aberta");
    expect(within(itens[0]).getByText("FAT-12")).toBeInTheDocument();
    expect(itens[1]).toHaveAttribute("data-status", "encerrada");
    expect(within(itens[1]).getByText("encerrada em 2026-10-20")).toBeInTheDocument();

    fireEvent.click(within(itens[0]).getByRole("button", { name: "D-0001: Parcelamento PJ" }));
    expect(navegar).toHaveBeenCalledWith({ tipo: "demanda", projeto: PROJETO, demanda: "D-0001" });
  });

  it("encerra e reabre", async () => {
    const { api } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Encerrar D-0001" }));
    await waitFor(() => expect(api.encerrarDemanda).toHaveBeenCalledWith(PROJETO, "D-0001"));
    // A lista é relida depois da mudança.
    await waitFor(() => expect(api.listarDemandas).toHaveBeenCalledTimes(2));
    fireEvent.click(screen.getByRole("button", { name: "Reabrir D-0002" }));
    await waitFor(() => expect(api.reabrirDemanda).toHaveBeenCalledWith(PROJETO, "D-0002"));
  });

  it("cria uma demanda com título, descrição e chave externa", async () => {
    const { api } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Nova demanda" }));
    const form = screen.getByRole("form", { name: "Nova demanda" });
    const criar = within(form).getByRole("button", { name: "Criar demanda" });
    expect(criar).toBeDisabled();
    fireEvent.change(within(form).getByLabelText("Título"), {
      target: { value: "Desconto por pagamento antecipado" },
    });
    fireEvent.change(within(form).getByLabelText("Descrição (opcional)"), {
      target: { value: "O cliente quer 5% de desconto." },
    });
    fireEvent.change(within(form).getByLabelText("Chave externa (opcional)"), {
      target: { value: "FAT-30" },
    });
    fireEvent.click(criar);
    await waitFor(() =>
      expect(api.criarDemanda).toHaveBeenCalledWith(PROJETO, {
        titulo: "Desconto por pagamento antecipado",
        descricao: "O cliente quer 5% de desconto.",
        chave: "FAT-30",
      }),
    );
    await waitFor(() => expect(screen.queryByRole("form", { name: "Nova demanda" })).toBeNull());
    expect(api.listarDemandas).toHaveBeenCalledTimes(2);
  });

  it("mostra a falha ao criar", async () => {
    const falsa = apiConhecimentoFalsa();
    falsa.api.criarDemanda.mockRejectedValueOnce("o título não pode ficar vazio");
    montar(falsa);
    fireEvent.click(await screen.findByRole("button", { name: "Nova demanda" }));
    fireEvent.change(screen.getByLabelText("Título"), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("button", { name: "Criar demanda" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("o título não pode ficar vazio");
  });
});
