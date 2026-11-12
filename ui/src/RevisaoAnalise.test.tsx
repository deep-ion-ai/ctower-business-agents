import { act } from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { PainelAnalise, type Abertura } from "./PainelAnalise";
import type { Afirmacao, Analise } from "./analise";
import type { AprovacaoFeita, ApiRevisao, DecisaoRevisao, Prontidao } from "./revisao";
import {
  AFIRMACAO_MULTA,
  apiAnaliseFalsa,
  apiConversaFalsa,
  apiTerminalFalsa,
  telaFalsa,
  vistaFicticia,
} from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: vi.fn(), open: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

const ERRADA: Afirmacao = {
  numero: 2,
  secao: "resposta",
  texto: "A multa incide também sobre os juros de mora.",
  confianca: "inferido",
  estado: "registrada",
  evidencias: [],
  registrada_ms: 3,
};

const AREAS = [
  { id: "cobranca", nome: "Cobrança", sigla: "COB" },
  { id: "parcelamento", nome: "Parcelamento", sigla: "PAR" },
  { id: "cancelamento", nome: "Cancelamento", sigla: "CAN" },
];

/** Análise concluída do sistema fictício, com a revisão aplicada como o Rust faria. */
function cenarioRevisao(areas = AREAS) {
  let estado: Analise = vistaFicticia(
    {},
    {
      estado: "concluida",
      afirmacoes: [AFIRMACAO_MULTA, ERRADA],
      perguntas: [
        {
          numero: 1,
          texto: "A carência vale para pessoa jurídica?",
          estado: "aberta",
          registrada_ms: 4,
        },
      ],
      termos: [
        {
          numero: 1,
          termo: "Carência",
          definicao: "Dias sem multa depois do vencimento.",
          estado: "proposto",
        },
      ],
    },
  ).analise;
  const vista = () => vistaFicticia({}, estado);

  const aplicar = (d: DecisaoRevisao): Analise => {
    const a = structuredClone(estado);
    if (d.tipo === "afirmacao") {
      const x = a.afirmacoes.find((y) => y.numero === d.numero)!;
      Object.assign(x, d.estado === "incorreta" ? { estado: d.estado, comentario: d.comentario } : { estado: d.estado });
    } else if (d.tipo === "editar-afirmacao") {
      const x = a.afirmacoes.find((y) => y.numero === d.numero)!;
      x.texto_original = x.texto_original ?? x.texto;
      x.texto = d.texto;
    } else if (d.tipo === "termo") {
      a.termos.find((t) => t.numero === d.numero)!.estado = d.estado;
    } else if (d.tipo === "pergunta") {
      Object.assign(a.perguntas.find((p) => p.numero === d.numero)!, d);
    }
    a.versao += 1;
    return a;
  };

  const prontidao = (): Prontidao => {
    const pendencias = [
      ...estado.afirmacoes
        .filter((a) => a.estado !== "confirmada" && a.estado !== "incorreta")
        .map((a) => ({
          tipo: "afirmacao" as const,
          numero: a.numero,
          motivo: `afirmação ${a.numero}: sem decisão`,
        })),
      ...estado.termos
        .filter((t) => t.estado === "proposto")
        .map((t) => ({ tipo: "termo" as const, numero: t.numero, motivo: `termo "${t.termo}"` })),
    ];
    return {
      estado: estado.estado,
      podeAprovar: pendencias.length === 0 && estado.estado === "concluida",
      pendencias,
      avisos: [],
      areas,
      areaObrigatoria: areas.length > 1,
      regras: estado.afirmacoes
        .filter((a) => a.estado === "confirmada")
        .map((a) => ({ afirmacao: a.numero, titulo: "Multa de 2% após a carência", texto: a.texto })),
      incorretas: estado.afirmacoes.filter((a) => a.estado === "incorreta").map((a) => a.numero),
      termosAceitos: estado.termos.filter((t) => t.estado === "aceito").map((t) => t.termo),
    };
  };

  const feita: AprovacaoFeita = {
    jaAprovada: false,
    regras: [
      {
        id: "RN-COB-001",
        arquivo: "regras/RN-COB-001.md",
        area: "cobranca",
        afirmacoes: [1],
        acao: "criada",
      },
    ],
    termos: [{ termo: "Carência", arquivo: "glossario/carencia.md", acao: "criada" }],
    avisos: [],
  };

  const revisao = {
    revisar: vi.fn((_s: string, _a: string, d: DecisaoRevisao) => {
      estado = aplicar(d);
      return Promise.resolve(vista());
    }),
    prontidao: vi.fn(() => Promise.resolve(prontidao())),
    aprovar: vi.fn(() => {
      estado = { ...estado, estado: "aprovada", versao: estado.versao + 1 };
      return Promise.resolve(feita);
    }),
    escolherDestino: vi.fn((nome: string, formato: "markdown" | "word") =>
      Promise.resolve<string | null>(`/home/analista/${nome}.${formato === "word" ? "docx" : "md"}`),
    ),
    exportar: vi.fn((_s: string, _a: string, p: { destino: string }) => Promise.resolve(p.destino)),
  } satisfies ApiRevisao;

  const analise = apiAnaliseFalsa(vista());
  analise.api.carregarAnalise.mockImplementation(() => Promise.resolve(vista()));
  return { revisao, analise, estado: () => estado };
}

function montar(c: ReturnType<typeof cenarioRevisao>, abertura: Abertura = { tipo: "reabrir" }) {
  const terminal = apiTerminalFalsa();
  render(
    <PainelAnalise
      api={c.analise.api}
      projeto="faturamento-exemplo"
      analise="A-0001"
      abertura={abertura}
      aoVoltar={vi.fn()}
      apiTerminal={terminal.api}
      apiConversa={apiConversaFalsa()}
      apiRevisao={c.revisao}
      criarTela={telaFalsa}
    />,
  );
  return terminal;
}

const afirmacao = (n: number) => screen.getByRole("group", { name: `Revisão da afirmação ${n}` });

describe("Revisão, aprovação e exportação", () => {
  it("revisa e aprova a análise do sistema fictício, e a incorreta não vira regra", async () => {
    const c = cenarioRevisao();
    montar(c);
    await screen.findByText(AFIRMACAO_MULTA.texto);

    // Antes da revisão, a aprovação mostra o que falta e fica bloqueada.
    const aprovacao = await screen.findByRole("region", { name: "Aprovação" });
    const pendencias = await within(aprovacao).findByRole("list", { name: "Pendências" });
    expect(within(pendencias).getAllByRole("listitem")).toHaveLength(3);
    expect(within(aprovacao).getByRole("button", { name: "Aprovar análise" })).toBeDisabled();

    // Confirmar a afirmação 1.
    fireEvent.click(within(afirmacao(1)).getByRole("button", { name: "Confirmar" }));
    await waitFor(() =>
      expect(c.revisao.revisar).toHaveBeenCalledWith("faturamento-exemplo", "A-0001", {
        tipo: "afirmacao",
        numero: 1,
        estado: "confirmada",
      }),
    );
    expect(await screen.findByText("(confirmada pelo analista)")).toBeInTheDocument();

    // Marcar a 2 como incorreta exige o comentário.
    fireEvent.click(within(afirmacao(2)).getByRole("button", { name: "Marcar incorreta" }));
    const comentario = within(afirmacao(2)).getByLabelText(/Por que está incorreta/);
    const marcar = within(afirmacao(2)).getByRole("button", { name: "Marcar como incorreta" });
    expect(marcar).toBeDisabled();
    fireEvent.change(comentario, { target: { value: "Os juros não entram na base da multa." } });
    fireEvent.click(marcar);
    await waitFor(() =>
      expect(c.revisao.revisar).toHaveBeenLastCalledWith("faturamento-exemplo", "A-0001", {
        tipo: "afirmacao",
        numero: 2,
        estado: "incorreta",
        comentario: "Os juros não entram na base da multa.",
      }),
    );
    expect(await screen.findByText("(marcada como incorreta)")).toBeInTheDocument();
    expect(screen.getByText("Os juros não entram na base da multa.")).toBeInTheDocument();

    // Termo aceito e pergunta respondida.
    fireEvent.click(
      within(screen.getByRole("group", { name: "Revisão do termo Carência" })).getByRole("button", {
        name: "Aceitar",
      }),
    );
    await waitFor(() => expect(c.estado().termos[0].estado).toBe("aceito"));
    fireEvent.click(screen.getByRole("button", { name: "Responder" }));
    fireEvent.change(screen.getByLabelText("Resposta"), { target: { value: "Sim." } });
    fireEvent.click(screen.getByRole("button", { name: "Salvar resposta" }));
    await waitFor(() => expect(c.estado().perguntas[0].estado).toBe("respondida"));

    // Sem pendências, mas com três áreas: a área precisa ser escolhida.
    await waitFor(() =>
      expect(within(aprovacao).queryByRole("list", { name: "Pendências" })).not.toBeInTheDocument(),
    );
    expect(within(aprovacao).getByText(/1 incorreta\(s\) ficam registradas e não viram regra/)).toBeInTheDocument();
    const aprovar = within(aprovacao).getByRole("button", { name: "Aprovar análise" });
    expect(aprovar).toBeDisabled();
    expect(within(aprovacao).getByText(/escolha a das regras/)).toBeInTheDocument();
    fireEvent.change(within(aprovacao).getByLabelText("Área das regras"), {
      target: { value: "cobranca" },
    });
    fireEvent.change(within(aprovacao).getByLabelText(/Título da regra da afirmação 1/), {
      target: { value: "Multa por atraso" },
    });
    expect(aprovar).toBeEnabled();
    fireEvent.click(aprovar);
    await waitFor(() =>
      expect(c.revisao.aprovar).toHaveBeenCalledWith("faturamento-exemplo", "A-0001", {
        area: "cobranca",
        afirmacoes: { 1: { area: null, titulo: "Multa por atraso" } },
      }),
    );
    const resultado = await screen.findByRole("status", { name: "Resultado da aprovação" });
    expect(within(resultado).getByText("RN-COB-001 (cobranca): criada")).toBeInTheDocument();
    expect(within(resultado).getByText("Carência: criada")).toBeInTheDocument();

    // Aprovada: o documento fica só de leitura, e a incorreta segue registrada.
    await screen.findByText(/Análise aprovada ·/);
    expect(screen.queryByRole("group", { name: "Revisão da afirmação 1" })).not.toBeInTheDocument();
    expect(c.estado().afirmacoes[1]).toMatchObject({
      estado: "incorreta",
      comentario: "Os juros não entram na base da multa.",
    });
  });

  it("edita o texto da afirmação e mantém as evidências", async () => {
    const c = cenarioRevisao();
    montar(c);
    await screen.findByText(AFIRMACAO_MULTA.texto);
    fireEvent.click(within(afirmacao(1)).getByRole("button", { name: "Editar texto" }));
    const campo = within(afirmacao(1)).getByLabelText(/Texto da afirmação/);
    expect(campo).toHaveValue(AFIRMACAO_MULTA.texto);
    fireEvent.change(campo, { target: { value: "Depois da carência, a multa é de 2%." } });
    fireEvent.click(within(afirmacao(1)).getByRole("button", { name: "Salvar texto" }));
    await waitFor(() =>
      expect(c.revisao.revisar).toHaveBeenCalledWith("faturamento-exemplo", "A-0001", {
        tipo: "editar-afirmacao",
        numero: 1,
        texto: "Depois da carência, a multa é de 2%.",
      }),
    );
    expect(await screen.findByText("Depois da carência, a multa é de 2%.")).toBeInTheDocument();
    expect(screen.getByText(/texto editado pelo analista/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "db/03_pkg_cobranca.sql:71-72" })).toBeInTheDocument();
  });

  it("mostra a recusa da revisão", async () => {
    const c = cenarioRevisao();
    c.revisao.revisar.mockRejectedValueOnce("a análise A-0001 já foi aprovada");
    montar(c);
    await screen.findByText(AFIRMACAO_MULTA.texto);
    fireEvent.click(within(afirmacao(1)).getByRole("button", { name: "Confirmar" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("já foi aprovada");
  });

  it("pedir aprofundamento envia o prompt pronto à sessão viva", async () => {
    const c = cenarioRevisao();
    c.analise.api.promptsProntos.mockResolvedValue([
      {
        id: "aprofundar",
        rotulo: "Aprofundar a análise",
        descricao: "",
        texto: "Aprofunde a análise A-0001 pelo ctower.",
      },
    ]);
    const terminal = montar(c, { tipo: "nova", promptInicial: "Inicie" });
    await waitFor(() => expect(terminal.aberturas.length).toBe(1));
    await act(async () => terminal.aberturas[0].resolver(5));
    await screen.findByText(AFIRMACAO_MULTA.texto);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Aprofundar a análise" })).toBeEnabled(),
    );
    fireEvent.click(within(afirmacao(2)).getByRole("button", { name: "Aprofundar" }));
    await waitFor(() =>
      expect(terminal.api.enviarPrompt).toHaveBeenCalledWith(
        5,
        "Aprofunde a análise A-0001 pelo ctower. Comece pela afirmação 2.",
      ),
    );
    expect(c.estado().afirmacoes[1].estado).toBe("aprofundar");
  });

  it("sem a conversa aberta, o aprofundamento fica registrado com o aviso", async () => {
    const c = cenarioRevisao();
    montar(c);
    await screen.findByText(AFIRMACAO_MULTA.texto);
    fireEvent.click(within(afirmacao(2)).getByRole("button", { name: "Aprofundar" }));
    expect(await screen.findByText(/Aprofundamento registrado/)).toBeInTheDocument();
    expect(await screen.findByText("(aprofundamento pedido)")).toBeInTheDocument();
  });

  it("com uma área só, ela já vem escolhida", async () => {
    const c = cenarioRevisao([AREAS[0]]);
    montar(c);
    await screen.findByText(AFIRMACAO_MULTA.texto);
    const aprovacao = await screen.findByRole("region", { name: "Aprovação" });
    await waitFor(() =>
      expect(within(aprovacao).getByLabelText("Área das regras")).toHaveValue("cobranca"),
    );
  });

  it("exporta para Word e Markdown com o diálogo de salvar", async () => {
    const c = cenarioRevisao();
    montar(c);
    const exportacao = await screen.findByRole("region", { name: "Exportação" });
    expect(within(exportacao).getByText(/sai como rascunho/)).toBeInTheDocument();
    fireEvent.click(within(exportacao).getByRole("button", { name: "Exportar…" }));
    await waitFor(() =>
      expect(c.revisao.exportar).toHaveBeenCalledWith("faturamento-exemplo", "A-0001", {
        destino: "/home/analista/A-0001.docx",
        formato: "word",
        opcoes: { evidencias: true, perguntas: true, trechos: true },
      }),
    );
    expect(c.revisao.escolherDestino).toHaveBeenCalledWith("A-0001", "word");
    expect(await within(exportacao).findByRole("status")).toHaveTextContent(
      "Exportado para /home/analista/A-0001.docx",
    );

    fireEvent.click(within(exportacao).getByLabelText("Markdown (.md)"));
    fireEvent.click(within(exportacao).getByLabelText("Perguntas em aberto"));
    fireEvent.click(within(exportacao).getByRole("button", { name: "Exportar…" }));
    await waitFor(() =>
      expect(c.revisao.exportar).toHaveBeenLastCalledWith("faturamento-exemplo", "A-0001", {
        destino: "/home/analista/A-0001.md",
        formato: "markdown",
        opcoes: { evidencias: true, perguntas: false, trechos: true },
      }),
    );

    // Diálogo cancelado: nada é exportado. Recusa do app: aparece a mensagem.
    c.revisao.escolherDestino.mockResolvedValueOnce(null);
    fireEvent.click(within(exportacao).getByRole("button", { name: "Exportar…" }));
    await waitFor(() => expect(c.revisao.escolherDestino).toHaveBeenCalledTimes(3));
    expect(c.revisao.exportar).toHaveBeenCalledTimes(2);
    c.revisao.exportar.mockRejectedValueOnce("o destino fica dentro das pastas do projeto");
    fireEvent.click(within(exportacao).getByRole("button", { name: "Exportar…" }));
    expect(await within(exportacao).findByRole("alert")).toHaveTextContent("pastas do projeto");
  });
});
