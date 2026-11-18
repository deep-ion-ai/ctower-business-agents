import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { NovoMapeamento } from "./NovoMapeamento";
import type { ApiFuncionalidades } from "./funcionalidades";
import { apiConhecimentoFalsa } from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

describe("NovoMapeamento", () => {
  it("cria a análise com a pasta, a área e a orientação", async () => {
    const criada = {
      analise: {
        id: "A-0004",
        titulo: "Mapa de funcionalidades",
        fluxo: "mapear" as const,
        estado: "criada" as const,
        atualizadaMs: 1,
      },
      promptInicial: "Inicie a análise A-0004",
    };
    const apiF: ApiFuncionalidades = {
      catalogo: vi.fn(),
      explicar: vi.fn(),
      completude: vi.fn(),
      criarMapeamento: vi.fn().mockResolvedValue(criada),
      escolherDestino: vi.fn(),
      exportar: vi.fn(),
    };
    const aoCriar = vi.fn();
    render(
      <NovoMapeamento
        api={apiConhecimentoFalsa().api}
        apiFuncionalidades={apiF}
        projetos={[{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }]}
        aoCriar={aoCriar}
        aoCancelar={vi.fn()}
      />,
    );
    const mapear = screen.getByRole("button", { name: "Mapear" });
    await waitFor(() => expect(mapear).toBeEnabled());
    fireEvent.change(screen.getByLabelText("Pasta (opcional)"), { target: { value: " db " } });
    fireEvent.change(screen.getByLabelText("Área (opcional)"), { target: { value: "Cobrança" } });
    fireEvent.click(mapear);
    await waitFor(() => expect(aoCriar).toHaveBeenCalledWith("faturamento-exemplo", criada));
    expect(apiF.criarMapeamento).toHaveBeenCalledWith(
      "faturamento-exemplo",
      "db",
      "Cobrança",
      null,
    );
  });
});
