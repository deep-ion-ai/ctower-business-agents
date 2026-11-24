import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { NovaAnalise } from "./NovaAnalise";
import type { ApiIssues, IssueDoProjeto, ListaIssues } from "./issuesProjeto";
import { PROJETO, apiConhecimentoFalsa } from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const issue = (chave: string, titulo: string): IssueDoProjeto => ({
  origem: "github",
  chave,
  titulo,
  descricao: "",
  url: "",
  criadaEm: "2026-11-20T00:00:00Z",
  estado: "aberta",
  rotulos: [],
  situacao: "sem-analise",
  analise: null,
  demanda: null,
  funcionalidades: [],
  analises: [],
});

function montar(listar: ApiIssues["listar"]) {
  const falsa = apiConhecimentoFalsa();
  const issues: ApiIssues = {
    listar,
    analisarImpacto: vi.fn(),
    aoAtualizar: () => Promise.resolve(() => undefined),
  };
  const api = { ...falsa.api, issues };
  render(
    <NovaAnalise
      api={api}
      fluxo="impacto"
      projetos={[{ id: PROJETO, nome: "Faturamento Exemplo" }] as never}
      aoCriar={vi.fn()}
      aoCancelar={vi.fn()}
    />,
  );
  return api;
}

const lista = (issues: IssueDoProjeto[]): ListaIssues => ({
  issues,
  total: issues.length,
  rotulos: [],
  lidaEm: "2026-11-24T00:00:00Z",
  atualizando: false,
  aviso: null,
  excedeuTeto: false,
  desde: "2026-05-24",
});

describe("NovaAnalise: Issue do projeto vira busca sobre a lista das issues (#323)", () => {
  it("sugere issues abertas pelo número ou título e usa a chave escolhida", async () => {
    const listar = vi.fn((_p: string, f?: { busca?: string }) =>
      Promise.resolve(
        lista(
          (f?.busca ?? "").includes("multa")
            ? [issue("#377", "Multa por atraso: carência por produto")]
            : [issue("#412", "Portabilidade"), issue("#377", "Multa por atraso: carência por produto")],
        ),
      ),
    );
    const api = montar(listar);
    const campo = await screen.findByLabelText(/Issue ou demanda/);
    fireEvent.focus(campo);
    const opcoes = await screen.findByRole("list", { name: "Demandas e issues do projeto" });
    // As demandas abertas do projeto vêm antes das issues, no mesmo campo (#350).
    expect(within(opcoes).getAllByRole("button").map((b) => b.querySelector(".id")?.textContent)).toEqual([
      "D-0001",
      "#412",
      "#377",
    ]);
    expect(listar).toHaveBeenCalledWith(PROJETO, { busca: "", estado: "abertas" });

    fireEvent.change(campo, { target: { value: "multa" } });
    await waitFor(() => expect(within(screen.getByRole("list", { name: "Demandas e issues do projeto" })).getAllByRole("button")).toHaveLength(1));
    // O mousedown na lista não tira o foco do campo: no WebKit, o blur fecharia a
    // lista antes do clique (#350).
    expect(fireEvent.mouseDown(screen.getByRole("button", { name: /#377/ }))).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: /#377/ }));
    expect(campo).toHaveValue("#377");
    expect(screen.queryByRole("list", { name: "Demandas e issues do projeto" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Iniciar a análise" }));
    await waitFor(() =>
      expect(api.criarAnaliseDeDemanda).toHaveBeenCalledWith(PROJETO, "impacto", "#377"),
    );
  });

  it("sem a lista, o campo segue como texto livre", async () => {
    const api = montar(() => Promise.reject(new Error("sem rede")));
    const campo = await screen.findByLabelText(/Issue ou demanda/);
    fireEvent.focus(campo);
    fireEvent.change(campo, { target: { value: "PROJ-123" } });
    expect(screen.queryByRole("list", { name: "Demandas e issues do projeto" })).toBeNull();
    const iniciar = screen.getByRole("button", { name: "Iniciar a análise" });
    await waitFor(() => expect(iniciar).toBeEnabled());
    fireEvent.click(iniciar);
    await waitFor(() =>
      expect(api.criarAnaliseDeDemanda).toHaveBeenCalledWith(PROJETO, "impacto", "PROJ-123"),
    );
  });
});
