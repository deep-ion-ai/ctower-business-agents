import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { TelaAnalise } from "./TelaAnalise";
import { apiAnaliseFalsa, apiTerminalFalsa, telaFalsa } from "./test/falsos";
import { PROJETO, apiConhecimentoFalsa, painelFicticio } from "./test/conhecimentoFalso";
import { quandoMudou } from "./TelaAnalise";
import type { AnaliseResumo } from "./analise";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

function montar(analise = apiAnaliseFalsa()) {
  const terminal = apiTerminalFalsa();
  render(<TelaAnalise api={analise.api} apiTerminal={terminal.api} criarTela={telaFalsa} />);
  return { ...analise, terminal };
}

/** "Nova análise" → "Perguntar ao sistema": o formulário da pergunta, no painel lateral. */
async function abrirPergunta() {
  fireEvent.click(await screen.findByRole("button", { name: "Nova análise" }));
  const painel = await screen.findByRole("dialog", { name: "Nova análise" });
  fireEvent.click(within(painel).getByRole("button", { name: "Perguntar ao sistema" }));
}

/** A tela com os tipos além da pergunta (a API do conhecimento) e o painel aberto. */
async function abrirPainelCompleto(conhecimento = apiConhecimentoFalsa()) {
  const analise = apiAnaliseFalsa();
  const aoAbrirAnalise = vi.fn();
  const aoAbrirDemanda = vi.fn();
  render(
    <TelaAnalise
      api={analise.api}
      apiConhecimento={conhecimento.api}
      projetoInicial={PROJETO}
      aoAbrirAnalise={aoAbrirAnalise}
      aoAbrirDemanda={aoAbrirDemanda}
      novaInicial
    />,
  );
  const painel = await screen.findByRole("dialog", { name: "Nova análise" });
  return { analise, conhecimento, painel, aoAbrirAnalise, aoAbrirDemanda };
}

describe("TelaAnalise", () => {
  it("sem base ou sem projeto, leva ao primeiro uso", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.situacao.mockResolvedValueOnce({ base: "/base", projetos: [] });
    const aoPrimeiroUso = vi.fn();
    render(<TelaAnalise api={analise.api} aoPrimeiroUso={aoPrimeiroUso} />);
    expect(await screen.findByText(/Nenhum projeto cadastrado/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Abrir o primeiro uso" }));
    expect(aoPrimeiroUso).toHaveBeenCalled();
    expect(screen.queryByRole("form", { name: "Nova pergunta" })).not.toBeInTheDocument();
  });

  it("abre com o projeto e a pergunta sugeridos pelo primeiro uso", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.situacao.mockResolvedValueOnce({
      base: "/base",
      projetos: [
        { id: "estoque-exemplo", nome: "Estoque Exemplo" },
        { id: "faturamento-exemplo", nome: "Faturamento Exemplo" },
      ],
    });
    render(
      <TelaAnalise
        api={analise.api}
        projetoInicial="faturamento-exemplo"
        perguntaInicial="Quais são as principais regras de negócio deste projeto?"
      />,
    );
    expect(await screen.findByLabelText("Pergunta")).toHaveValue(
      "Quais são as principais regras de negócio deste projeto?",
    );
    // O projeto é o da barra de título: a tela só o mostra, sem outro seletor (#350).
    expect(screen.getByText(/Faturamento Exemplo, o projeto da barra de título/)).toBeInTheDocument();
    expect(screen.queryByLabelText(/Projeto/)).toBeNull();
  });

  it("só prepara o ambiente depois de mostrar o plano e do consentimento", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.verificarAmbiente
      .mockResolvedValueOnce({
        pronto: false,
        pendencias: ["o servidor MCP do app não está registrado para esta pasta"],
      })
      .mockResolvedValueOnce({ pronto: true, pendencias: [] });
    analise.api.planejarPreparo.mockResolvedValue({
      diretorio: "/codigo",
      gravado: false,
      alteracoes: [
        {
          arquivo: "/home/ana/.claude.json",
          item: { tipo: "servidorMcp", chave: "/codigo" },
          acao: "adicionado",
        },
        {
          arquivo: "/home/ana/.claude.json",
          item: { tipo: "confianca", chave: "/codigo" },
          acao: "adicionado",
        },
        {
          arquivo: "/home/ana/.claude/settings.json",
          item: { tipo: "hook", evento: "SessionStart" },
          acao: "adicionado",
        },
      ],
    });
    montar(analise);
    await abrirPergunta();
    expect(
      await screen.findByText("o servidor MCP do app não está registrado para esta pasta"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Perguntar" })).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "Ver e preparar" }));
    expect(await screen.findByText(/Servidor MCP "cba" para a pasta \/codigo/)).toBeInTheDocument();
    expect(screen.getByText("Hook SessionStart")).toBeInTheDocument();
    // A confiança na pasta entra no plano (#218).
    expect(
      screen.getByText(/Pasta \/codigo marcada como confiável no Claude Code/),
    ).toBeInTheDocument();
    expect(analise.api.prepararAmbiente).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Concordo, ligar o agente" }));
    await waitFor(() =>
      expect(analise.api.prepararAmbiente).toHaveBeenCalledWith("faturamento-exemplo"),
    );
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Preparar o ambiente" })).not.toBeInTheDocument(),
    );
    fireEvent.change(screen.getByLabelText("Pergunta"), { target: { value: "Qual a multa?" } });
    expect(screen.getByRole("button", { name: "Perguntar" })).toBeEnabled();
  });

  it("avisa que os avisos vão para outra instalação e pede concordância (#213)", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.verificarAmbiente
      .mockResolvedValueOnce({ pronto: false, pendencias: ["os hooks estão desatualizados"] })
      .mockResolvedValueOnce({ pronto: true, pendencias: [] });
    const dev = "/home/ana/projetos/cba/target/debug/cba";
    analise.api.planejarPreparo.mockResolvedValue({
      diretorio: "/codigo",
      gravado: false,
      alteracoes: ["SessionStart", "Stop"].map((evento) => ({
        arquivo: "/home/ana/.claude/settings.json",
        item: { tipo: "hook" as const, evento },
        acao: "atualizado" as const,
        outraInstalacao: dev,
      })),
    });
    montar(analise);
    fireEvent.click(await screen.findByRole("button", { name: "Ver e preparar" }));
    const aviso = await screen.findByRole("note");
    expect(aviso).toHaveTextContent("Hoje os avisos do Claude vão para outra instalação do app");
    expect(within(aviso).getAllByText(dev)).toHaveLength(1);
    expect(screen.getByText("Hook SessionStart").parentElement).toHaveTextContent(
      "hoje vai para outra instalação do app",
    );
    const concordo = screen.getByRole("button", { name: "Concordo, ligar o agente" });
    expect(concordo).toBeDisabled();
    fireEvent.click(within(aviso).getByRole("checkbox"));
    expect(concordo).toBeEnabled();
    fireEvent.click(concordo);
    await waitFor(() =>
      expect(analise.api.prepararAmbiente).toHaveBeenCalledWith("faturamento-exemplo"),
    );
  });

  it("cria a análise da pergunta e abre o terminal com o pedido de início", async () => {
    const { api, terminal } = montar();
    await abrirPergunta();
    fireEvent.change(await screen.findByLabelText("Pergunta"), {
      target: { value: "Como é calculada a multa por atraso de pagamento?" },
    });
    const perguntar = screen.getByRole("button", { name: "Perguntar" });
    await waitFor(() => expect(perguntar).toBeEnabled());
    fireEvent.click(perguntar);
    await waitFor(() =>
      expect(api.criarAnalise).toHaveBeenCalledWith(
        "faturamento-exemplo",
        "Como é calculada a multa por atraso de pagamento?",
        undefined,
      ),
    );
    await waitFor(() => expect(terminal.aberturas).toHaveLength(1));
    expect(terminal.aberturas[0].pedido).toEqual({
      promptInicial: "Inicie a análise A-0001 pelo ctower",
    });
    expect(await screen.findByRole("article", { name: "Documento da análise" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "← Análises" }));
    expect(await screen.findByRole("button", { name: "Nova análise" })).toBeInTheDocument();
  });

  it("mostra a recusa ao criar a análise", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.criarAnalise.mockRejectedValue(
      "a pasta da base de conhecimento não está acessível",
    );
    montar(analise);
    await abrirPergunta();
    fireEvent.change(await screen.findByLabelText("Pergunta"), { target: { value: "Qual?" } });
    const perguntar = screen.getByRole("button", { name: "Perguntar" });
    await waitFor(() => expect(perguntar).toBeEnabled());
    fireEvent.click(perguntar);
    expect(await screen.findByRole("alert")).toHaveTextContent("não está acessível");
  });

  it("lista e reabre as análises do projeto", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.listarAnalises.mockResolvedValue([
      {
        id: "A-0001",
        titulo: "Multa por atraso",
        fluxo: "perguntar",
        estado: "concluida",
        atualizadaMs: 1,
      },
    ]);
    const { terminal } = montar(analise);
    const linha = await screen.findByRole("listitem");
    expect(within(linha).getByText("Aguardando revisão")).toBeInTheDocument();
    // A análise que aguarda revisão tem "Revisar" como ação da linha.
    expect(screen.getByRole("button", { name: "Revisar A-0001" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Multa por atraso" }));
    // O documento à esquerda; a conversa à direita, sem o bloco "Como continuar".
    expect(await screen.findByRole("article", { name: "Documento da análise" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Escolher onde continuar" })).toBeInTheDocument();
    expect(screen.queryByText("Como continuar esta análise?")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continuar a conversa" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    expect(analise.api.carregarAnalise).toHaveBeenCalledWith("faturamento-exemplo", "A-0001");
    expect(terminal.api.abrir).not.toHaveBeenCalled();
  });

  it("filtra a lista por situação, tipo e busca, com o mesmo rótulo de estado", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.listarAnalises.mockResolvedValue([
      { id: "A-0001", titulo: "Multa por atraso", fluxo: "perguntar", estado: "concluida", atualizadaMs: 1 },
      { id: "A-0002", titulo: "Parcelamento", fluxo: "impacto", estado: "aprovada", atualizadaMs: 2 },
      { id: "A-0003", titulo: "Cancelamento", fluxo: "perguntar", estado: "em-andamento", atualizadaMs: 3 },
    ]);
    montar(analise);
    const lista = await screen.findByRole("region", { name: "Análises do projeto" });
    expect(within(lista).getAllByRole("listitem")).toHaveLength(3);
    // Cada situação mostra quantas há (#350).
    expect(within(lista).getByRole("button", { name: "Todas 3" })).toHaveAttribute("aria-pressed", "true");
    expect(within(lista).getByRole("button", { name: "Criadas 0" })).toBeInTheDocument();
    fireEvent.click(within(lista).getByRole("button", { name: "Aguardando revisão 1" }));
    expect(within(lista).getAllByRole("listitem")).toHaveLength(1);
    expect(within(lista).getByText("Multa por atraso")).toBeInTheDocument();
    fireEvent.click(within(lista).getByRole("button", { name: "Aguardando revisão 1" }));
    fireEvent.change(screen.getByLabelText("Tipo de análise"), { target: { value: "impacto" } });
    expect(within(lista).getAllByRole("listitem")).toHaveLength(1);
    expect(within(lista).getByText("Parcelamento")).toBeInTheDocument();
    // A contagem acompanha o tipo escolhido.
    expect(within(lista).getByRole("button", { name: "Aguardando revisão 0" })).toBeInTheDocument();
    fireEvent.click(within(lista).getByRole("button", { name: "Limpar filtros" }));
    fireEvent.change(screen.getByLabelText("Buscar análise"), { target: { value: "cancelamento" } });
    expect(within(lista).getAllByRole("listitem")).toHaveLength(1);
    fireEvent.change(screen.getByLabelText("Buscar análise"), { target: { value: "zzz" } });
    expect(within(lista).getByText("Nenhuma análise com este filtro.")).toBeInTheDocument();
  });

  it("abre já filtrada pela situação do link e com a escolha do tipo da ação global", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.listarAnalises.mockResolvedValue([
      { id: "A-0001", titulo: "Multa", fluxo: "perguntar", estado: "concluida", atualizadaMs: 1 },
      { id: "A-0002", titulo: "Parcelamento", fluxo: "perguntar", estado: "aprovada", atualizadaMs: 2 },
    ]);
    render(<TelaAnalise api={analise.api} situacaoInicial="concluida" novaInicial />);
    const lista = await screen.findByRole("region", { name: "Análises do projeto" });
    await waitFor(() => expect(within(lista).getAllByRole("listitem")).toHaveLength(1));
    expect(within(lista).getByRole("button", { name: "Aguardando revisão 1" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    const painel = screen.getByRole("dialog", { name: "Nova análise" });
    expect(within(painel).getByRole("group", { name: "Tipo de análise" })).toBeInTheDocument();
    // Esc fecha o painel; a lista continua.
    fireEvent.keyDown(painel, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Nova análise" })).toBeNull();
  });

  it("mostra a demanda de cada análise: a chave no rastreador e a demanda da base (#350)", async () => {
    const analise = apiAnaliseFalsa();
    analise.api.listarAnalises.mockResolvedValue([
      { id: "A-0003", titulo: "Ligada à issue", fluxo: "impacto", estado: "concluida", atualizadaMs: 3, chave: "PROJ-7", link: "https://rastreador.exemplo/PROJ-7" },
      { id: "A-0002", titulo: "Ligada à demanda", fluxo: "revisar", estado: "em-andamento", atualizadaMs: 2, demanda: "D-0001" },
      { id: "A-0001", titulo: "Solta", fluxo: "perguntar", estado: "aprovada", atualizadaMs: 1 },
    ]);
    const aoAbrirDemanda = vi.fn();
    render(
      <TelaAnalise
        api={analise.api}
        apiConhecimento={apiConhecimentoFalsa().api}
        projetoInicial={PROJETO}
        aoAbrirDemanda={aoAbrirDemanda}
      />,
    );
    const lista = await screen.findByRole("region", { name: "Análises do projeto" });
    const [issue, daBase, solta] = within(lista).getAllByRole("listitem");
    expect(within(issue).getByText("PROJ-7")).toHaveAttribute("title", "https://rastreador.exemplo/PROJ-7");
    // A chave da demanda da base vem da lista de demandas do projeto.
    expect(await within(daBase).findByText("FAT-12")).toBeInTheDocument();
    fireEvent.click(within(daBase).getByRole("button", { name: "Abrir a demanda D-0001" }));
    expect(aoAbrirDemanda).toHaveBeenCalledWith(PROJETO, "D-0001");
    expect(within(solta).getByText("sem demanda")).toBeInTheDocument();
    // A busca também acha pela chave.
    fireEvent.change(screen.getByLabelText("Buscar análise"), { target: { value: "proj-7" } });
    expect(within(lista).getAllByRole("listitem")).toHaveLength(1);
  });

  it("demanda cadastrada: exige a demanda, mostra o título e as análises que ela já tem (#350)", async () => {
    const conhecimento = apiConhecimentoFalsa();
    const emAndamento: AnaliseResumo = {
      id: "A-0003",
      titulo: "Parcelamento para pessoa jurídica",
      fluxo: "impacto",
      estado: "em-andamento",
      atualizadaMs: 9,
    };
    // O falso tipa a lista pelas análises que traz; esta tem outra situação.
    conhecimento.api.listarAnalises.mockResolvedValue([emAndamento] as never);
    const { painel, aoAbrirAnalise } = await abrirPainelCompleto(conhecimento);
    fireEvent.click(within(painel).getByRole("button", { name: "Analisar demanda cadastrada" }));
    const form = within(painel).getByRole("form", { name: "Analisar demanda cadastrada" });
    const iniciar = within(form).getByRole("button", { name: "Iniciar a análise" });
    expect(iniciar).toBeDisabled();
    expect(within(form).getByText("Para iniciar, informe a demanda ou a issue.")).toBeInTheDocument();
    const campo = within(form).getByLabelText(/Demanda ou issue/);
    expect(campo).toHaveFocus();
    // Texto que não é demanda nem issue não libera o botão.
    fireEvent.change(campo, { target: { value: "exp" } });
    expect(iniciar).toBeDisabled();
    expect(within(form).getByText(/escolha uma sugestão ou informe D-0012/)).toBeInTheDocument();

    fireEvent.change(campo, { target: { value: "D-0001" } });
    expect(await within(form).findByText("Parcelamento PJ")).toBeInTheDocument();
    const existentes = within(form).getByRole("list", { name: "Análises desta demanda" });
    expect(within(existentes).getByText("Parcelamento para pessoa jurídica")).toBeInTheDocument();
    expect(within(form).getByRole("button", { name: "Iniciar uma análise nova" })).toBeEnabled();
    fireEvent.click(within(existentes).getByRole("button", { name: "Continuar A-0003" }));
    expect(aoAbrirAnalise).toHaveBeenCalledWith(PROJETO, "A-0003", { tipo: "reabrir" });
  });

  it("a demanda da base mostra também as análises ligadas só pela chave da issue dela (#350)", async () => {
    const conhecimento = apiConhecimentoFalsa();
    const pelaChave: AnaliseResumo = {
      id: "A-0009",
      titulo: "Criada direto da issue",
      fluxo: "impacto",
      estado: "concluida",
      atualizadaMs: 9,
    };
    conhecimento.api.listarAnalises.mockResolvedValue([pelaChave] as never);
    // D-0001 tem a chave FAT-12; a issue FAT-12 tem a A-0009, ligada só pela chave.
    const listar = vi.fn(() =>
      Promise.resolve({
        issues: [
          {
            origem: "jira" as const,
            chave: "FAT-12",
            titulo: "Parcelamento PJ",
            descricao: "",
            url: "https://rastreador.exemplo/FAT-12",
            criadaEm: "2026-11-01T00:00:00Z",
            estado: "aberta",
            rotulos: [],
            situacao: "em-revisao" as const,
            analise: "A-0009",
            analises: ["A-0009"],
            demanda: null,
            funcionalidades: [],
          },
        ],
        total: 1,
        rotulos: [],
        lidaEm: "2026-11-24T00:00:00Z",
        atualizando: false,
        aviso: null,
        excedeuTeto: false,
        desde: "2026-01-01",
      }),
    );
    (conhecimento.api as { issues?: unknown }).issues = {
      listar,
      analisarImpacto: vi.fn(),
      aoAtualizar: vi.fn(() => Promise.resolve(() => undefined)),
    };
    const { painel } = await abrirPainelCompleto(conhecimento);
    fireEvent.click(within(painel).getByRole("button", { name: "Revisar requisito" }));
    const form = within(painel).getByRole("form", { name: "Revisar requisito" });
    fireEvent.change(within(form).getByLabelText(/Issue ou demanda/), { target: { value: "D-0001" } });
    const existentes = await within(form).findByRole("list", { name: "Análises desta demanda" });
    expect(within(existentes).getByText("Criada direto da issue")).toBeInTheDocument();
    expect(listar).toHaveBeenCalledWith(PROJETO, { busca: "FAT-12", estado: "todas" });
    // Fora da demanda cadastrada, o texto continua obrigatório com uma demanda da base.
    expect(within(form).getByRole("button", { name: "Iniciar uma análise nova" })).toBeDisabled();
  });

  it("demanda cadastrada: inicia a análise ligada à demanda, com o título dela", async () => {
    const { painel, conhecimento } = await abrirPainelCompleto();
    fireEvent.click(within(painel).getByRole("button", { name: "Analisar demanda cadastrada" }));
    const form = within(painel).getByRole("form", { name: "Analisar demanda cadastrada" });
    fireEvent.change(within(form).getByLabelText(/Demanda ou issue/), { target: { value: "D-0001" } });
    expect(await within(form).findByText("Esta demanda ainda não tem análise.")).toBeInTheDocument();
    fireEvent.click(within(form).getByRole("button", { name: "Iniciar a análise" }));
    await waitFor(() =>
      expect(conhecimento.api.criarAnalise).toHaveBeenCalledWith(
        PROJETO,
        "impacto",
        "Analisar a demanda D-0001: Parcelamento PJ.",
        "Parcelamento PJ",
        "D-0001",
      ),
    );
  });

  it("Demanda Negocial: registra a demanda com o tipo no campo próprio e oferece analisá-la (#350, #358)", async () => {
    const { painel, conhecimento } = await abrirPainelCompleto();
    fireEvent.click(within(painel).getByRole("button", { name: "Demanda Negocial" }));
    const form = within(painel).getByRole("form", { name: "Demanda Negocial" });
    const registrar = within(form).getByRole("button", { name: "Registrar demanda" });
    expect(registrar).toBeDisabled();
    expect(
      within(form).getByText("Para registrar, falta o tipo de demanda, o título e a descrição."),
    ).toBeInTheDocument();

    fireEvent.click(within(form).getByRole("radio", { name: /Corrigir bug/ }));
    fireEvent.change(within(form).getByLabelText("Título"), {
      target: { value: "Parcelas seguem cobradas" },
    });
    fireEvent.change(within(form).getByLabelText("O que acontece e o que deveria acontecer"), {
      target: { value: "Cancelar a fatura não cancela as parcelas." },
    });
    fireEvent.change(within(form).getByLabelText(/Funcionalidade afetada/), {
      target: { value: "Faturamento" },
    });
    expect(registrar).toBeEnabled();
    fireEvent.click(registrar);
    await waitFor(() =>
      expect(conhecimento.api.criarDemanda).toHaveBeenCalledWith(PROJETO, {
        titulo: "Parcelas seguem cobradas",
        // O tipo vai no frontmatter, não mais na linha `**Tipo:**` da descrição (#358).
        descricao: "**Funcionalidade:** Faturamento\n\nCancelar a fatura não cancela as parcelas.",
        chave: undefined,
        tipo: "bug",
      }),
    );
    expect(await within(painel).findByText("Demanda D-0003 registrada.")).toBeInTheDocument();
    fireEvent.click(within(painel).getByRole("button", { name: "Analisar esta demanda" }));
    expect(within(painel).getByLabelText(/Demanda ou issue/)).toHaveValue("D-0003");
  });

  it("Perguntar ao sistema: com a demanda, mostra o título e as análises dela e liga a pergunta (#359)", async () => {
    const conhecimento = apiConhecimentoFalsa();
    const emAndamento: AnaliseResumo = {
      id: "A-0003",
      titulo: "Parcelamento para pessoa jurídica",
      fluxo: "impacto",
      estado: "em-andamento",
      atualizadaMs: 9,
    };
    conhecimento.api.listarAnalises.mockResolvedValue([emAndamento] as never);
    const { painel, aoAbrirAnalise } = await abrirPainelCompleto(conhecimento);
    fireEvent.click(within(painel).getByRole("button", { name: "Perguntar ao sistema" }));
    const form = within(painel).getByRole("form", { name: "Nova pergunta" });
    // O foco continua na pergunta; a demanda é opcional.
    expect(within(form).getByLabelText("Pergunta")).toHaveFocus();
    expect(within(form).getByRole("button", { name: "Perguntar" })).toBeDisabled();
    fireEvent.change(within(form).getByLabelText("Pergunta"), {
      target: { value: "Como é calculada a parcela?" },
    });
    fireEvent.change(within(form).getByLabelText(/Issue ou demanda/), { target: { value: "D-0001" } });
    expect(await within(form).findByText("Parcelamento PJ")).toBeInTheDocument();
    const existentes = within(form).getByRole("list", { name: "Análises desta demanda" });
    expect(within(existentes).getByText("Parcelamento para pessoa jurídica")).toBeInTheDocument();
    fireEvent.click(within(existentes).getByRole("button", { name: "Continuar A-0003" }));
    expect(aoAbrirAnalise).toHaveBeenCalledWith(PROJETO, "A-0003", { tipo: "reabrir" });

    const iniciar = within(form).getByRole("button", { name: "Iniciar uma análise nova" });
    expect(iniciar).toBeEnabled();
    fireEvent.click(iniciar);
    await waitFor(() =>
      expect(conhecimento.api.criarAnalise).toHaveBeenCalledWith(
        PROJETO,
        "perguntar",
        "Como é calculada a parcela?",
        undefined,
        "D-0001",
      ),
    );
  });

  it("Perguntar ao sistema: com uma issue, a pergunta continua a entrada e a issue liga o resultado (#359)", async () => {
    const { painel, analise, conhecimento } = await abrirPainelCompleto();
    fireEvent.click(within(painel).getByRole("button", { name: "Perguntar ao sistema" }));
    const form = within(painel).getByRole("form", { name: "Nova pergunta" });
    fireEvent.change(within(form).getByLabelText("Pergunta"), { target: { value: "Quem aprova o parcelamento?" } });
    fireEvent.change(within(form).getByLabelText(/Issue ou demanda/), { target: { value: "PROJ-123" } });
    // Texto que não é demanda nem issue segura o botão.
    fireEvent.change(within(form).getByLabelText(/Issue ou demanda/), { target: { value: "exp" } });
    expect(within(form).getByRole("button", { name: "Perguntar" })).toBeDisabled();
    fireEvent.change(within(form).getByLabelText(/Issue ou demanda/), { target: { value: "PROJ-123" } });
    expect(await within(form).findByText(/Não achei PROJ-123/)).toBeInTheDocument();
    fireEvent.click(within(form).getByRole("button", { name: "Perguntar" }));
    await waitFor(() =>
      expect(conhecimento.api.criarAnaliseDeDemanda).toHaveBeenCalledWith(PROJETO, "perguntar", "PROJ-123", {
        entrada: "Quem aprova o parcelamento?",
        titulo: undefined,
      }),
    );
    expect(analise.api.criarAnalise).not.toHaveBeenCalled();
  });

  describe("Histórias e Cenários: a demanda da análise de origem e o que ela já tem (#359)", () => {
    /** D-0001 tem a análise de impacto (origem), uma de histórias e uma de cenários. */
    function comDemanda() {
      const conhecimento = apiConhecimentoFalsa();
      const analise = (
        id: string,
        titulo: string,
        fluxo: AnaliseResumo["fluxo"],
        estado: AnaliseResumo["estado"],
        atualizadaMs: number,
      ): AnaliseResumo => ({ id, titulo, fluxo, estado, atualizadaMs, demanda: "D-0001" });
      const lista = [
        analise("A-0008", "Cenários do parcelamento", "cenarios", "concluida", 8),
        analise("A-0007", "Histórias do parcelamento", "historias", "em-andamento", 7),
        analise("A-0003", "Parcelamento para pessoa jurídica", "impacto", "aprovada", 3),
      ];
      conhecimento.api.listarAnalises.mockResolvedValue(lista as never);
      const base = painelFicticio();
      const item = (a: AnaliseResumo) => ({
        tipo: "analise" as const,
        data: "2026-11-04",
        instanteMs: a.atualizadaMs,
        id: a.id,
        titulo: a.titulo,
        fluxo: a.fluxo,
        estado: a.estado,
        criadaMs: a.atualizadaMs,
      });
      conhecimento.api.painelDemanda.mockResolvedValue({ ...base, linhaDoTempo: lista.map(item) } as never);
      return conhecimento;
    }

    it("Histórias e critérios: mostra a demanda da origem e as histórias que ela já tem", async () => {
      const { painel, aoAbrirAnalise } = await abrirPainelCompleto(comDemanda());
      fireEvent.click(within(painel).getByRole("button", { name: "Histórias e critérios" }));
      const form = within(painel).getByRole("form", { name: "Histórias e critérios" });
      const grupo = await within(form).findByRole("group", { name: "Demanda da análise de origem" });
      expect(within(grupo).getByText("Parcelamento PJ")).toBeInTheDocument();
      expect(within(grupo).getByText(/já tem uma análise de histórias/)).toBeInTheDocument();
      const existentes = within(grupo).getByRole("list", { name: "Análises desta demanda" });
      expect(within(existentes).getAllByRole("listitem")).toHaveLength(1);
      expect(within(existentes).getByText("Histórias do parcelamento")).toBeInTheDocument();
      fireEvent.click(within(existentes).getByRole("button", { name: "Continuar A-0007" }));
      expect(aoAbrirAnalise).toHaveBeenCalledWith(PROJETO, "A-0007", { tipo: "reabrir" });
      expect(within(form).getByRole("button", { name: "Iniciar uma análise nova" })).toBeEnabled();
    });

    it("Cenários de teste: mostra a demanda da origem e os cenários que ela já tem", async () => {
      const { painel, aoAbrirAnalise } = await abrirPainelCompleto(comDemanda());
      fireEvent.click(within(painel).getByRole("button", { name: "Cenários de teste" }));
      const form = within(painel).getByRole("form", { name: "Cenários de teste" });
      const grupo = await within(form).findByRole("group", { name: "Demanda da análise de origem" });
      expect(within(grupo).getByText("Parcelamento PJ")).toBeInTheDocument();
      const existentes = within(grupo).getByRole("list", { name: "Análises desta demanda" });
      expect(within(existentes).getAllByRole("listitem")).toHaveLength(1);
      fireEvent.click(within(existentes).getByRole("button", { name: "Abrir A-0008" }));
      expect(aoAbrirAnalise).toHaveBeenCalledWith(PROJETO, "A-0008", { tipo: "reabrir" });
      expect(within(form).getByRole("button", { name: "Iniciar uma análise nova" })).toBeEnabled();
    });

    it("sem análises do fluxo na demanda, o botão continua o do fluxo", async () => {
      const conhecimento = comDemanda();
      conhecimento.api.painelDemanda.mockResolvedValue(painelFicticio() as never);
      const { painel } = await abrirPainelCompleto(conhecimento);
      fireEvent.click(within(painel).getByRole("button", { name: "Histórias e critérios" }));
      const form = within(painel).getByRole("form", { name: "Histórias e critérios" });
      expect(await within(form).findByText("Esta demanda ainda não tem histórias.")).toBeInTheDocument();
      expect(within(form).getByRole("button", { name: "Gerar as histórias" })).toBeEnabled();
    });
  });

  it("quandoMudou: minutos, horas, ontem e a data", () => {
    const agora = new Date(2026, 10, 24, 15, 0).getTime();
    expect(quandoMudou(agora - 20 * 60_000, agora)).toBe("há 20 min");
    expect(quandoMudou(agora - 2 * 3_600_000, agora)).toBe("há 2 h");
    expect(quandoMudou(new Date(2026, 10, 23, 10, 0).getTime(), agora)).toBe("ontem");
    expect(quandoMudou(new Date(2026, 10, 12, 10, 0).getTime(), agora)).toBe("12 de nov");
  });
});
