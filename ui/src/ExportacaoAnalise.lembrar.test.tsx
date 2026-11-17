import { fireEvent, render, screen } from "@testing-library/react";
import { ExportacaoAnalise } from "./ExportacaoAnalise";
import type { ApiRevisao } from "./revisao";

const api = {} as unknown as ApiRevisao;

describe("Exportar lembra a escolha (Faturamento Exemplo)", () => {
  beforeEach(() => localStorage.clear());

  it("restaura formato e opções na próxima abertura", () => {
    const { unmount } = render(
      <ExportacaoAnalise api={api} projeto="faturamento-exemplo" analise="multa" aprovada />,
    );
    fireEvent.click(screen.getByRole("radio", { name: /Markdown/ }));
    fireEvent.click(screen.getByRole("checkbox", { name: /evidências/i }));
    unmount();
    render(<ExportacaoAnalise api={api} projeto="faturamento-exemplo" analise="multa" aprovada />);
    expect((screen.getByRole("radio", { name: /Markdown/ }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole("checkbox", { name: /evidências/i }) as HTMLInputElement).checked).toBe(false);
  });

  it("segue sem armazenamento", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("bloqueado");
    });
    render(<ExportacaoAnalise api={api} projeto="faturamento-exemplo" analise="multa" aprovada />);
    expect((screen.getByRole("radio", { name: /Word/ }) as HTMLInputElement).checked).toBe(true);
    vi.restoreAllMocks();
  });
});
