import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { Navegacao } from "./Navegacao";
import {
  PROJETO,
  REGRA,
  apiConhecimentoFalsa,
  inicioFicticio,
  resumoFicticio,
} from "./test/conhecimentoFalso";
import { MAPA } from "./test/mapaFalso";
import { apiConfiguracoesFalsa } from "./test/configuracoesFalso";
import { act } from "react";
import {
  apiAnaliseFalsa,
  apiTerminalFalsa,
  clientesProntos,
  telaFalsa,
  vistaFicticia,
} from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.reject(new Error("fora do Tauri"))),
  Channel: vi.fn(),
}));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));
vi.mock("./clientes", () => ({ estadoClientes: vi.fn(() => Promise.resolve(clientesProntos())) }));

const OUTRO = "emprestimos";

/** Conhecimento com dois projetos; o segundo sem pendências. */
function conhecimentoComDois() {
  const conhecimento = apiConhecimentoFalsa();
  conhecimento.api.situacao.mockResolvedValue({
    base: "/base",
    projetos: [
      { id: PROJETO, nome: "Faturamento Exemplo" },
      { id: OUTRO, nome: "Empréstimos" },
    ],
  });
  const base = inicioFicticio();
  conhecimento.api.inicio.mockResolvedValue(
    inicioFicticio({
      projetos: [
        ...base.projetos,
        { id: OUTRO, nome: "Empréstimos", analises: 0, regras: 0, aRevalidar: 0, termos: 0, erro: null },
      ],
    }),
  );
  return conhecimento;
}

const ANALISE_A1 = { id: "A-0001", titulo: "Multa por atraso", fluxo: "perguntar", estado: "concluida", atualizadaMs: 1 } as const;

function montar(conhecimento = apiConhecimentoFalsa()) {
  const analise = apiAnaliseFalsa();
  render(
    <Navegacao
      api={conhecimento.api}
      apiAnalise={analise.api}
      apiConfiguracoes={apiConfiguracoesFalsa()}
      apiMapa={{ mapa: () => Promise.resolve(MAPA) }}
    />,
  );
  return { conhecimento, analise };
}

/** Navegação aberta numa análise recém-criada, com o terminal embutido falso. */
function montarComAnalise(analise = apiAnaliseFalsa()) {
  const terminal = apiTerminalFalsa();
  render(
    <Navegacao
      api={apiConhecimentoFalsa().api}
      apiAnalise={analise.api}
      apiConfiguracoes={apiConfiguracoesFalsa()}
      apiMapa={{ mapa: () => Promise.resolve(MAPA) }}
      apiTerminal={terminal.api}
      criarTela={telaFalsa}
      inicial={{
        tipo: "analise",
        projeto: PROJETO,
        aberta: { id: "A-0001", abertura: { tipo: "nova", promptInicial: "Inicie a A-0001" } },
      }}
    />,
  );
  return { analise, terminal };
}

/** Conclui a abertura do terminal pedida em `indice`, com o identificador `id`. */
async function abrirTerminal(terminal: ReturnType<typeof apiTerminalFalsa>, indice: number, id: number) {
  await waitFor(() => expect(terminal.aberturas).toHaveLength(indice + 1));
  await act(async () => terminal.aberturas[indice].resolver(id));
}

const menu = () => screen.getByRole("navigation", { name: "Seções do projeto" });
/** O item do menu pelo rótulo (o nome acessível leva a contagem junto). */
const item = (rotulo: string) => within(menu()).getByRole("button", { name: new RegExp(`^${rotulo}`) });

/** Da seção Base: abre a regra ao lado da lista e depois "Abrir em aba". */
async function abrirRegraEmAba() {
  fireEvent.click(item("Base"));
  const resultados = await screen.findByRole("region", { name: "Resultados" });
  fireEvent.click(await within(resultados).findByRole("button", { name: /Passada a carência/ }));
  await screen.findByRole("article", { name: /Passada a carência/ });
  fireEvent.click(screen.getByRole("button", { name: "Abrir em aba" }));
  await screen.findByRole("tab", { name: "RN-COB-001" });
}

describe("Navegacao", () => {
  afterEach(async () => {
    localStorage.clear();
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockImplementation(() => Promise.reject(new Error("fora do Tauri")));
  });

  it("abre na Visão geral do último projeto usado, sem abas", async () => {
    localStorage.setItem("cba.projeto.ultimo", OUTRO);
    const { conhecimento } = montar(conhecimentoComDois());
    await waitFor(() => expect(conhecimento.api.resumoProjeto).toHaveBeenCalledWith(OUTRO));
    expect(item("Visão geral")).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: /Projeto: Empréstimos/ })).toBeInTheDocument();
    expect(screen.queryByRole("tablist", { name: "Abas abertas" })).not.toBeInTheDocument();
  });

  it("sem projeto lembrado (ou que sumiu), abre o primeiro e lembra a escolha", async () => {
    localStorage.setItem("cba.projeto.ultimo", "descadastrado");
    const { conhecimento } = montar(conhecimentoComDois());
    await waitFor(() => expect(conhecimento.api.resumoProjeto).toHaveBeenCalledWith(PROJETO));
    expect(conhecimento.api.resumoProjeto).not.toHaveBeenCalledWith("descadastrado");
    expect(localStorage.getItem("cba.projeto.ultimo")).toBe(PROJETO);
  });

  it("trocar de projeto troca o conteúdo de todas as seções", async () => {
    const { conhecimento } = montar(conhecimentoComDois());
    await waitFor(() => expect(conhecimento.api.resumoProjeto).toHaveBeenCalledWith(PROJETO));

    fireEvent.click(item("Demandas"));
    await waitFor(() => expect(conhecimento.api.listarDemandas).toHaveBeenCalledWith(PROJETO));

    fireEvent.click(screen.getByRole("button", { name: /Projeto: Faturamento Exemplo/ }));
    fireEvent.click(within(screen.getByRole("listbox", { name: "Projetos" })).getByRole("option", { name: /Empréstimos/ }));
    // A seção à vista passa a mostrar o projeto escolhido.
    await waitFor(() => expect(conhecimento.api.listarDemandas).toHaveBeenCalledWith(OUTRO));
    expect(item("Demandas")).toHaveAttribute("aria-current", "page");
    expect(localStorage.getItem("cba.projeto.ultimo")).toBe(OUTRO);

    // As outras seções também.
    fireEvent.click(item("Visão geral"));
    await waitFor(() => expect(conhecimento.api.resumoProjeto).toHaveBeenCalledWith(OUTRO));
    fireEvent.click(item("Base"));
    await waitFor(() =>
      expect(conhecimento.api.buscar).toHaveBeenLastCalledWith("", expect.objectContaining({ projeto: OUTRO })),
    );
    fireEvent.click(item("Código"));
    await waitFor(() => expect(conhecimento.api.mapa).toHaveBeenCalledWith(OUTRO, ""));
  });

  it("o seletor mostra as pendências de cada projeto e funciona pelo teclado", async () => {
    const { conhecimento } = montar(conhecimentoComDois());
    await waitFor(() => expect(conhecimento.api.inicio).toHaveBeenCalled());
    const botao = await screen.findByRole("button", { name: /Projeto: Faturamento Exemplo/ });
    fireEvent.click(botao);
    const lista = screen.getByRole("listbox", { name: "Projetos" });
    const [faturamento, emprestimos] = within(lista).getAllByRole("option");
    // A-0002 espera o analista e há regra a revalidar: duas pendências.
    expect(faturamento).toHaveTextContent("2 pendências");
    expect(faturamento).toHaveAttribute("aria-selected", "true");
    expect(emprestimos).toHaveTextContent("em dia");
    expect(faturamento).toHaveFocus();

    // Esc fecha e devolve o foco ao botão.
    fireEvent.keyDown(lista, { key: "Escape" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(botao).toHaveFocus();

    // Seta para baixo abre; outra seta anda; Enter escolhe.
    fireEvent.keyDown(botao, { key: "ArrowDown" });
    fireEvent.keyDown(screen.getByRole("listbox"), { key: "ArrowDown" });
    expect(within(screen.getByRole("listbox")).getByRole("option", { name: /Empréstimos/ })).toHaveFocus();
    fireEvent.keyDown(screen.getByRole("listbox"), { key: "Enter" });
    expect(await screen.findByRole("button", { name: /Projeto: Empréstimos/ })).toBeInTheDocument();
    await waitFor(() => expect(conhecimento.api.resumoProjeto).toHaveBeenCalledWith(OUTRO));
  });

  it("o menu mostra rótulos e contagens e é navegável por teclado", async () => {
    const { conhecimento } = montar();
    await waitFor(() => expect(conhecimento.api.inicio).toHaveBeenCalled());
    expect(within(menu()).getByRole("group", { name: "Projeto" })).toHaveTextContent(
      /Visão geral.*Atividades.*Análises.*Demandas/,
    );
    expect(within(menu()).getByRole("group", { name: "Conhecimento" })).toHaveTextContent(
      /Base.*Funcionalidades.*Código/,
    );
    // Atividades: a A-0002 espera o analista e há regra a revalidar.
    await waitFor(() => expect(item("Atividades")).toHaveTextContent("Atividades2"));
    expect(item("Atividades").querySelector(".menu-item-contagem")).toHaveAttribute(
      "data-tone",
      "attention",
    );
    expect(item("Análises")).toHaveTextContent("Análises2");
    expect(item("Configurações")).toBeInTheDocument();

    item("Visão geral").focus();
    fireEvent.keyDown(item("Visão geral"), { key: "ArrowDown" });
    expect(item("Atividades")).toHaveFocus();
    fireEvent.keyDown(item("Atividades"), { key: "End" });
    expect(item("Configurações")).toHaveFocus();
    fireEvent.keyDown(item("Configurações"), { key: "ArrowDown" });
    expect(item("Visão geral")).toHaveFocus();
    fireEvent.keyDown(item("Visão geral"), { key: "ArrowUp" });
    expect(item("Configurações")).toHaveFocus();
  });

  it("o selo de Atividades se atualiza ao vivo pelo diário de eventos (#321)", async () => {
    const conhecimento = apiConhecimentoFalsa();
    conhecimento.api.resumoProjeto.mockResolvedValue(resumoFicticio({ atividades: [] }));
    const { analise } = montar(conhecimento);
    await waitFor(() => expect(conhecimento.api.resumoProjeto).toHaveBeenCalledWith(PROJETO));
    await waitFor(() => expect(item("Atividades")).toHaveTextContent(/^Atividades$/));
    expect(analise.api.vigiar).toHaveBeenCalledWith(PROJETO, expect.any(Function));

    // A sessão da A-0002 passou a esperar o analista: o aviso da vigia relê o resumo.
    conhecimento.api.resumoProjeto.mockResolvedValue(resumoFicticio());
    await act(async () =>
      analise.avisar({ tipo: "mudanca", analise: "A-0002", mudanca: "sessao", versao: 3 }),
    );
    await waitFor(() => expect(item("Atividades")).toHaveTextContent("Atividades2"));

    // A base editada fora do app também relê.
    conhecimento.api.resumoProjeto.mockResolvedValue(resumoFicticio({ atividades: [] }));
    await act(async () => conhecimento.avisar());
    await waitFor(() => expect(item("Atividades")).toHaveTextContent(/^Atividades$/));
  });

  it("o selo soma o ambiente a preparar e deixa de fora as demandas novas, ocultas até a #322", async () => {
    const conhecimento = apiConhecimentoFalsa();
    const resumo = resumoFicticio();
    conhecimento.api.resumoProjeto.mockResolvedValue(
      resumoFicticio({
        atividades: [
          ...resumo.atividades,
          { ...resumo.atividades[1], tipo: "demanda-sem-analise", regras: null, demanda: "D-0001", titulo: "Pix" },
        ],
      }),
    );
    conhecimento.api.verificarAmbiente.mockResolvedValue({ pronto: false, pendencias: ["servidor MCP"] });
    montar(conhecimento);
    // Responder (A-0002), revalidar e o ambiente; a demanda nova não conta.
    await waitFor(() => expect(item("Atividades")).toHaveTextContent("Atividades3"));
  });

  it("“N sessões aguardam você”, na barra de status, leva às Atividades (#321)", async () => {
    montar();
    await screen.findByRole("region", { name: "Visão geral do projeto" });
    const barra = screen.getByRole("contentinfo", { name: "Barra de status" });
    fireEvent.click(await within(barra).findByRole("button", { name: "1 sessão aguarda você" }));
    expect(await screen.findByRole("region", { name: "Atividades" })).toBeInTheDocument();
    expect(item("Atividades")).toHaveAttribute("aria-current", "page");
    const grupo = await screen.findByRole("region", { name: "O agente aguarda você" });
    expect(within(grupo).getByText("Parcelamento para pessoa jurídica")).toBeInTheDocument();
  });

  it("a ação de um item das Atividades abre a análise numa aba", async () => {
    const { analise } = montar();
    await screen.findByRole("region", { name: "Visão geral do projeto" });
    fireEvent.click(item("Atividades"));
    const grupo = await screen.findByRole("region", { name: "O agente aguarda você" });
    fireEvent.click(within(grupo).getByRole("button", { name: "Responder ao agente: A-0002" }));
    await waitFor(() => expect(analise.api.carregarAnalise).toHaveBeenCalledWith(PROJETO, "A-0002"));
    expect(screen.getByRole("tab", { name: "A-0002" })).toHaveAttribute("aria-selected", "true");
  });

  it("\"Nova análise\" fica só na seção Análises e abre o painel com a escolha do tipo (#350)", async () => {
    montar();
    await screen.findByRole("region", { name: "Visão geral do projeto" });
    expect(within(menu()).queryByRole("button", { name: "Nova análise" })).toBeNull();
    fireEvent.click(item("Análises"));
    const secao = await screen.findByRole("region", { name: "Análises" });
    fireEvent.click(await within(secao).findByRole("button", { name: "Nova análise" }));
    const painel = await screen.findByRole("dialog", { name: "Nova análise" });
    expect(within(painel).getByRole("group", { name: "Tipo de análise" })).toBeInTheDocument();
    expect(item("Análises")).toHaveAttribute("aria-current", "page");
  });

  it("passear pelas seções não abre abas; abrir análise, documento ou demanda abre", async () => {
    const { analise } = montar();
    analise.api.listarAnalises.mockResolvedValue([ANALISE_A1]);
    expect(await screen.findByRole("region", { name: "Visão geral do projeto" })).toBeInTheDocument();

    fireEvent.click(item("Atividades"));
    expect(await screen.findByRole("region", { name: "Início" })).toBeInTheDocument();
    expect(item("Atividades")).toHaveAttribute("aria-current", "page");
    fireEvent.click(item("Análises"));
    expect(await screen.findByRole("region", { name: "Análises" })).toBeInTheDocument();
    fireEvent.click(item("Demandas"));
    expect(await screen.findByRole("region", { name: "Demandas" })).toBeInTheDocument();
    fireEvent.click(item("Base"));
    expect(screen.getByRole("search", { name: "Busca na base" })).toBeInTheDocument();
    fireEvent.click(item("Código"));
    expect(await screen.findByRole("region", { name: "Mapa do projeto" })).toBeInTheDocument();
    fireEvent.click(item("Configurações"));
    expect(screen.queryByRole("tablist", { name: "Abas abertas" })).not.toBeInTheDocument();

    // A análise abre numa aba; o menu deixa de marcar a seção.
    fireEvent.click(item("Análises"));
    const recentes = await screen.findByRole("region", { name: "Análises do projeto" });
    fireEvent.click(await within(recentes).findByRole("button", { name: "Multa por atraso" }));
    expect(await screen.findByRole("region", { name: "Análise" })).toBeInTheDocument();
    await waitFor(() => expect(analise.api.carregarAnalise).toHaveBeenCalledWith(PROJETO, "A-0001"));
    const abas = screen.getByRole("tablist", { name: "Abas abertas" });
    expect(within(abas).getByRole("tab", { name: "A-0001" })).toHaveAttribute("aria-selected", "true");
    expect(item("Atividades")).not.toHaveAttribute("aria-current");

    // A demanda também.
    fireEvent.click(item("Demandas"));
    fireEvent.click(await screen.findByRole("button", { name: /^D-0001:/ }));
    expect(await within(abas).findByRole("tab", { name: "D-0001" })).toHaveAttribute("aria-selected", "true");

    // Voltar à seção pelo menu não fecha as abas.
    fireEvent.click(item("Demandas"));
    expect(await screen.findByRole("region", { name: "Demandas" })).toBeInTheDocument();
    expect(within(abas).getAllByRole("tab")).toHaveLength(2);
  });

  it("da Visão geral para a análise, numa aba", async () => {
    const { analise } = montar();
    const passos = await screen.findByRole("region", { name: "Próximos passos" });
    fireEvent.click(await within(passos).findByRole("button", { name: "Responder ao agente" }));
    expect(await screen.findByRole("region", { name: "Análise" })).toBeInTheDocument();
    await waitFor(() => expect(analise.api.carregarAnalise).toHaveBeenCalledWith(PROJETO, "A-0002"));
    expect(screen.getByRole("tab", { name: "A-0002" })).toHaveAttribute("aria-selected", "true");

    // Fechar a única aba volta à seção.
    fireEvent.click(screen.getByRole("button", { name: "Fechar A-0002" }));
    expect(await screen.findByRole("region", { name: "Visão geral do projeto" })).toBeInTheDocument();
    expect(item("Visão geral")).toHaveAttribute("aria-current", "page");
  });

  it("a Visão geral não mostra mais as regras nem o glossário (#328)", async () => {
    montar();
    await screen.findByRole("region", { name: "Visão geral do projeto" });
    expect(screen.queryByRole("region", { name: "Regras" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Glossário" })).not.toBeInTheDocument();
  });

  it("da Base para o documento, numa aba", async () => {
    const { conhecimento } = montar();
    await abrirRegraEmAba();
    expect(conhecimento.api.abrirDocumento).toHaveBeenCalledWith(REGRA);
    expect(screen.getByRole("tab", { name: "RN-COB-001" })).toHaveAttribute("aria-selected", "true");

    // Fechar a única aba volta à seção de onde ela veio: a Base.
    fireEvent.click(screen.getByRole("button", { name: "Fechar RN-COB-001" }));
    expect(await screen.findByRole("search", { name: "Busca na base" })).toBeInTheDocument();
    expect(item("Base")).toHaveAttribute("aria-current", "page");
  });

  it("os números da Visão geral levam à seção correspondente", async () => {
    montar();
    await screen.findByRole("region", { name: "Próximos passos" });
    fireEvent.click(await screen.findByRole("button", { name: /^Regras na base/ }));
    expect(screen.getByRole("search", { name: "Busca na base" })).toBeInTheDocument();
    expect(item("Base")).toHaveAttribute("aria-current", "page");
  });

  it("a análise continua com o agente vivo ao ir a outra seção e voltar (#212)", async () => {
    const falsa = apiAnaliseFalsa();
    falsa.api.listarAnalises.mockResolvedValue([ANALISE_A1]);
    const { terminal } = montarComAnalise(falsa);
    await abrirTerminal(terminal, 0, 7);
    expect(terminal.aberturas[0].pedido).toEqual({ promptInicial: "Inicie a A-0001" });

    fireEvent.click(item("Base"));
    expect(screen.getByRole("search", { name: "Busca na base" })).toBeInTheDocument();
    // Fora da vez, a análise fica escondida, sem desmontar o terminal.
    expect(screen.queryByRole("region", { name: "Análise" })).not.toBeInTheDocument();
    fireEvent.click(item("Atividades"));
    expect(await screen.findByRole("region", { name: "Início" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "A-0001" }));
    expect(await screen.findByRole("region", { name: "Análise" })).toBeInTheDocument();
    // Abrir de novo pela lista de Análises só foca a aba: nada de retomada.
    fireEvent.click(item("Análises"));
    const recentes = await screen.findByRole("region", { name: "Análises do projeto" });
    fireEvent.click(await within(recentes).findByRole("button", { name: "Multa por atraso" }));
    expect(await screen.findByRole("region", { name: "Análise" })).toBeInTheDocument();
    expect(terminal.api.abrir).toHaveBeenCalledTimes(1);
    expect(terminal.api.encerrar).not.toHaveBeenCalled();
  });

  it("duas análises têm agente ao mesmo tempo, cada uma no seu terminal (#212)", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.criarAnalise.mockResolvedValueOnce({
      analise: {
        id: "A-0002",
        titulo: "Juros de mora",
        fluxo: "perguntar",
        estado: "criada",
        atualizadaMs: 2,
      },
      promptInicial: "Inicie a A-0002",
    });
    const { terminal } = montarComAnalise(analise);
    await abrirTerminal(terminal, 0, 1);

    // "← Análises" leva à seção sem fechar a análise; a nova abre numa aba própria.
    fireEvent.click(await screen.findByRole("button", { name: "← Análises" }));
    expect(await screen.findByRole("region", { name: "Análises" })).toBeInTheDocument();
    expect(item("Análises")).toHaveAttribute("aria-current", "page");
    fireEvent.click(
      within(await screen.findByRole("region", { name: "Análises" })).getByRole("button", { name: "Nova análise" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Perguntar ao sistema" }));
    fireEvent.change(await screen.findByRole("textbox", { name: "Pergunta" }), {
      target: { value: "Como são os juros de mora?" },
    });
    await waitFor(() => expect(screen.getByRole("button", { name: "Perguntar" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Perguntar" }));
    await abrirTerminal(terminal, 1, 2);

    expect(terminal.aberturas[1].pedido).toEqual({ promptInicial: "Inicie a A-0002" });
    expect(screen.getByRole("tab", { name: "A-0002" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "A-0001" })).toBeInTheDocument();
    expect(terminal.api.encerrar).not.toHaveBeenCalled();

    // A saída de cada agente vai para o seu terminal, mesmo com a aba escondida.
    act(() => terminal.aberturas[0].aoEvento({ tipo: "saida", texto: "da A-0001" }));
    fireEvent.click(screen.getByRole("tab", { name: "A-0001" }));
    expect(await screen.findByRole("region", { name: "Análise" })).toHaveAccessibleName("Análise");
    expect(terminal.api.abrir).toHaveBeenCalledTimes(2);
    expect(terminal.api.encerrar).not.toHaveBeenCalled();
  });

  it("fechar a aba com o agente trabalhando pede confirmação (#212)", async () => {
    const { terminal } = montarComAnalise(
      apiAnaliseFalsa(
        vistaFicticia({ sessao: { id: "sessao-1", estado: "ativa", cliente: "claude-code" } }),
      ),
    );
    await abrirTerminal(terminal, 0, 3);
    await screen.findByText("Trabalhando na análise");

    fireEvent.click(screen.getByRole("button", { name: "Fechar A-0001" }));
    let dialogo = within(await screen.findByRole("dialog", { name: "Fechar a análise" }));
    expect(
      dialogo.getByText("O agente ainda está trabalhando. Encerrar a conversa?"),
    ).toBeInTheDocument();
    fireEvent.click(dialogo.getByRole("button", { name: "Manter a conversa" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "A-0001" })).toBeInTheDocument();
    expect(terminal.api.encerrar).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Fechar A-0001" }));
    dialogo = within(await screen.findByRole("dialog", { name: "Fechar a análise" }));
    fireEvent.click(dialogo.getByRole("button", { name: "Encerrar a conversa" }));
    expect(screen.queryByRole("tab", { name: "A-0001" })).not.toBeInTheDocument();
    await waitFor(() => expect(terminal.api.encerrar).toHaveBeenCalledWith(3));
  });

  it("fechar a aba de uma análise sem conversa viva não pergunta", async () => {
    const { terminal } = montarComAnalise();
    await abrirTerminal(terminal, 0, 4);
    act(() => terminal.aberturas[0].aoEvento({ tipo: "encerrado", fim: { codigo: 0 } }));
    expect((await screen.findAllByText(/A sessão terminou/)).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: "Fechar A-0001" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "A-0001" })).not.toBeInTheDocument();
  });

  it("“Fechar outras” fecha as abas sem conversa e pergunta antes das análises com conversa viva (#227)", async () => {
    const { terminal } = montarComAnalise();
    await abrirTerminal(terminal, 0, 5);
    fireEvent.click(item("Visão geral"));
    const passos = await screen.findByRole("region", { name: "Próximos passos" });
    fireEvent.click(await within(passos).findByRole("button", { name: "Responder ao agente" }));
    await screen.findByRole("tab", { name: "A-0002" });
    fireEvent.click(item("Demandas"));
    fireEvent.click(await screen.findByRole("button", { name: /^D-0001:/ }));
    await screen.findByRole("tab", { name: "D-0001" });
    expect(screen.getAllByRole("tab")).toHaveLength(3);

    fireEvent.click(screen.getByRole("button", { name: "Fechar outras" }));
    const dialogo = within(await screen.findByRole("dialog", { name: "Fechar as outras abas" }));
    expect(dialogo.getByText(/Uma das outras abas tem uma conversa/)).toBeInTheDocument();
    // Manter as conversas: só as abas sem conversa fecham.
    fireEvent.click(dialogo.getByRole("button", { name: "Manter as conversas" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getAllByRole("tab").map((t) => t.textContent)).toEqual(["A-0001", "D-0001"]);
    expect(screen.getByRole("tab", { name: "D-0001" })).toHaveAttribute("aria-selected", "true");
    expect(terminal.api.encerrar).not.toHaveBeenCalled();

    // Encerrar as conversas: fecha também a análise e encerra o agente dela.
    fireEvent.click(screen.getByRole("button", { name: "Fechar outras" }));
    fireEvent.click(
      within(await screen.findByRole("dialog")).getByRole("button", { name: "Encerrar as conversas" }),
    );
    expect(screen.getAllByRole("tab").map((t) => t.textContent)).toEqual(["D-0001"]);
    await waitFor(() => expect(terminal.api.encerrar).toHaveBeenCalledWith(5));
    expect(screen.queryByRole("button", { name: "Fechar outras" })).not.toBeInTheDocument();
  });

  it("a sessão de mapeamento continua viva ao passear pelas seções (#257)", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockImplementation(((comando: string) => {
      if (comando === "mapa_funcionalidades_projeto") return Promise.resolve(MAPA);
      if (comando === "pedido_de_mapeamento") return Promise.resolve("Mapeie o projeto");
      return Promise.reject(new Error("fora do Tauri"));
    }) as never);
    const terminal = apiTerminalFalsa();
    render(
      <Navegacao
        api={apiConhecimentoFalsa().api}
        apiAnalise={apiAnaliseFalsa().api}
        apiTerminal={terminal.api}
        criarTela={telaFalsa}
        inicial={{ tipo: "mapa", projeto: PROJETO }}
      />,
    );
    // Funcionalidades é seção: não abre aba.
    expect(item("Funcionalidades")).toHaveAttribute("aria-current", "page");
    expect(screen.queryByRole("tablist", { name: "Abas abertas" })).not.toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Mapear com o Claude" }));
    fireEvent.click(screen.getByRole("button", { name: "Começar" }));
    await abrirTerminal(terminal, 0, 9);

    fireEvent.click(item("Base"));
    expect(screen.getByRole("search", { name: "Busca na base" })).toBeInTheDocument();
    fireEvent.click(item("Funcionalidades"));
    expect(item("Funcionalidades")).toHaveAttribute("aria-current", "page");
    expect(terminal.api.abrir).toHaveBeenCalledTimes(1);
    expect(terminal.api.encerrar).not.toHaveBeenCalled();
  });

  it("Configurações ficam com o app; “Configurar projeto” abre do cabeçalho da Visão geral", async () => {
    const conhecimento = apiConhecimentoFalsa();
    const configuracoes = apiConfiguracoesFalsa();
    // A escolha antiga de um projeto nas configurações não vale mais lá.
    localStorage.setItem("cba.configuracoes.secao", "projeto:faturamento-exemplo");
    render(
      <Navegacao
        api={conhecimento.api}
        apiAnalise={apiAnaliseFalsa().api}
        apiConfiguracoes={configuracoes}
      />,
    );
    await screen.findByRole("region", { name: "Visão geral do projeto" });
    fireEvent.click(item("Configurações"));
    const cfg = await screen.findByRole("region", { name: "Configurações" });
    const projetos = await within(cfg).findByRole("group", { name: "Projetos" });
    expect(within(projetos).queryByRole("button", { name: /Faturamento Exemplo/ })).not.toBeInTheDocument();
    expect(within(projetos).getByRole("button", { name: "+ Novo projeto" })).toBeInTheDocument();
    expect(within(cfg).getByRole("region", { name: "Aparência" })).toBeInTheDocument();

    fireEvent.click(item("Visão geral"));
    fireEvent.click(await screen.findByRole("button", { name: "Configurar projeto" }));
    const pagina = await screen.findByRole("region", { name: "Configurar projeto" });
    expect(within(pagina).getByRole("tab", { name: /Saúde do cadastro/ })).toBeInTheDocument();
    expect(item("Visão geral")).toHaveAttribute("aria-current", "page");

    // Descadastrar relê os projetos.
    const lidas = conhecimento.api.situacao.mock.calls.length;
    fireEvent.click(within(pagina).getByRole("tab", { name: "Avançado" }));
    fireEvent.click(screen.getByRole("button", { name: "Descadastrar…" }));
    fireEvent.change(screen.getByLabelText(/digite o nome do projeto/), {
      target: { value: "Faturamento Exemplo" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Descadastrar o projeto" }));
    await waitFor(() => expect(conhecimento.api.situacao.mock.calls.length).toBeGreaterThan(lidas));
  });

  it("pede confirmação ao trocar de seção com alterações não salvas no editor de textos", async () => {
    localStorage.setItem("cba.configuracoes.secao", "modelos");
    render(
      <Navegacao
        api={apiConhecimentoFalsa().api}
        apiAnalise={apiAnaliseFalsa().api}
        apiConfiguracoes={apiConfiguracoesFalsa()}
      />,
    );
    await screen.findByRole("region", { name: "Visão geral do projeto" });
    fireEvent.click(item("Configurações"));
    fireEvent.click(await screen.findByRole("button", { name: "Editar Perguntar ao sistema" }));
    const campo = await screen.findByRole("textbox", { name: "Texto" });

    // Sem alterações, trocar de seção não pergunta.
    fireEvent.click(item("Atividades"));
    expect(await screen.findByRole("region", { name: "Início" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(item("Configurações"));
    // A seção abre do zero: o editor fechou junto.
    fireEvent.click(await screen.findByRole("button", { name: "Editar Perguntar ao sistema" }));
    const campoDeNovo = await screen.findByRole("textbox", { name: "Texto" });
    expect(campoDeNovo).not.toBe(campo);

    fireEvent.change(campoDeNovo, { target: { value: "rascunho" } });
    fireEvent.click(item("Atividades"));
    let dialogo = within(await screen.findByRole("dialog", { name: "Alterações não salvas" }));
    fireEvent.click(dialogo.getByRole("button", { name: "Continuar editando" }));
    expect(screen.getByRole("textbox", { name: "Texto" })).toHaveValue("rascunho");

    // Trocar de projeto também pergunta.
    fireEvent.click(screen.getByRole("button", { name: /Projeto: Faturamento Exemplo/ }));
    fireEvent.click(screen.getByRole("option", { name: /Faturamento Exemplo/ }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    fireEvent.click(item("Atividades"));
    dialogo = within(await screen.findByRole("dialog", { name: "Alterações não salvas" }));
    fireEvent.click(dialogo.getByRole("button", { name: "Descartar e sair" }));
    expect(await screen.findByRole("region", { name: "Início" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("o botão da barra de título mostra e esconde o menu lateral", async () => {
    montar();
    await screen.findByRole("region", { name: "Visão geral do projeto" });
    const alternar = screen.getByRole("button", { name: "Fechar menu lateral" });
    expect(alternar).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(alternar);
    expect(screen.queryByRole("navigation", { name: "Seções do projeto" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Abrir menu lateral" }));
    expect(menu()).toBeInTheDocument();
  });

  it("em tela estreita, o menu é gaveta: começa fechado e fecha ao escolher", async () => {
    vi.stubGlobal(
      "matchMedia",
      vi.fn((q: string) => ({
        matches: q.includes("max-width: 900px"),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );
    try {
      montar();
      await screen.findByRole("region", { name: "Visão geral do projeto" });
      expect(screen.queryByRole("navigation", { name: "Seções do projeto" })).not.toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "Abrir menu lateral" }));
      fireEvent.click(item("Atividades"));
      expect(await screen.findByRole("region", { name: "Início" })).toBeInTheDocument();
      expect(screen.queryByRole("navigation", { name: "Seções do projeto" })).not.toBeInTheDocument();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("troca o tema pela barra de título", async () => {
    montar();
    await screen.findByRole("region", { name: "Visão geral do projeto" });
    const tema = screen.getByRole("group", { name: "Tema" });
    fireEvent.click(within(tema).getByRole("button", { name: "Tema claro" }));
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(within(tema).getByRole("button", { name: "Tema claro" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.click(within(tema).getByRole("button", { name: "Tema escuro" }));
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
