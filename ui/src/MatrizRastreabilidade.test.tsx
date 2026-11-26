import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ConsultaReversa } from "./ConsultaReversa";
import { MatrizRastreabilidade } from "./MatrizRastreabilidade";
import {
  PROJETO_BASE,
  apiRastreabilidadeFalsa,
  matrizFicticia,
} from "./test/rastreabilidadeFalsa";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

const SERVICO = "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java";

describe("MatrizRastreabilidade", () => {
  function montar() {
    const api = apiRastreabilidadeFalsa();
    const navegar = vi.fn();
    render(
      <MatrizRastreabilidade
        projeto="faturamento-exemplo"
        demanda="D-0001"
        navegar={navegar}
        api={api}
      />,
    );
    return { api, navegar };
  }

  it("mostra a matriz por história com as lacunas destacadas", async () => {
    const { api, navegar } = montar();
    const tabela = await screen.findByRole("table", { name: "Matriz por história" });
    expect(api.matriz).toHaveBeenCalledWith("faturamento-exemplo", "D-0001");
    const linhas = within(tabela).getAllByRole("row").slice(1);
    expect(linhas).toHaveLength(3);
    expect(linhas[0]).toHaveTextContent("HU-0001 Parcelar fatura de pessoa física aprovada");
    expect(linhas[0]).toHaveTextContent(`${SERVICO}:30-32 a revalidar`);
    expect(linhas[0]).toHaveTextContent("evidência a revalidar");
    expect(linhas[0]).toHaveAttribute("data-alerta");
    expect(linhas[1]).toHaveTextContent("nenhum cenário");
    expect(linhas[1]).toHaveTextContent("sem cenário");
    expect(linhas[2]).not.toHaveAttribute("data-alerta");
    expect(within(linhas[2]).getByText("ok")).toBeInTheDocument();

    const lacunas = screen.getByRole("complementary", { name: "Lacunas e alertas" });
    expect(lacunas).toHaveTextContent("Evidência mudou sob história aprovada");
    expect(lacunas).toHaveTextContent("Regra afetada sem história");
    expect(lacunas).toHaveTextContent("História sem cenário");
    // Cada lacuna num alerta fixo (não anunciado).
    expect(within(lacunas).getAllByRole("note")).toHaveLength(3);
    // Sem filtro, nada de "Mostrando N de N".
    expect(screen.queryByText(/Mostrando/)).toBeNull();

    // Cada célula abre o documento ou a análise.
    fireEvent.click(within(linhas[0]).getByRole("button", { name: "RN-FAT-007" }));
    expect(navegar).toHaveBeenLastCalledWith({
      tipo: "base",
      documento: `${PROJETO_BASE}/regras/RN-FAT-007.md`,
    });
    fireEvent.click(within(linhas[0]).getByRole("button", { name: "CT-0001" }));
    expect(navegar).toHaveBeenLastCalledWith({
      tipo: "analise",
      projeto: "faturamento-exemplo",
      aberta: { id: "A-0003", abertura: { tipo: "reabrir" } },
    });

    // Só lacunas e alertas.
    fireEvent.click(screen.getByRole("checkbox", { name: /só lacunas e alertas/ }));
    expect(within(tabela).getAllByRole("row").slice(1)).toHaveLength(2);
    expect(screen.getByText(/Mostrando 2 de 3 linha/)).toBeInTheDocument();
  });

  it("mostra as linhas por regra", async () => {
    montar();
    await screen.findByRole("table", { name: "Matriz por história" });
    fireEvent.change(screen.getByLabelText(/Linhas por/), { target: { value: "regra" } });
    const tabela = screen.getByRole("table", { name: "Matriz por regra" });
    const linhas = within(tabela).getAllByRole("row").slice(1);
    expect(linhas).toHaveLength(1);
    expect(linhas[0]).toHaveTextContent("RN-FAT-009 Valor mínimo da parcela");
    expect(linhas[0]).toHaveTextContent("nenhuma história");
    expect(linhas[0]).toHaveTextContent("regra afetada sem história");
  });

  it("exporta para Excel e Word", async () => {
    const { api } = montar();
    await screen.findByRole("table", { name: "Matriz por história" });
    fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Excel (.xlsx)" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Matriz exportada em /tmp/matriz-D-0001.xlsx.",
    );
    expect(api.escolherDestino).toHaveBeenCalledWith("matriz-D-0001", "xlsx");
    expect(api.exportar).toHaveBeenCalledWith(
      "faturamento-exemplo",
      "D-0001",
      "xlsx",
      "/tmp/matriz-D-0001.xlsx",
    );
    api.escolherDestino.mockResolvedValueOnce(null);
    fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Word (.docx)" }));
    await waitFor(() => expect(api.escolherDestino).toHaveBeenCalledWith("matriz-D-0001", "word"));
    expect(api.exportar).toHaveBeenCalledTimes(1);
  });

  it("explica a matriz vazia e mostra a falha", async () => {
    const api = apiRastreabilidadeFalsa();
    api.matriz.mockResolvedValueOnce({
      ...(await api.matriz()),
      analises: [],
      historias: [],
      regras: [],
      lacunas: [],
    });
    const { unmount } = render(
      <MatrizRastreabilidade projeto="p" demanda="D-0001" navegar={vi.fn()} api={api} />,
    );
    expect(await screen.findByText(/Nenhuma análise ligada à demanda/)).toBeInTheDocument();
    // Sem linhas, nem tabela nem controles.
    expect(screen.queryByRole("table")).toBeNull();
    expect(screen.queryByRole("button", { name: "Exportar" })).toBeNull();
    unmount();
    api.matriz.mockRejectedValueOnce("índice da base: ilegível");
    render(<MatrizRastreabilidade projeto="p" demanda="D-0001" navegar={vi.fn()} api={api} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("índice da base: ilegível");
  });

  it("sem histórias, mostra as lacunas em alerta e o link para gerá-las, sem tabela", async () => {
    const api = apiRastreabilidadeFalsa();
    const base = matrizFicticia();
    api.matriz.mockResolvedValueOnce({
      ...base,
      historias: [],
      lacunas: base.lacunas.filter((l) => l.alerta === "regra-sem-historia"),
    });
    const gerar = vi.fn();
    const aoCarregar = vi.fn();
    render(
      <MatrizRastreabilidade
        projeto="p"
        demanda="D-0001"
        navegar={vi.fn()}
        api={api}
        aoGerarHistorias={gerar}
        aoCarregar={aoCarregar}
      />,
    );
    const secao = await screen.findByRole("region", { name: "Matriz de rastreabilidade" });
    const alerta = await within(secao).findByRole("note");
    expect(alerta).toHaveTextContent(
      "Regra afetada sem história RN-FAT-009 aparece em A-0001 e não entrou em nenhuma história.",
    );
    expect(secao).toHaveTextContent(
      "Ainda não há histórias. A matriz aparece quando a primeira história for criada.",
    );
    expect(within(secao).queryByRole("table")).toBeNull();
    expect(within(secao).queryByRole("button", { name: "Exportar" })).toBeNull();
    expect(secao).not.toHaveTextContent(/Mostrando/);
    fireEvent.click(within(secao).getByRole("button", { name: "Gerar histórias e critérios" }));
    expect(gerar).toHaveBeenCalled();
    expect(aoCarregar).toHaveBeenCalledWith(expect.objectContaining({ historias: [] }));
  });

  it("com o filtro sem resultado, mostra uma linha em vez da tabela vazia", async () => {
    const api = apiRastreabilidadeFalsa();
    const base = matrizFicticia();
    api.matriz.mockResolvedValueOnce({
      ...base,
      historias: base.historias.map((h) => ({ ...h, alertas: [] })),
    });
    render(<MatrizRastreabilidade projeto="p" demanda="D-0001" navegar={vi.fn()} api={api} />);
    await screen.findByRole("table", { name: "Matriz por história" });
    fireEvent.click(screen.getByRole("checkbox", { name: /só lacunas e alertas/ }));
    expect(screen.queryByRole("table")).toBeNull();
    expect(screen.getByText("Nenhuma linha com lacuna ou alerta.")).toBeInTheDocument();
    expect(screen.queryByText(/Mostrando 0/)).toBeNull();
  });
});

describe("ConsultaReversa", () => {
  it("lista o que depende da regra e troca para o arquivo", async () => {
    const api = apiRastreabilidadeFalsa();
    const abrirDocumento = vi.fn();
    const abrirAnalise = vi.fn();
    render(
      <ConsultaReversa
        projeto={PROJETO_BASE}
        regra="RN-FAT-007"
        aoAbrirDocumento={abrirDocumento}
        aoAbrirAnalise={abrirAnalise}
        api={api}
      />,
    );
    const secao = await screen.findByRole("region", { name: "O que depende desta regra" });
    expect(api.dependentes).toHaveBeenCalledWith(PROJETO_BASE, {
      tipo: "regra",
      valor: "RN-FAT-007",
    });
    expect(secao).toHaveTextContent("1 demanda");
    expect(secao).toHaveTextContent("1 critério");
    const criterios = within(secao).getByRole("article", { name: "Histórias e critérios" });
    expect(criterios).toHaveTextContent("HU-0001 CA-1 são geradas 3 parcelas revisar");
    const cenarios = within(secao).getByRole("article", { name: "Cenários de teste" });
    expect(cenarios).toHaveTextContent("CT-0001");
    expect(secao).toHaveTextContent('rastreabilidade(regra: "RN-FAT-007")');

    fireEvent.click(within(secao).getByRole("button", { name: "D-0001" }));
    expect(abrirDocumento).toHaveBeenCalledWith(
      `${PROJETO_BASE}/demandas/D-0001-parcelamento-pj/demanda.md`,
    );
    fireEvent.click(within(secao).getByRole("button", { name: "A-0002" }));
    expect(abrirAnalise).toHaveBeenCalledWith(PROJETO_BASE, "A-0002");

    fireEvent.click(within(secao).getByRole("button", { name: `Arquivo ${SERVICO.split("/").pop()}` }));
    const doArquivo = await screen.findByRole("region", { name: "O que depende deste arquivo" });
    expect(api.dependentes).toHaveBeenLastCalledWith(PROJETO_BASE, {
      tipo: "arquivo",
      valor: SERVICO,
    });
    expect(
      within(doArquivo).getByRole("article", { name: "Regras com evidência no arquivo" }),
    ).toHaveTextContent("RN-FAT-008");
    expect(within(doArquivo).getByRole("button", { name: "Regra" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });
});
