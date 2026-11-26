import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { TelaDemandas, acaoDaIssue } from "./TelaDemandas";
import type { ApiConhecimento } from "./conhecimento";
import type { ApiIssues, FiltroIssues, IssueDoProjeto, ListaIssues } from "./issuesProjeto";
import type { DemandaVista } from "./demandas";
import { PROJETO, apiConhecimentoFalsa, demandasFicticias } from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const agora = new Date();
const atras = (dias: number) => new Date(agora.getTime() - dias * 86_400_000).toISOString();

function issue(parcial: Partial<IssueDoProjeto>): IssueDoProjeto {
  return {
    origem: "github",
    chave: "#412",
    titulo: "Portabilidade: informar o saldo devedor ao banco de destino",
    descricao: "O Open Finance exige o saldo devedor atualizado.",
    url: "https://github.com/o/r/issues/412",
    criadaEm: atras(1),
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

const ISSUES: IssueDoProjeto[] = [
  issue({}),
  issue({
    chave: "#398",
    titulo: "Expor contratos, parcelas e encargos pela API de dados",
    descricao: "",
    criadaEm: atras(9),
    situacao: "em-revisao",
    analise: "A-0003",
    demanda: "D-0004",
    funcionalidades: ["F-002"],
  }),
  issue({
    chave: "#377",
    titulo: "Multa por atraso: carência por produto",
    criadaEm: atras(20),
    rotulos: ["cobrança"],
    situacao: "aprovada",
    analise: "A-0001",
    demanda: "D-0005",
  }),
  issue({
    chave: "#352",
    titulo: "Portabilidade de crédito (Open Finance)",
    criadaEm: atras(30),
    situacao: "em-andamento",
    analise: "A-0005",
    demanda: "D-0006",
  }),
];

function lista(parcial: Partial<ListaIssues> = {}): ListaIssues {
  return {
    issues: ISSUES,
    total: 17,
    rotulos: ["cobrança", "open-finance"],
    lidaEm: new Date(agora.getTime() - 5 * 60_000).toISOString(),
    atualizando: false,
    aviso: null,
    excedeuTeto: false,
    desde: "2026-05-24",
    ...parcial,
  };
}

function issuesFalsa(l: ListaIssues = lista()) {
  const avisos: ((p: string) => void)[] = [];
  const api = {
    listar: vi.fn((_p: string, _f?: FiltroIssues, _forcar?: boolean) => Promise.resolve(l)),
    analisarImpacto: vi.fn(() =>
      Promise.resolve({
        demanda: "D-0007",
        demandaCriada: true,
        criada: {
          analise: {
            id: "A-0009",
            titulo: "Portabilidade",
            fluxo: "impacto" as const,
            estado: "criada" as const,
            atualizadaMs: 9,
          },
          promptInicial: "Inicie a análise A-0009 pelo ctower",
        },
      }),
    ),
    aoAtualizar: vi.fn((f: (p: string) => void) => {
      avisos.push(f);
      return Promise.resolve(() => undefined);
    }),
  } satisfies ApiIssues;
  return { api, avisar: (p = PROJETO) => avisos.forEach((f) => f(p)) };
}

function montar(issues = issuesFalsa(), extra: Partial<ApiConhecimento> = {}) {
  const falsa = apiConhecimentoFalsa();
  const api: ApiConhecimento = { ...falsa.api, issues: issues.api, ...extra };
  const navegar = vi.fn();
  render(<TelaDemandas api={api} projeto={PROJETO} navegar={navegar} />);
  return { ...issues, conhecimento: falsa.api, navegar };
}

const linha = (nome: RegExp | string) =>
  screen.getByRole("button", { name: nome }).closest("li") as HTMLElement;

describe("TelaDemandas", () => {
  it("lista as issues com situação na base e a ação que acompanha a situação", async () => {
    montar();
    const itens = await within(await screen.findByRole("list", { name: "Lista de demandas" })).findAllByRole(
      "listitem",
    );
    expect(itens).toHaveLength(4);
    // Mais novas primeiro, cada uma com número, idade, rótulo e situação.
    expect(within(itens[0]).getByText("#412")).toBeInTheDocument();
    expect(within(itens[0]).getByText(/aberta ontem · open-finance/)).toBeInTheDocument();
    expect(within(itens[0]).getByText("sem análise")).toBeInTheDocument();
    expect(within(itens[0]).getByRole("button", { name: "Analisar impacto #412" })).toBeInTheDocument();
    expect(within(itens[1]).getByText("A-0003 em revisão")).toBeInTheDocument();
    expect(within(itens[1]).getByRole("button", { name: "Abrir a análise #398" })).toBeInTheDocument();
    expect(within(itens[2]).getByText("A-0001 aprovada")).toBeInTheDocument();
    expect(within(itens[2]).getByRole("button", { name: "Abrir a demanda #377" })).toBeInTheDocument();
    expect(within(itens[3]).getByText("A-0005 em andamento")).toBeInTheDocument();
    expect(screen.getByText(/4 de 18 demandas/)).toBeInTheDocument();
    expect(screen.getByTestId("frescor")).toHaveTextContent("conferido há 5 min");
  });

  it("Analisar impacto: dois cliques, sem digitar número, e abre a análise com o agente", async () => {
    const { api, navegar } = montar();
    // Primeiro clique: Demandas (no menu); segundo: a ação da linha.
    fireEvent.click(await screen.findByRole("button", { name: "Analisar impacto #412" }));
    await waitFor(() =>
      expect(navegar).toHaveBeenCalledWith({
        tipo: "analise",
        projeto: PROJETO,
        aberta: {
          id: "A-0009",
          abertura: { tipo: "nova", promptInicial: "Inicie a análise A-0009 pelo ctower" },
        },
      }),
    );
    expect(api.analisarImpacto).toHaveBeenCalledWith(PROJETO, "github", "#412");
  });

  it("mostra a falha de Analisar impacto e deixa tentar de novo", async () => {
    const issues = issuesFalsa();
    issues.api.analisarImpacto.mockRejectedValueOnce(new Error("o agente não está ligado"));
    const { navegar } = montar(issues);
    fireEvent.click(await screen.findByRole("button", { name: "Analisar impacto #412" }));
    expect(await screen.findByText(/o agente não está ligado/)).toBeInTheDocument();
    expect(navegar).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Analisar impacto #412" })).toBeEnabled();
  });

  it("abre a análise e a demanda das issues que já têm", async () => {
    const { navegar } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Abrir a análise #398" }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      aberta: { id: "A-0003", abertura: { tipo: "reabrir" } },
    });
    fireEvent.click(screen.getByRole("button", { name: "Abrir a demanda #377" }));
    expect(navegar).toHaveBeenCalledWith({ tipo: "demanda", projeto: PROJETO, demanda: "D-0005" });
  });

  it("issue encerrada sem demanda não se analisa, e com demanda abre a demanda", () => {
    expect(acaoDaIssue(issue({ estado: "fechada" })).bloqueio).toMatch(/encerrada/);
    expect(acaoDaIssue(issue({ estado: "fechada", demanda: "D-0001" }))).toEqual({
      rotulo: "Abrir a demanda",
      tipo: "demanda",
    });
  });

  it("busca e filtros pedem a lista ao app, sem digitar número de cor", async () => {
    const { api } = montar();
    await screen.findByRole("list", { name: "Lista de demandas" });
    expect(api.listar).toHaveBeenLastCalledWith(
      PROJETO,
      { busca: "", estado: "abertas", situacao: undefined, rotulo: undefined },
      false,
    );
    fireEvent.change(screen.getByLabelText("Buscar demandas"), { target: { value: "portab" } });
    await waitFor(() =>
      expect(api.listar).toHaveBeenLastCalledWith(
        PROJETO,
        { busca: "portab", estado: "abertas", situacao: undefined, rotulo: undefined },
        false,
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: "Encerradas" }));
    await waitFor(() => expect(api.listar).toHaveBeenLastCalledWith(
      PROJETO,
      expect.objectContaining({ estado: "encerradas" }),
      false,
    ));
    fireEvent.click(screen.getByRole("button", { name: "Sem análise" }));
    await waitFor(() => expect(api.listar).toHaveBeenLastCalledWith(
      PROJETO,
      expect.objectContaining({ situacao: "sem-analise" }),
      false,
    ));
    fireEvent.change(screen.getByLabelText("Rótulo"), { target: { value: "cobrança" } });
    await waitFor(() => expect(api.listar).toHaveBeenLastCalledWith(
      PROJETO,
      expect.objectContaining({ rotulo: "cobrança", situacao: "sem-analise" }),
      false,
    ));
  });

  it("junta as demandas criadas à mão, que seguem o estado e a busca", async () => {
    montar();
    // D-0002 (sem chave) está encerrada: fora das abertas; D-0001 tem chave (é de issue).
    await screen.findByRole("list", { name: "Lista de demandas" });
    expect(screen.queryByText("D-0002")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Todas" }));
    const mao = await screen.findByText("D-0002");
    expect(within(mao.closest("li")!).getByText(/criada à mão/)).toBeInTheDocument();
    expect(within(mao.closest("li")!).getByRole("button", { name: "Abrir a demanda D-0002" })).toBeInTheDocument();
    // A situação na base não existe para a demanda à mão: o filtro a esconde.
    fireEvent.click(screen.getByRole("button", { name: "Sem análise" }));
    await waitFor(() => expect(screen.queryByText("D-0002")).toBeNull());
  });

  it("mostra o tipo da demanda na linha e no painel e filtra por tipo (#358)", async () => {
    const [base] = demandasFicticias();
    const daBase: DemandaVista[] = [
      { ...base, id: "D-0004", chave: "#398", tipo: "bug" },
      { ...base, id: "D-0005", chave: "#377", tipo: "melhoria" },
      { ...base, id: "D-0006", chave: "#352", tipo: null },
      { ...base, id: "D-0010", titulo: "Relatório de inadimplência", chave: null, tipo: "bug", criado: atras(2).slice(0, 10) },
    ];
    montar(issuesFalsa(), { listarDemandas: vi.fn(() => Promise.resolve(daBase)) });
    const lista = await screen.findByRole("list", { name: "Lista de demandas" });
    // A issue mostra o tipo da demanda ligada a ela.
    expect(await within(lista).findByText(/Corrigir bug · aberta há 9 dias/)).toBeInTheDocument();
    expect(within(lista).getByText(/Melhoria · aberta há 20 dias/)).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Tipo da demanda"), { target: { value: "bug" } });
    const itens = within(lista).getAllByRole("listitem");
    // #398 (pela demanda D-0004) e a demanda à mão D-0010; as sem tipo somem.
    expect(itens.map((i) => within(i).getByText(/^(#|D-)\d+$/).textContent)).toEqual(["D-0010", "#398"]);
    const painel = screen.getByRole("complementary", { name: "Demanda selecionada" });
    expect(within(painel).getByTitle("Tipo da demanda")).toHaveTextContent("Corrigir bug");

    fireEvent.change(screen.getByLabelText("Tipo da demanda"), { target: { value: "funcionalidade" } });
    expect(await screen.findByText("Nenhuma demanda com este filtro.")).toBeInTheDocument();
  });

  it("o painel mostra a descrição, o link do rastreador, o que há na base e os parecidos", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    const { navegar, conhecimento } = montar();
    await screen.findByRole("list", { name: "Lista de demandas" });
    const painel = screen.getByRole("complementary", { name: "Demanda selecionada" });
    expect(within(painel).getByText("O Open Finance exige o saldo devedor atualizado.")).toBeInTheDocument();
    expect(within(painel).getByText(/Ainda sem análise/)).toBeInTheDocument();
    expect(within(painel).getByText(/Ainda sem funcionalidade ligada/)).toBeInTheDocument();
    // Parecidas: a outra issue de portabilidade e a regra achada na base.
    const parecidas = await within(painel).findByRole("list", { name: "Itens parecidos" });
    expect(within(parecidas).getByText("#352")).toBeInTheDocument();
    expect(await within(parecidas).findByText("RN-COB-001")).toBeInTheDocument();
    expect(conhecimento.buscar).toHaveBeenCalledWith(expect.stringContaining("Portabilidade"), {
      projeto: PROJETO,
      tipos: ["regra"],
    });

    fireEvent.click(within(painel).getByRole("button", { name: /Abrir no GitHub/ }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("abrir_link_do_projeto_app", {
        projeto: PROJETO,
        link: "https://github.com/o/r/issues/412",
      }),
    );
    fireEvent.click(within(painel).getByRole("button", { name: "Vincular a funcionalidades" }));
    expect(navegar).toHaveBeenCalledWith({ tipo: "vincular", projeto: PROJETO });
  });

  it("selecionar outra linha troca o painel e mostra as funcionalidades ligadas", async () => {
    montar();
    await screen.findByRole("list", { name: "Lista de demandas" });
    fireEvent.click(within(linha("Abrir a análise #398")).getByText("#398"));
    const painel = screen.getByRole("complementary", { name: "Demanda selecionada" });
    expect(within(painel).getByText("A-0003 em revisão.")).toBeInTheDocument();
    expect(within(painel).getByText(/Funcionalidades: F-002\./)).toBeInTheDocument();
    expect(within(painel).getByText("A issue não tem descrição.")).toBeInTheDocument();
  });

  it("sem rede, mostra a última leitura e avisa", async () => {
    montar(issuesFalsa(lista({ aviso: "error connecting to api.github.com" })));
    const aviso = await screen.findByRole("status");
    expect(aviso).toHaveTextContent(/error connecting to api\.github\.com/);
    expect(aviso).toHaveTextContent(/última leitura, conferido há 5 min/);
    expect(screen.getByRole("button", { name: "Analisar impacto #412" })).toBeInTheDocument();
  });

  it("Atualizar relê o rastreador na hora e a releitura em segundo plano recarrega a lista", async () => {
    const issues = issuesFalsa();
    const { api, avisar } = montar(issues);
    await screen.findByRole("list", { name: "Lista de demandas" });
    fireEvent.click(screen.getByRole("button", { name: "Atualizar" }));
    await waitFor(() => expect(api.listar).toHaveBeenLastCalledWith(PROJETO, expect.anything(), true));
    await screen.findByRole("button", { name: "Atualizar" });

    const antes = api.listar.mock.calls.length;
    act(() => avisar("outro-projeto"));
    expect(api.listar).toHaveBeenCalledTimes(antes);
    act(() => avisar());
    await waitFor(() => expect(api.listar).toHaveBeenCalledTimes(antes + 1));
    expect(api.listar).toHaveBeenLastCalledWith(PROJETO, expect.anything(), false);
  });

  it("mostra que a lista está sendo atualizada em segundo plano", async () => {
    montar(issuesFalsa(lista({ atualizando: true })));
    expect(await screen.findByRole("button", { name: "Atualizando…" })).toBeDisabled();
  });

  it("sem rastreador ou sem leitura, avisa e mantém as demandas à mão", async () => {
    const issues = issuesFalsa();
    issues.api.listar.mockRejectedValue("o projeto não tem repositório de issues cadastrado");
    montar(issues);
    expect(await screen.findByRole("alert")).toHaveTextContent(/repositório de issues/);
    // As demandas da base continuam (D-0002 só aparece em "Todas"; aqui, "abertas": nenhuma).
    fireEvent.click(screen.getByRole("button", { name: "Todas" }));
    expect(await screen.findByRole("button", { name: "Abrir a demanda D-0002" })).toBeInTheDocument();
  });

  it("Nova demanda à mão cria a demanda e a seleciona", async () => {
    const { conhecimento } = montar();
    await screen.findByRole("list", { name: "Lista de demandas" });
    fireEvent.click(screen.getByRole("button", { name: "Nova demanda à mão" }));
    fireEvent.change(screen.getByLabelText("Título"), { target: { value: "Cobrança por boleto" } });
    fireEvent.click(screen.getByRole("button", { name: "Criar demanda" }));
    await waitFor(() =>
      expect(conhecimento.criarDemanda).toHaveBeenCalledWith(PROJETO, {
        titulo: "Cobrança por boleto",
        descricao: undefined,
        chave: undefined,
      }),
    );
  });

  it("sem a API das issues, mostra só as demandas da base", async () => {
    const falsa = apiConhecimentoFalsa();
    render(<TelaDemandas api={falsa.api} projeto={PROJETO} navegar={vi.fn()} />);
    expect(await screen.findByRole("region", { name: "Demandas" })).toBeInTheDocument();
    expect(await screen.findByText(/D-0001: Parcelamento PJ/)).toBeInTheDocument();
  });

  it("abre já filtrada pela situação pedida", async () => {
    const issues = issuesFalsa();
    const falsa = apiConhecimentoFalsa();
    render(
      <TelaDemandas
        api={{ ...falsa.api, issues: issues.api }}
        projeto={PROJETO}
        navegar={vi.fn()}
        situacaoInicial="sem-analise"
      />,
    );
    await waitFor(() =>
      expect(issues.api.listar).toHaveBeenCalledWith(
        PROJETO,
        expect.objectContaining({ situacao: "sem-analise" }),
        false,
      ),
    );
    expect(screen.getByRole("button", { name: "Sem análise" })).toHaveAttribute("aria-pressed", "true");
  });

  it("a descrição da issue sai em Markdown, sem marcadores crus (#398)", async () => {
    montar(
      issuesFalsa(
        lista({
          issues: [
            issue({
              descricao: "## O que fazer\n\n- item **forte**\n- outro `código`\n\nVeja a [wiki](https://github.com/o/r/wiki).",
            }),
          ],
        }),
      ),
    );
    const painel = await screen.findByRole("complementary", { name: "Demanda selecionada" });
    expect(within(painel).getByRole("heading", { name: "O que fazer", level: 2 })).toBeInTheDocument();
    expect(within(painel).getByText("forte").tagName).toBe("STRONG");
    expect(painel.querySelectorAll(".demandas-descricao li")).toHaveLength(2);
    expect(within(painel).queryByText(/\*\*forte\*\*/)).not.toBeInTheDocument();
    expect(within(painel).queryByText(/^## /)).not.toBeInTheDocument();
  });

  it("link da descrição do host da issue abre pelo comando; outro host não abre", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    montar(
      issuesFalsa(
        lista({
          issues: [
            issue({
              descricao: "Ver [a wiki](https://github.com/o/r/wiki) e [um site](https://exemplo.com/x).",
            }),
          ],
        }),
      ),
    );
    const painel = await screen.findByRole("complementary", { name: "Demanda selecionada" });
    fireEvent.click(within(painel).getByRole("link", { name: "a wiki" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("abrir_link_do_projeto_app", {
        projeto: PROJETO,
        link: "https://github.com/o/r/wiki",
      }),
    );
    // De outro host, é texto: sem link clicável, com o endereço no title.
    expect(within(painel).queryByRole("link", { name: "um site" })).not.toBeInTheDocument();
    expect(within(painel).getByText("um site")).toHaveAttribute("title", "https://exemplo.com/x");
  });

  it("se o link não abre, mostra a mensagem com a URL para copiar, e não some em silêncio", async () => {
    vi.mocked(invoke).mockRejectedValue("o link é de evil.com, que não é o rastreador nem um repositório deste projeto");
    const copiar = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", { value: { writeText: copiar }, configurable: true });
    montar();
    const painel = await screen.findByRole("complementary", { name: "Demanda selecionada" });
    fireEvent.click(within(painel).getByRole("button", { name: /Abrir no GitHub/ }));
    const aviso = await within(painel).findByRole("alert");
    expect(aviso).toHaveTextContent("Não foi possível abrir o link no navegador.");
    expect(aviso).toHaveTextContent("o link é de evil.com");
    fireEvent.click(within(aviso).getByRole("button", { name: "Copiar a URL" }));
    expect(copiar).toHaveBeenCalledWith("https://github.com/o/r/issues/412");
  });

  it("clique do meio na linha e o botão do painel abrem numa aba; a issue com demanda abre a demanda", async () => {
    const { navegar } = montar();
    await screen.findByRole("list", { name: "Lista de demandas" });

    // #412 não tem demanda: abre a aba da issue, pelo clique do meio na linha.
    const lista = screen.getByRole("list", { name: "Lista de demandas" });
    const meio = new MouseEvent("auxclick", { bubbles: true, cancelable: true, button: 1 });
    fireEvent(within(lista).getByText("#412").closest("li")!, meio);
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "issue", projeto: PROJETO, origem: "github", chave: "#412" });

    // #398 tem a demanda D-0004: a aba é a dela, pelo botão do painel.
    fireEvent.click(within(lista).getByText("#398"));
    const painel = screen.getByRole("complementary", { name: "Demanda selecionada" });
    fireEvent.click(within(painel).getByRole("button", { name: "Abrir em nova aba" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "demanda", projeto: PROJETO, demanda: "D-0004" });
  });
});
