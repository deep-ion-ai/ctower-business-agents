import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { RegistrosDemanda } from "./RegistrosDemanda";
import {
  textoCompleto,
  type ApiRegistros,
  type Proposta,
  type RegistroVista,
  type RegistrosDaDemanda,
} from "./registros";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const PROJETO = "faturamento-exemplo";
const PASTA = "faturamento-exemplo/demandas/D-0001-parcelamento-pj/registros";

function registro(r: Partial<RegistroVista> & Pick<RegistroVista, "id" | "categoria" | "titulo">) {
  return {
    detalhe: null,
    quem: null,
    pessoa: null,
    origem: null,
    referencia: null,
    criado: "2026-11-10",
    status: null,
    validacao: null,
    decididoEm: null,
    alternativas: [],
    probabilidade: null,
    impacto: null,
    resposta: null,
    restricao: null,
    historico: [],
    documento: `${PASTA}/${r.id}.md`,
    ...r,
  } satisfies RegistroVista;
}

function propostaDeRisco(n: number, titulo = `Risco proposto ${n}.`): Proposta {
  return {
    referencia: `A-0001/${n}`,
    categoria: "risco",
    fonte: { tipo: "analise", id: "A-0001", titulo: "Parcelamento PJ" },
    dados: { titulo, probabilidade: "medio", impacto: "medio" },
  };
}

function abaFicticia(): RegistrosDaDemanda {
  return {
    pasta: PASTA,
    pessoas: [{ slug: "maria-silva", nome: "Maria Silva", papel: "PO" }],
    registros: [
      registro({
        id: "PR-0001",
        categoria: "premissa",
        titulo: "Clientes PJ já têm limite de crédito cadastrado.",
        status: "nao-validada",
        quem: "Ana Exemplo",
        referencia: "A-0001/1",
        origem: "faturamento-exemplo/analises/A-0001.md",
      }),
      registro({
        id: "DE-0001",
        categoria: "decisao",
        titulo: "Máximo de 3 parcelas.",
        quem: "Maria Silva",
        pessoa: "maria-silva",
        decididoEm: "2026-11-10",
        referencia: "R-0001/D-1",
        alternativas: ["6 parcelas", "sem limite"],
      }),
      registro({
        id: "RI-0001",
        categoria: "risco",
        titulo: "Integração bancária rejeita remessa com parcelas.",
        probabilidade: "alto",
        impacto: "alto",
      }),
      registro({
        id: "RE-0001",
        categoria: "restricao",
        titulo: "Sem alteração no layout CNAB.",
        restricao: "tecnica",
      }),
    ],
    propostas: [
      propostaDeRisco(4, "Faturas antigas sem coluna de parcela."),
      {
        referencia: "R-0001/D-2",
        categoria: "decisao",
        fonte: { tipo: "ata", id: "R-0001", data: "2026-11-10" },
        dados: { titulo: "Relatório de recebíveis fica para a fase 2.", quem: "Maria Silva", pessoa: "maria-silva" },
      },
    ],
  };
}

function apiFalsa(aba = abaFicticia()) {
  const api: ApiRegistros = {
    registros: vi.fn(async () => aba),
    criar: vi.fn(async (_p, _d, categoria, campos) =>
      registro({ id: "PR-0002", categoria, titulo: campos.titulo }),
    ),
    editar: vi.fn(async (_p, id, campos) => registro({ id, categoria: "premissa", titulo: campos.titulo })),
    validarPremissa: vi.fn(async (_p, id) => registro({ id, categoria: "premissa", titulo: "x" })),
    aceitarProposta: vi.fn(async () => registro({ id: "RI-0002", categoria: "risco", titulo: "x" })),
    recusarProposta: vi.fn(async () => undefined),
  };
  return api;
}

const aba = (nome: RegExp) => screen.getByRole("tab", { name: nome });
const painel = () => screen.getByRole("tabpanel");

describe("RegistrosDemanda", () => {
  it("abas por tipo com contagem, no padrão tablist, abrindo na primeira com itens", async () => {
    const api = apiFalsa();
    const abrir = vi.fn();
    render(<RegistrosDemanda projeto={PROJETO} demanda="D-0001" api={api} abrirDocumento={abrir} />);
    const lista = await screen.findByRole("tablist", { name: "Tipo de registro" });
    expect(api.registros).toHaveBeenCalledWith(PROJETO, "D-0001");
    expect(within(lista).getAllByRole("tab").map((t) => t.textContent)).toEqual([
      "Premissas 1",
      "Decisões 2",
      "Riscos 2",
      "Restrições 1",
    ]);
    const premissas = aba(/Premissas/);
    expect(premissas).toHaveAttribute("aria-selected", "true");
    expect(premissas).toHaveAttribute("tabIndex", "0");
    expect(aba(/Decisões/)).toHaveAttribute("aria-selected", "false");
    expect(aba(/Decisões/)).toHaveAttribute("tabIndex", "-1");
    expect(painel()).toHaveAttribute("aria-labelledby", premissas.id);
    expect(premissas).toHaveAttribute("aria-controls", painel().id);

    const pr = painel().querySelector('[data-registro="PR-0001"]') as HTMLElement;
    expect(within(pr).getByText("não validada")).toHaveAttribute("data-tom", "atencao");
    expect(pr).toHaveTextContent("assumida por Ana Exemplo");
    expect(pr).toHaveTextContent("análise A-0001");
    fireEvent.click(within(pr).getByRole("button", { name: "abrir a origem" }));
    expect(abrir).toHaveBeenCalledWith("faturamento-exemplo/analises/A-0001.md");
    // O caminho interno da pasta não aparece na tela (#407).
    expect(screen.queryByText(new RegExp(PASTA))).toBeNull();

    // Setas, Home e End trocam a aba e levam o foco (roving tabindex).
    fireEvent.keyDown(premissas, { key: "ArrowRight" });
    expect(aba(/Decisões/)).toHaveAttribute("aria-selected", "true");
    expect(aba(/Decisões/)).toHaveFocus();
    expect(premissas).toHaveAttribute("tabIndex", "-1");
    expect(painel()).toHaveTextContent("decidiu Maria Silva");
    expect(painel()).toHaveTextContent("ata R-0001, item D-1");
    expect(painel()).toHaveTextContent("alternativas: 6 parcelas, sem limite");
    expect(painel()).not.toHaveTextContent("Clientes PJ");

    fireEvent.keyDown(aba(/Decisões/), { key: "End" });
    expect(aba(/Restrições/)).toHaveFocus();
    expect(painel()).toHaveTextContent("técnica");
    fireEvent.keyDown(aba(/Restrições/), { key: "ArrowRight" });
    expect(aba(/Premissas/)).toHaveFocus();
    fireEvent.keyDown(aba(/Premissas/), { key: "ArrowLeft" });
    expect(aba(/Restrições/)).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(aba(/Restrições/), { key: "Home" });
    expect(aba(/Premissas/)).toHaveAttribute("aria-selected", "true");

    fireEvent.click(aba(/Riscos/));
    const matriz = within(painel()).getByRole("table", { name: /probabilidade × impacto/ });
    expect(matriz).toHaveTextContent("RI-0001");
    expect(within(painel()).getByText("risco alto")).toBeInTheDocument();
  });

  it("propostas do agente: aviso com a origem, selo de tom agente, sugestão e ações em ícone", async () => {
    render(<RegistrosDemanda projeto={PROJETO} demanda="D-0001" api={apiFalsa()} />);
    fireEvent.click(await screen.findByRole("tab", { name: /Decisões/ }));
    const aviso = within(painel()).getByRole("note");
    expect(aviso).toHaveTextContent(
      "1 proposta do agente vinda de ata R-0001. Revise antes de valerem para a demanda.",
    );
    const item = painel().querySelector('[data-proposta="R-0001/D-2"]') as HTMLElement;
    expect(within(item).getByText("proposta do agente")).toHaveAttribute("data-tom", "agente");
    expect(item).toHaveTextContent("ata R-0001, item D-2");
    expect(item).toHaveTextContent("Maria Silva");
    for (const nome of ["Aceitar", "Ajustar", "Recusar"]) {
      expect(screen.getByRole("button", { name: `${nome} R-0001/D-2` })).toHaveClass("icone-acao");
    }

    fireEvent.click(aba(/Riscos/));
    expect(within(painel()).getByRole("note")).toHaveTextContent("1 proposta do agente vinda de A-0001.");
    const risco = painel().querySelector('[data-proposta="A-0001/4"]') as HTMLElement;
    expect(within(risco).getByText("análise A-0001")).toBeInTheDocument();
    expect(within(risco).getByText("sugere médio × médio")).toBeInTheDocument();
  });

  it("texto inteiro em Markdown, recolhido em 4 linhas com ver mais", async () => {
    const longo = `A remessa **CNAB 240** do banco ${"rejeita parcelas com vencimentos distintos no mesmo lote ".repeat(8)}e isso trava o faturamento.`;
    const dados = abaFicticia();
    dados.propostas = [propostaDeRisco(7, `${longo.slice(0, 159)}…`)];
    dados.propostas[0].dados.detalhe = longo;
    render(<RegistrosDemanda projeto={PROJETO} demanda="D-0001" api={apiFalsa(dados)} />);
    fireEvent.click(await screen.findByRole("tab", { name: /Riscos/ }));
    const item = painel().querySelector('[data-proposta="A-0001/7"]') as HTMLElement;
    expect(within(item).getByText("CNAB 240").tagName).toBe("STRONG");
    expect(item).not.toHaveTextContent("**");
    expect(item).not.toHaveTextContent("…");
    expect(item).toHaveTextContent("e isso trava o faturamento.");
    const verMais = within(item).getByRole("button", { name: "ver mais" });
    expect(verMais).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(verMais);
    expect(within(item).getByRole("button", { name: "ver menos" })).toHaveAttribute("aria-expanded", "true");
  });

  it("aba vazia mostra o estado vazio com Criar o primeiro", async () => {
    const dados = abaFicticia();
    dados.registros = dados.registros.filter((r) => r.categoria === "risco");
    dados.propostas = [];
    const api = apiFalsa(dados);
    render(<RegistrosDemanda projeto={PROJETO} demanda="D-0001" api={api} />);
    // A primeira aba com itens é a de riscos.
    expect(await screen.findByRole("tab", { name: "Riscos 1" })).toHaveAttribute("aria-selected", "true");
    fireEvent.click(aba(/Decisões/));
    expect(painel()).toHaveTextContent("Nenhum registro deste tipo.");
    fireEvent.click(within(painel()).getByRole("button", { name: "Criar o primeiro" }));
    expect(screen.getByRole("form", { name: "Nova decisão" })).toBeInTheDocument();
  });

  it("aceita, ajusta e recusa propostas uma a uma", async () => {
    const api = apiFalsa();
    render(<RegistrosDemanda projeto={PROJETO} demanda="D-0001" api={api} />);
    fireEvent.click(await screen.findByRole("tab", { name: /Decisões/ }));
    fireEvent.click(screen.getByRole("button", { name: "Aceitar R-0001/D-2" }));
    await waitFor(() =>
      expect(api.aceitarProposta).toHaveBeenCalledWith(PROJETO, "D-0001", "R-0001/D-2", undefined),
    );
    await waitFor(() => expect(screen.getByRole("button", { name: "Recusar R-0001/D-2" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Recusar R-0001/D-2" }));
    await waitFor(() =>
      expect(api.recusarProposta).toHaveBeenCalledWith(PROJETO, "D-0001", "R-0001/D-2"),
    );

    fireEvent.click(aba(/Riscos/));
    fireEvent.click(screen.getByRole("button", { name: "Ajustar A-0001/4" }));
    const form = screen.getByRole("form", { name: "Ajustar a proposta A-0001/4" });
    fireEvent.change(within(form).getByLabelText("Probabilidade"), { target: { value: "alto" } });
    fireEvent.click(within(form).getByRole("button", { name: "Aceitar" }));
    await waitFor(() =>
      expect(api.aceitarProposta).toHaveBeenLastCalledWith(
        PROJETO,
        "D-0001",
        "A-0001/4",
        expect.objectContaining({ probabilidade: "alto", impacto: "medio" }),
      ),
    );
  });

  it("aceita em lote as selecionadas como o aceite individual, e diz qual falhou", async () => {
    const dados = abaFicticia();
    dados.propostas = [propostaDeRisco(1), propostaDeRisco(2), propostaDeRisco(3)];
    const api = apiFalsa(dados);
    api.aceitarProposta = vi.fn(async (_p, _d, referencia) => {
      if (referencia === "A-0001/2") throw new Error("o registro já existe");
      return registro({ id: "RI-0009", categoria: "risco", titulo: "x" });
    });
    render(<RegistrosDemanda projeto={PROJETO} demanda="D-0001" api={api} />);
    fireEvent.click(await screen.findByRole("tab", { name: /Riscos/ }));
    const aviso = within(painel()).getByRole("note");
    expect(aviso).toHaveTextContent("3 propostas do agente vindas de A-0001.");
    const emLote = within(aviso).getByRole("button", { name: /Aceitar as selecionadas/ });
    expect(emLote).toBeDisabled();

    fireEvent.click(screen.getByRole("checkbox", { name: "Selecionar A-0001/1" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Selecionar A-0001/2" }));
    expect(emLote).toHaveTextContent("Aceitar as selecionadas (2)");
    fireEvent.click(emLote);

    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("1 de 2 propostas aceitas"));
    expect(screen.getByRole("alert")).toHaveTextContent("A-0001/2: o registro já existe");
    expect(api.aceitarProposta).toHaveBeenCalledTimes(2);
    expect(api.aceitarProposta).toHaveBeenNthCalledWith(1, PROJETO, "D-0001", "A-0001/1", undefined);
    expect(api.aceitarProposta).toHaveBeenNthCalledWith(2, PROJETO, "D-0001", "A-0001/2", undefined);
    // A aceita sai da seleção; a que falhou fica marcada para tentar de novo.
    expect(screen.getByRole("checkbox", { name: "Selecionar A-0001/1" })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Selecionar A-0001/2" })).toBeChecked();

    // "Selecionar todas" marca as da aba.
    fireEvent.click(within(aviso).getByRole("button", { name: "Selecionar todas" }));
    expect(screen.getAllByRole("checkbox").every((c) => (c as HTMLInputElement).checked)).toBe(true);
    expect(within(aviso).getByRole("button", { name: "Limpar a seleção" })).toBeInTheDocument();
  });

  it("cria pelo menu Novo registro e valida uma premissa dizendo como", async () => {
    const api = apiFalsa();
    render(<RegistrosDemanda projeto={PROJETO} demanda="D-0001" api={api} />);
    fireEvent.click(await screen.findByRole("button", { name: "Novo registro" }));
    expect(screen.getAllByRole("menuitem").map((i) => i.textContent)).toEqual([
      "Nova premissa",
      "Nova decisão",
      "Novo risco",
      "Nova restrição",
    ]);
    fireEvent.click(screen.getByRole("menuitem", { name: "Nova premissa" }));
    const form = screen.getByRole("form", { name: "Nova premissa" });
    fireEvent.change(within(form).getByLabelText("Enunciado da premissa"), {
      target: { value: "O banco aceita vencimentos distintos." },
    });
    fireEvent.change(within(form).getByLabelText(/Quem assumiu \(catálogo/), {
      target: { value: "maria-silva" },
    });
    fireEvent.click(within(form).getByRole("button", { name: "Salvar" }));
    await waitFor(() =>
      expect(api.criar).toHaveBeenCalledWith(
        PROJETO,
        "D-0001",
        "premissa",
        expect.objectContaining({
          titulo: "O banco aceita vencimentos distintos.",
          pessoa: "maria-silva",
          quem: "Maria Silva",
          status: "nao-validada",
        }),
      ),
    );

    await waitFor(() => expect(screen.getByRole("button", { name: "Validar PR-0001" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Validar PR-0001" }));
    const validar = screen.getByRole("form", { name: "Validar PR-0001" });
    fireEvent.change(within(validar).getByLabelText("Como foi validada"), {
      target: { value: "Confirmado pelo financeiro" },
    });
    fireEvent.click(within(validar).getByRole("button", { name: "Validar" }));
    await waitFor(() =>
      expect(api.validarPremissa).toHaveBeenCalledWith(PROJETO, "PR-0001", {
        status: "validada",
        como: "Confirmado pelo financeiro",
      }),
    );
  });

  it("sem edição, não mostra o menu, a seleção nem as ações", async () => {
    render(<RegistrosDemanda projeto={PROJETO} demanda="D-0001" api={apiFalsa()} podeEditar={false} />);
    fireEvent.click(await screen.findByRole("tab", { name: /Riscos/ }));
    expect(screen.queryByRole("button", { name: "Novo registro" })).not.toBeInTheDocument();
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Aceitar/ })).not.toBeInTheDocument();
    expect(within(painel()).getByRole("note")).toHaveTextContent("1 proposta do agente");
  });
});

describe("textoCompleto", () => {
  it("troca o enunciado cortado pelo detalhe inteiro", () => {
    expect(textoCompleto("O banco rejeita…", "O banco  rejeita\nremessas com parcelas.")).toBe(
      "O banco  rejeita\nremessas com parcelas.",
    );
  });

  it("junta enunciado e detalhe diferentes; sem detalhe, só o enunciado", () => {
    expect(textoCompleto("Máximo de 3 parcelas.", "Pedido do financeiro.")).toBe(
      "Máximo de 3 parcelas.\n\nPedido do financeiro.",
    );
    expect(textoCompleto("Máximo de 3 parcelas.", null)).toBe("Máximo de 3 parcelas.");
    expect(textoCompleto("Máximo de 3 parcelas.", " Máximo de 3 parcelas. ")).toBe("Máximo de 3 parcelas.");
  });
});
