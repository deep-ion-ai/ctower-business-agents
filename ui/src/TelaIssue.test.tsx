import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { TelaIssue } from "./TelaIssue";
import { chaveDoObjeto } from "./Navegacao";
import type { ApiIssues, IssueDoProjeto, ListaIssues } from "./issuesProjeto";
import { PROJETO, apiConhecimentoFalsa } from "./test/conhecimentoFalso";
import telasCss from "./telas.css?raw";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

function issue(parcial: Partial<IssueDoProjeto> = {}): IssueDoProjeto {
  return {
    origem: "github",
    chave: "#412",
    titulo: "Portabilidade: informar o saldo devedor ao banco de destino",
    descricao: "O Open Finance exige o **saldo devedor** atualizado.",
    url: "https://github.com/o/r/issues/412",
    criadaEm: new Date().toISOString(),
    estado: "aberta",
    rotulos: ["open-finance"],
    situacao: "sem-analise",
    analise: null,
    demanda: null,
    funcionalidades: [],
    analises: [],
    ...parcial,
  };
}

function lista(issues: IssueDoProjeto[]): ListaIssues {
  return {
    issues,
    total: issues.length,
    rotulos: [],
    lidaEm: new Date().toISOString(),
    atualizando: false,
    aviso: null,
    excedeuTeto: false,
    desde: "2026-05-24",
  };
}

function montar(issues: IssueDoProjeto[], chave = "#412") {
  const api: ApiIssues = {
    listar: vi.fn(() => Promise.resolve(lista(issues))),
    analisarImpacto: vi.fn(() =>
      Promise.resolve({
        demanda: "D-0007",
        demandaCriada: true,
        criada: {
          analise: { id: "A-0009", titulo: "Portabilidade", fluxo: "impacto" as const, estado: "criada" as const, atualizadaMs: 9 },
          promptInicial: "Inicie a análise A-0009 pelo ctower",
        },
      }),
    ),
    aoAtualizar: vi.fn(() => Promise.resolve(() => undefined)),
  };
  const navegar = vi.fn();
  render(
    <TelaIssue
      api={apiConhecimentoFalsa().api}
      issues={api}
      projeto={PROJETO}
      origem="github"
      chave={chave}
      navegar={navegar}
    />,
  );
  return { api, navegar };
}

describe("TelaIssue (aba da issue, #398)", () => {
  it("mostra a issue inteira, com a descrição em Markdown e a ação de análise", async () => {
    montar([issue()]);
    const aba = await screen.findByRole("region", { name: "Issue #412" });
    expect(within(aba).getByText("#412")).toBeInTheDocument();
    expect(within(aba).getByText("Portabilidade: informar o saldo devedor ao banco de destino")).toBeInTheDocument();
    expect(within(aba).getByText("saldo devedor").tagName).toBe("STRONG");
    expect(within(aba).getByRole("button", { name: /Abrir no GitHub/ })).toBeInTheDocument();
    expect(within(aba).getByRole("button", { name: "Analisar impacto" })).toBeInTheDocument();
  });

  it("Analisar impacto cria a análise e abre a análise com o agente", async () => {
    const { api, navegar } = montar([issue()]);
    const aba = await screen.findByRole("region", { name: "Issue #412" });
    fireEvent.click(within(aba).getByRole("button", { name: "Analisar impacto" }));
    await waitFor(() =>
      expect(navegar).toHaveBeenCalledWith({
        tipo: "analise",
        projeto: PROJETO,
        aberta: { id: "A-0009", abertura: { tipo: "nova", promptInicial: "Inicie a análise A-0009 pelo ctower" } },
      }),
    );
    expect(api.analisarImpacto).toHaveBeenCalledWith(PROJETO, "github", "#412");
  });

  it("abre a issue pelo link do rastreador e mostra a falha em português", async () => {
    vi.mocked(invoke).mockRejectedValue("o link é de evil.com, que não é o rastreador nem um repositório deste projeto");
    montar([issue()]);
    const aba = await screen.findByRole("region", { name: "Issue #412" });
    fireEvent.click(within(aba).getByRole("button", { name: /Abrir no GitHub/ }));
    await waitFor(() => expect(within(aba).getByRole("alert")).toHaveTextContent("Não foi possível abrir o link no navegador."));
    expect(invoke).toHaveBeenCalledWith("abrir_link_do_projeto_app", {
      projeto: PROJETO,
      link: "https://github.com/o/r/issues/412",
    });
  });

  it("a ação principal fica no topo, ao lado da chave, e não no fim do bloco Na base (#404)", async () => {
    const longa = Array.from({ length: 40 }, (_, i) => `Parágrafo ${i + 1} da demanda.`).join("\n\n");
    montar([issue({ descricao: longa })]);
    const aba = await screen.findByRole("region", { name: "Issue #412" });
    const topo = within(aba).getByRole("group", { name: "Ações da demanda" });
    expect(within(topo).getByRole("button", { name: "Analisar impacto" })).toBeInTheDocument();
    expect(within(aba).getAllByRole("button", { name: "Analisar impacto" })).toHaveLength(1);
    // O topo vem antes da descrição; o bloco Na base não tem mais o botão.
    const descricao = aba.querySelector(".demandas-descricao")!;
    expect(topo.compareDocumentPosition(descricao) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(aba.querySelector(".demandas-nabase") as HTMLElement).queryByRole("button", { name: "Analisar impacto" })).toBeNull();
    // A descrição inteira, sem caixa de rolagem.
    expect(within(aba).getByText("Parágrafo 40 da demanda.")).toBeInTheDocument();
    const regra = /\.demandas-descricao\s*\{([^}]*)\}/.exec(telasCss)![1];
    expect(regra).not.toMatch(/max-height|overflow/);
    // Os rótulos de seção.
    expect(within(aba).getByRole("heading", { name: "Descrição" })).toHaveClass("demandas-rotulo");
    expect(within(aba).getByRole("heading", { name: "Na base" })).toHaveClass("demandas-rotulo");
  });

  it("no topo, mostra Analisando… enquanto cria a análise", async () => {
    const { api } = montar([issue()]);
    vi.mocked(api.analisarImpacto).mockReturnValue(new Promise(() => undefined));
    const aba = await screen.findByRole("region", { name: "Issue #412" });
    fireEvent.click(within(aba).getByRole("button", { name: "Analisar impacto" }));
    const topo = within(aba).getByRole("group", { name: "Ações da demanda" });
    expect(within(topo).getByRole("button", { name: "Analisando…" })).toBeDisabled();
  });

  it("issue encerrada sem demanda: o botão do topo fica bloqueado, com o motivo junto", async () => {
    montar([issue({ estado: "fechada" })]);
    const aba = await screen.findByRole("region", { name: "Issue #412" });
    const acoes = aba.querySelector(".demandas-acoes") as HTMLElement;
    expect(within(acoes).getByRole("button", { name: "Analisar impacto" })).toBeDisabled();
    expect(within(acoes).getByText(/A issue está encerrada no rastreador/)).toBeInTheDocument();
  });

  it("issue que não está na lista avisa para atualizar, em vez de mostrar vazio", async () => {
    montar([issue({ chave: "#1" })], "#412");
    expect(await screen.findByText(/não está na lista do rastreador/)).toBeInTheDocument();
  });

  it("a chave da aba é a da issue: abrir de novo traz a mesma aba, sem duplicar", () => {
    const a = chaveDoObjeto({ tipo: "issue", projeto: PROJETO, origem: "github", chave: "#412" });
    const b = chaveDoObjeto({ tipo: "issue", projeto: PROJETO, origem: "github", chave: "#412" });
    expect(a).toBe(b);
    expect(a).not.toBe(chaveDoObjeto({ tipo: "issue", projeto: PROJETO, origem: "jira", chave: "#412" }));
  });
});
