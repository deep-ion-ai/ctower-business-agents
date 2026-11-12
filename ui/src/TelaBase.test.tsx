import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaBase } from "./TelaBase";
import { ANALISE, REGRA, SISTEMA, TERMO, apiConhecimentoFalsa } from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const SISTEMAS = [{ id: SISTEMA, nome: "Faturamento Exemplo" }];

function montar(documentoInicial?: string) {
  const falsa = apiConhecimentoFalsa();
  const navegar = vi.fn();
  render(
    <TelaBase
      api={falsa.api}
      sistemas={SISTEMAS}
      documentoInicial={documentoInicial}
      navegar={navegar}
    />,
  );
  return { ...falsa, navegar };
}

async function buscar(texto: string) {
  fireEvent.change(screen.getByRole("searchbox", { name: "Buscar" }), {
    target: { value: texto },
  });
  fireEvent.click(screen.getByRole("button", { name: "Buscar" }));
  return screen.findByRole("region", { name: "Resultados" });
}

describe("TelaBase", () => {
  it("a busca por um termo do sistema mostra a regra e a análise de origem", async () => {
    const { api } = montar();
    const resultados = await buscar("multa");
    expect(api.buscar).toHaveBeenCalledWith("multa", {
      sistema: undefined,
      tipos: [],
      deslocamento: 0,
    });
    expect(within(resultados).getByText(/2 documento\(s\) para "multa"/)).toBeInTheDocument();
    expect(
      within(resultados).getByRole("button", { name: /RN-COB-001: Passada a carência/ }),
    ).toBeInTheDocument();
    expect(
      within(resultados).getByRole("button", { name: "A-0001: Multa por atraso" }),
    ).toBeInTheDocument();
    // As palavras encontradas ficam destacadas.
    const marcas = resultados.querySelectorAll("mark");
    expect(marcas).toHaveLength(2);
    expect(marcas[0]).toHaveTextContent("multa");
  });

  it("filtra por sistema e por tipo", async () => {
    const { api } = montar();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: SISTEMA } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Regras" }));
    await buscar("carencia");
    expect(api.buscar).toHaveBeenLastCalledWith("carencia", {
      sistema: SISTEMA,
      tipos: ["regra"],
      deslocamento: 0,
    });
  });

  it("abre o documento renderizado em Markdown, sem HTML cru", async () => {
    const { api } = montar();
    const resultados = await buscar("multa");
    fireEvent.click(within(resultados).getByRole("button", { name: /RN-COB-001/ }));
    const doc = await screen.findByRole("article", {
      name: "Passada a carência, a multa é de 2% sobre o valor original",
    });
    expect(api.abrirDocumento).toHaveBeenCalledWith(REGRA);
    expect(within(doc).getByRole("heading", { name: "RN-COB-001" })).toBeInTheDocument();
    expect(within(doc).getByText("multa", { selector: "strong" })).toBeInTheDocument();
    // O <script> do Markdown aparece como texto, nunca como elemento.
    expect(doc.querySelector("script")).toBeNull();
    expect(doc).toHaveTextContent('<script>alert("x")</script>');
    // Link externo fica como texto.
    expect(within(doc).queryByRole("link", { name: "exemplo" })).not.toBeInTheDocument();
    expect(within(doc).getByText("exemplo")).toHaveAttribute(
      "title",
      "https://exemplo.invalido/pagina",
    );
  });

  it("os links e as ligações entre documentos são navegáveis", async () => {
    const { api, navegar } = montar(REGRA);
    const doc = await screen.findByRole("article", { name: /Passada a carência/ });

    fireEvent.click(within(doc).getByRole("link", { name: "termo" }));
    await waitFor(() => expect(api.abrirDocumento).toHaveBeenLastCalledWith(TERMO));

    fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
    const regra = await screen.findByRole("article", { name: /Passada a carência/ });
    // Ligação pelo campo `origem`: a análise que originou a regra.
    const ligacoes = within(regra).getByRole("navigation", { name: "Ligações" });
    fireEvent.click(within(ligacoes).getByRole("button", { name: "Multa por atraso" }));
    const analise = await screen.findByRole("article", { name: "Multa por atraso" });
    expect(api.abrirDocumento).toHaveBeenLastCalledWith(ANALISE);
    expect(within(analise).getByText("Citado por")).toBeInTheDocument();

    fireEvent.click(within(analise).getByRole("button", { name: "Abrir na tela da análise" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      sistema: SISTEMA,
      aberta: { id: "A-0001", abertura: { tipo: "reabrir" } },
    });
  });

  it("relê a busca e o documento quando a base muda fora do app", async () => {
    const { api, avisar } = montar(REGRA);
    await buscar("multa");
    await screen.findByRole("article", { name: /Passada a carência/ });
    const buscas = api.buscar.mock.calls.length;
    const aberturas = api.abrirDocumento.mock.calls.length;
    await waitFor(() => expect(api.vigiarBase).toHaveBeenCalled());
    act(() => avisar());
    await waitFor(() => expect(api.buscar.mock.calls.length).toBeGreaterThan(buscas));
    await waitFor(() => expect(api.abrirDocumento.mock.calls.length).toBeGreaterThan(aberturas));
    expect(await screen.findByRole("article", { name: /Passada a carência/ })).toBeInTheDocument();
  });
});
