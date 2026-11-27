import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PautaPerguntas, PERGUNTAS_VISIVEIS } from "./PautaPerguntas";
import {
  agrupar,
  destinatarioDoValor,
  origemCurta,
  valorDestinatario,
  type ApiPauta,
  type Pauta,
} from "./pauta";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

const PROJETO = "faturamento-exemplo";

function pautaFicticia(): Pauta {
  return {
    titulo: "D-0001: Parcelamento da fatura para PJ",
    total: 4,
    antigas: 1,
    geradaEm: "2026-11-20",
    pessoas: [{ slug: "maria-silva", nome: "Maria Silva", papel: "PO" }],
    papeis: ["PO", "usuário-chave"],
    premissas: [
      {
        id: "PR-0001",
        titulo: "O banco aceita vencimentos distintos.",
        status: "nao-validada",
      },
    ],
    emails: [
      "Olá, Maria Silva,\n\n1. O limite de parcelas vale para PJ?",
      "Olá,",
    ],
    grupos: [
      {
        destinatario: { tipo: "pessoa", pessoa: "maria-silva" },
        rotulo: "Maria Silva (PO)",
        itens: [
          {
            texto: "O limite de parcelas vale para PJ?",
            destinatario: { tipo: "pessoa", pessoa: "maria-silva" },
            idadeDias: 3,
            antiga: false,
            perguntas: [
              {
                referencia: "A-0001/1",
                fonte: {
                  tipo: "analise",
                  id: "A-0001",
                  titulo: "Impacto do parcelamento",
                  fluxo: "impacto",
                },
                texto: "O limite de parcelas vale para PJ?",
                contexto:
                  "Hoje há **dois caminhos**:\n\n- **Acordo de cobrança**: mantém a fatura.\n- **Renegociação**: apaga o histórico.",
                destinatario: { tipo: "pessoa", pessoa: "maria-silva" },
                tema: "como e hoje",
                desde: "2026-11-17",
                idadeDias: 3,
                bloqueia: ["HU-0001 · CA-2"],
                documento: "faturamento-exemplo/analises/A-0001.md",
              },
              {
                referencia: "A-0002/1",
                fonte: {
                  tipo: "analise",
                  id: "A-0002",
                  titulo: "Impacto na remessa",
                  fluxo: "impacto",
                },
                texto: "o limite de parcelas vale para PJ",
                contexto: null,
                destinatario: { tipo: "pessoa", pessoa: "maria-silva" },
                tema: "Impacto na remessa",
                desde: "2026-11-17",
                idadeDias: 3,
                bloqueia: [],
                documento: "faturamento-exemplo/analises/A-0002.md",
              },
            ],
          },
          {
            texto: "O boleto aceita vencimentos distintos?",
            destinatario: { tipo: "pessoa", pessoa: "maria-silva" },
            idadeDias: 17,
            antiga: true,
            perguntas: [
              {
                referencia: "R-0001/DV-1",
                fonte: { tipo: "ata", id: "R-0001", data: "2026-11-03" },
                texto: "O boleto aceita vencimentos distintos?",
                contexto: null,
                destinatario: { tipo: "pessoa", pessoa: "maria-silva" },
                tema: "Reunião R-0001",
                desde: "2026-11-03",
                idadeDias: 17,
                bloqueia: [],
                documento:
                  "faturamento-exemplo/demandas/D-0001-x/reunioes/R-0001.md",
              },
            ],
          },
        ],
      },
      {
        destinatario: null,
        rotulo: "Sem destinatário",
        itens: [
          {
            texto: "Há juros no parcelamento?",
            destinatario: null,
            idadeDias: 0,
            antiga: false,
            perguntas: [
              {
                referencia: "A-0001/2",
                fonte: {
                  tipo: "analise",
                  id: "A-0001",
                  titulo: "Impacto do parcelamento",
                  fluxo: "impacto",
                },
                texto: "Há juros no parcelamento?",
                contexto: null,
                destinatario: null,
                tema: "Impacto do parcelamento",
                desde: "2026-11-20",
                idadeDias: 0,
                bloqueia: [],
                documento: "faturamento-exemplo/analises/A-0001.md",
              },
            ],
          },
        ],
      },
    ],
  };
}

function apiFalsa(): ApiPauta {
  return {
    pauta: vi.fn(async () => pautaFicticia()),
    definirDestinatario: vi.fn(async () => undefined),
    registrarResposta: vi.fn(async (_p, r) => ({
      fechadas: r.perguntas,
      regra:
        r.destino.tipo === "regra"
          ? { id: "RN-GER-001", documento: "x/regras/RN-GER-001.md" }
          : null,
      premissa: null,
      avisos: [],
    })),
    escolherDestino: vi.fn(
      async (nome: string, formato: string) => `/tmp/${nome}.${formato}`,
    ),
    exportar: vi.fn(async (_p, _d, _f, destino: string) => destino),
    copiar: vi.fn(async () => undefined),
  };
}

describe("PautaPerguntas", () => {
  it("agrupa por destinatário, junta as perguntas iguais e destaca as antigas", async () => {
    const api = apiFalsa();
    render(<PautaPerguntas projeto={PROJETO} demanda="D-0001" api={api} />);
    const maria = await screen.findByRole("region", {
      name: "Maria Silva (PO)",
    });
    expect(within(maria).getByText("2 perguntas iguais")).toBeInTheDocument();
    // A origem é curta, sem o título da análise.
    expect(
      within(maria).getByRole("button", { name: "A-0001 · pergunta 1" }),
    ).toBeInTheDocument();
    expect(
      within(maria).getByRole("button", { name: "A-0002 · pergunta 1" }),
    ).toBeInTheDocument();
    expect(
      within(maria).getByRole("button", { name: "R-0001 · dúvida 1" }),
    ).toBeInTheDocument();
    expect(
      within(maria).queryByText(/Impacto do parcelamento|Impacto na remessa/),
    ).not.toBeInTheDocument();
    expect(
      within(maria).getByText(/bloqueia HU-0001 · CA-2/),
    ).toBeInTheDocument();
    expect(within(maria).getByText("há 17 dias")).toHaveAttribute(
      "data-tom",
      "atencao",
    );
    expect(screen.getByText("aberta hoje")).toHaveAttribute(
      "data-tom",
      "neutro",
    );
    expect(within(maria).getByText("2 perguntas")).toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: "Sem destinatário" }),
    ).toBeInTheDocument();
    expect(screen.getByText("1 antiga")).toBeInTheDocument();
    expect(api.pauta).toHaveBeenCalledWith(PROJETO, "D-0001");

    fireEvent.click(
      within(maria).getByRole("button", { name: "Copiar como e-mail" }),
    );
    await waitFor(() =>
      expect(api.copiar).toHaveBeenCalledWith(pautaFicticia().emails[0]),
    );

    fireEvent.change(screen.getByLabelText("Destinatário de A-0001/2"), {
      target: { value: "papel:PO" },
    });
    await waitFor(() =>
      expect(api.definirDestinatario).toHaveBeenCalledWith(
        PROJETO,
        ["A-0001/2"],
        {
          tipo: "papel",
          papel: "PO",
        },
      ),
    );
  });

  it("registra a resposta nas duas análises e a transforma em regra informada", async () => {
    const api = apiFalsa();
    render(<PautaPerguntas projeto={PROJETO} demanda="D-0001" api={api} />);
    const maria = await screen.findByRole("region", {
      name: "Maria Silva (PO)",
    });
    fireEvent.click(
      within(maria).getAllByRole("button", { name: "Registrar resposta" })[0],
    );
    const form = screen.getByRole("form", { name: "Registrar resposta" });
    fireEvent.change(within(form).getByLabelText("Resposta"), {
      target: { value: "Sim, só para PJ." },
    });
    expect(within(form).getByLabelText("Nome")).toHaveValue("Maria Silva");
    fireEvent.change(within(form).getByLabelText("Fonte"), {
      target: { value: "e-mail" },
    });
    fireEvent.click(within(form).getByLabelText(/Virar regra de negócio/));
    expect(within(form).getByRole("note")).toHaveTextContent(
      "sem evidência no código",
    );
    fireEvent.click(within(form).getByRole("button", { name: "Registrar" }));
    await waitFor(() => expect(api.registrarResposta).toHaveBeenCalled());
    const enviada = vi.mocked(api.registrarResposta).mock.calls[0][1];
    expect(enviada).toMatchObject({
      texto: "Sim, só para PJ.",
      por: "Maria Silva",
      pessoa: "maria-silva",
      fonte: "e-mail",
      perguntas: ["A-0001/1", "A-0002/1"],
      destino: { tipo: "regra", titulo: null, area: null },
    });
    expect(await screen.findByRole("status")).toHaveTextContent(
      "as perguntas A-0001/1, A-0002/1 foram fechadas. Regra RN-GER-001 criada",
    );
  });

  it("mostra a decisão com as opções e registra a opção escolhida como resposta", async () => {
    const api = apiFalsa();
    const p = pautaFicticia();
    const [sem] = p.grupos.slice(-1);
    const decisao = {
      opcoes: [
        { texto: "Manter a regra da DEM-12", efeito: "a fatura com pagamento parcial segue sem cancelamento" },
        { texto: "Revogar a regra", efeito: "revoga a regra da DEM-12" },
      ],
      recomendacao: "Revogar, com estorno do valor pago.",
    };
    sem.itens.unshift({
      texto: "A regra da DEM-12 continua valendo?",
      destinatario: null,
      idadeDias: 0,
      antiga: false,
      decisao,
      perguntas: [
        {
          ...sem.itens[0].perguntas[0],
          referencia: "A-0001/3",
          texto: "A regra da DEM-12 continua valendo?",
          decisao,
        },
      ],
    });
    vi.mocked(api.pauta).mockResolvedValue(p);
    render(<PautaPerguntas projeto={PROJETO} demanda="D-0001" api={api} />);
    const grupo = await screen.findByRole("region", { name: "Sem destinatário" });
    const cartoes = within(grupo).getAllByRole("listitem").filter((li) =>
      li.classList.contains("pauta-item"),
    );
    // A decisão vem primeiro, com o selo, as opções e o efeito de cada uma.
    expect(cartoes[0]).toHaveClass("pauta-item-decisao");
    expect(within(cartoes[0]).getByText("Decisão")).toBeInTheDocument();
    const opcoes = within(cartoes[0]).getByRole("list", { name: "Opções" });
    expect(within(opcoes).getAllByRole("listitem")).toHaveLength(2);
    expect(opcoes).toHaveTextContent("Revogar a regra — efeito: revoga a regra da DEM-12");
    expect(cartoes[0]).toHaveTextContent("Recomendação: Revogar, com estorno do valor pago.");

    fireEvent.click(within(cartoes[0]).getByRole("button", { name: "Registrar resposta" }));
    const form = screen.getByRole("form", { name: "Registrar resposta" });
    fireEvent.click(within(form).getByRole("radio", { name: "Revogar a regra" }));
    expect(within(form).getByLabelText("Resposta")).toHaveValue("Revogar a regra");
    fireEvent.change(within(form).getByLabelText("Nome"), {
      target: { value: "Maria Silva" },
    });
    fireEvent.click(within(form).getByRole("button", { name: "Registrar" }));
    await waitFor(() => expect(api.registrarResposta).toHaveBeenCalled());
    expect(vi.mocked(api.registrarResposta).mock.calls[0][1]).toMatchObject({
      texto: "Revogar a regra",
      perguntas: ["A-0001/3"],
    });
  });

  it("mostra o que o sistema faz hoje recolhido e, aberto, em Markdown", async () => {
    const api = apiFalsa();
    render(<PautaPerguntas projeto={PROJETO} demanda="D-0001" api={api} />);
    const maria = await screen.findByRole("region", {
      name: "Maria Silva (PO)",
    });
    // Só a pergunta com contexto tem o botão.
    const alternar = within(maria).getByRole("button", {
      name: "O que o sistema faz hoje",
    });
    expect(alternar.tagName).toBe("BUTTON");
    expect(alternar).toHaveAttribute("aria-expanded", "false");
    expect(alternar).toHaveTextContent("▸ O que o sistema faz hoje");
    expect(within(maria).queryByText("dois caminhos")).not.toBeInTheDocument();

    fireEvent.click(alternar);
    expect(alternar).toHaveAttribute("aria-expanded", "true");
    const contexto = document.getElementById(
      alternar.getAttribute("aria-controls")!,
    )!;
    expect(within(contexto).getByText("dois caminhos").tagName).toBe("STRONG");
    expect(within(contexto).getAllByRole("listitem")).toHaveLength(2);
    expect(contexto).not.toHaveTextContent("**");
    expect(contexto).not.toHaveTextContent("- ");

    fireEvent.click(alternar);
    expect(alternar).toHaveAttribute("aria-expanded", "false");
    expect(within(maria).queryByText("dois caminhos")).not.toBeInTheDocument();
  });

  it("mostra as primeiras perguntas de cada grupo e um botão para as outras", async () => {
    const api = apiFalsa();
    const base = pautaFicticia();
    const modelo = base.grupos[1].itens[0];
    const itens = Array.from({ length: PERGUNTAS_VISIVEIS + 3 }, (_, i) => ({
      ...modelo,
      texto: `Pergunta ${i + 1}?`,
      perguntas: [
        {
          ...modelo.perguntas[0],
          referencia: `A-0003/${i + 1}`,
          texto: `Pergunta ${i + 1}?`,
        },
      ],
    }));
    vi.mocked(api.pauta).mockResolvedValue({
      ...base,
      total: itens.length,
      antigas: 0,
      emails: [],
      grupos: [{ destinatario: null, rotulo: "Sem destinatário", itens }],
    });
    render(<PautaPerguntas projeto={PROJETO} demanda="D-0001" api={api} />);
    const grupo = await screen.findByRole("region", {
      name: "Sem destinatário",
    });
    expect(within(grupo).getByText("6 perguntas")).toBeInTheDocument();
    expect(
      within(grupo).queryByRole("button", { name: "Copiar como e-mail" }),
    ).not.toBeInTheDocument();
    const lista = grupo.querySelector("ol")!;
    expect(within(grupo).getAllByText(/^Pergunta \d\?$/)).toHaveLength(
      PERGUNTAS_VISIVEIS,
    );
    const mais = within(grupo).getByRole("button", {
      name: "Mostrar as outras 3 perguntas",
    });
    expect(mais).toHaveAttribute("aria-expanded", "false");
    expect(mais).toHaveAttribute("aria-controls", lista.id);

    mais.focus();
    fireEvent.click(mais);
    expect(within(grupo).getAllByText(/^Pergunta \d\?$/)).toHaveLength(6);
    // O mesmo botão fica no lugar: o foco não se perde.
    expect(mais).toHaveFocus();
    expect(mais).toHaveAttribute("aria-expanded", "true");
    expect(mais).toHaveTextContent("Mostrar menos");

    fireEvent.click(mais);
    expect(within(grupo).getAllByText(/^Pergunta \d\?$/)).toHaveLength(
      PERGUNTAS_VISIVEIS,
    );
  });

  it("exporta a pauta em Word pelo menu Exportar", async () => {
    const api = apiFalsa();
    render(<PautaPerguntas projeto={PROJETO} demanda="D-0001" api={api} />);
    const exportar = await screen.findByRole("button", { name: /^Exportar/ });
    expect(exportar).toHaveAttribute("aria-haspopup", "menu");
    fireEvent.click(exportar);
    const menu = screen.getByRole("menu", { name: "Exportar" });
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((b) => b.textContent),
    ).toEqual(["Markdown", "Word"]);
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Word" }));
    await waitFor(() =>
      expect(api.exportar).toHaveBeenCalledWith(
        PROJETO,
        "D-0001",
        "word",
        "/tmp/pauta-D-0001.word",
      ),
    );
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Pauta exportada em",
    );
  });
});

describe("pauta", () => {
  it("agrupa por papel e por tema", () => {
    const p = pautaFicticia();
    expect(agrupar(p, "papel").map((g) => g.rotulo)).toEqual([
      "PO",
      "Sem destinatário",
    ]);
    expect(agrupar(p, "tema").map((g) => g.rotulo)).toEqual([
      "como e hoje",
      "Impacto do parcelamento",
      "Reunião R-0001",
    ]);
  });

  it("origem curta, sem o título da análise", () => {
    const [maria] = pautaFicticia().grupos;
    const [a1, a2] = maria.itens[0].perguntas;
    expect(origemCurta(a1)).toBe("A-0001 · pergunta 1");
    expect(origemCurta(a2)).toBe("A-0002 · pergunta 1");
    expect(origemCurta(maria.itens[1].perguntas[0])).toBe("R-0001 · dúvida 1");
    expect(origemCurta({ ...a1, referencia: "A-0001" })).toBe("A-0001");
  });

  it("destinatário no select", () => {
    expect(valorDestinatario({ tipo: "pessoa", pessoa: "maria-silva" })).toBe(
      "pessoa:maria-silva",
    );
    expect(destinatarioDoValor("papel:PO")).toEqual({
      tipo: "papel",
      papel: "PO",
    });
    expect(destinatarioDoValor("")).toBeNull();
  });
});
