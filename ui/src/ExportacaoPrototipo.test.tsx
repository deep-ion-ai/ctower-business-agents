import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ExportacaoPrototipo } from "./ExportacaoPrototipo";
import type { ApiExportacaoPrototipo } from "./apiExportacaoPrototipo";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const PASTA = "/exportados/A-0003-prototipo-do-parcelamento";

function api(existe = false): ApiExportacaoPrototipo {
  let jaExiste = existe;
  return {
    escolherPasta: vi.fn().mockResolvedValue("/exportados"),
    exportar: vi.fn().mockImplementation(async (_p, _a, _d, substituir: boolean) => {
      if (jaExiste && !substituir) return { situacao: "pasta_existe", pasta: PASTA };
      jaExiste = true;
      return { situacao: "exportada", pasta: PASTA, telas: 2, semPrototipo: 1 };
    }),
  };
}

beforeEach(() => {
  try {
    localStorage.clear();
  } catch {
    // Sem armazenamento no ambiente de teste.
  }
});

describe("ExportacaoPrototipo", () => {
  it("escolhe a pasta, exporta e lembra o destino", async () => {
    const falsa = api();
    render(<ExportacaoPrototipo projeto="faturamento-exemplo" analise="A-0003" api={falsa} />);
    fireEvent.click(screen.getByRole("button", { name: "Exportar protótipo…" }));
    const status = await screen.findByRole("status");
    expect(status.textContent).toContain(PASTA);
    expect(status.textContent).toContain("2 telas");
    expect(status.textContent).toContain("1 tela sem protótipo ainda");
    expect(falsa.exportar).toHaveBeenCalledWith(
      "faturamento-exemplo",
      "A-0003",
      "/exportados",
      false,
    );
    expect(falsa.escolherPasta).toHaveBeenCalledWith(undefined);

    // Na próxima, o diálogo abre na última pasta.
    fireEvent.click(screen.getByRole("button", { name: "Exportar protótipo…" }));
    await waitFor(() => expect(falsa.escolherPasta).toHaveBeenLastCalledWith("/exportados"));
  });

  it("cancelar o diálogo não exporta", async () => {
    const falsa = api();
    vi.mocked(falsa.escolherPasta).mockResolvedValue(null);
    render(<ExportacaoPrototipo projeto="faturamento-exemplo" analise="A-0003" api={falsa} />);
    fireEvent.click(screen.getByRole("button", { name: "Exportar protótipo…" }));
    await waitFor(() => expect(falsa.escolherPasta).toHaveBeenCalled());
    expect(falsa.exportar).not.toHaveBeenCalled();
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("pasta que já existe só é substituída depois de confirmar", async () => {
    const falsa = api(true);
    render(<ExportacaoPrototipo projeto="faturamento-exemplo" analise="A-0003" api={falsa} />);
    fireEvent.click(screen.getByRole("button", { name: "Exportar protótipo…" }));
    const alerta = await screen.findByRole("alert");
    expect(alerta.textContent).toContain(`${PASTA} já existe`);
    expect(screen.queryByRole("status")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(falsa.exportar).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Exportar protótipo…" }));
    await screen.findByRole("alert");
    fireEvent.click(screen.getByRole("button", { name: "Substituir" }));
    const status = await screen.findByRole("status");
    expect(status.textContent).toContain(PASTA);
    expect(falsa.exportar).toHaveBeenLastCalledWith(
      "faturamento-exemplo",
      "A-0003",
      "/exportados",
      true,
    );
  });

  it("mostra o erro da exportação", async () => {
    const falsa = api();
    vi.mocked(falsa.exportar).mockRejectedValue("a análise A-0003 ainda não tem telas para exportar");
    render(<ExportacaoPrototipo projeto="faturamento-exemplo" analise="A-0003" api={falsa} />);
    fireEvent.click(screen.getByRole("button", { name: "Exportar protótipo…" }));
    const alerta = await screen.findByRole("alert");
    expect(alerta.textContent).toContain("ainda não tem telas");
  });
});
