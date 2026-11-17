import { useState } from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Afirmacao, Analise } from "./analise";
import { DocumentoAoVivo, type OrigemEvidencia } from "./DocumentoAoVivo";
import type { Revisao } from "./AcoesRevisao";
import type { DecisaoRevisao } from "./revisao";
import { contagemPorFiltro, passaNoFiltro, resumir } from "./resumoRevisao";
import { AFIRMACAO_MULTA, vistaFicticia } from "./test/falsos";

const afirma = (numero: number, parcial: Partial<Afirmacao> = {}): Afirmacao =>
  ({
    numero,
    secao: "resposta",
    texto: `Afirmação número ${numero}.`,
    confianca: "inferido",
    estado: "registrada",
    evidencias: [],
    registrada_ms: numero,
    ...parcial,
  }) as Afirmacao;

const AFIRMACOES: Afirmacao[] = [
  { ...AFIRMACAO_MULTA, texto: "Passada a carência, a multa é de 2%." },
  afirma(2),
  afirma(3, { estado: "confirmada" }),
  afirma(4, { secao: "regras_encontradas", estado: "incorreta", comentario: "Não vale." }),
  afirma(5, { secao: "regras_encontradas", estado: "aprofundar", pedido: "Mostre o perfil." }),
  afirma(6, { secao: "regras_encontradas", estado: "a-revalidar", confianca: "a_confirmar" }),
];

/** Aplica a decisão como o Rust faria, no mínimo que os testes precisam. */
function aplicar(a: Analise, d: DecisaoRevisao): Analise {
  const b = structuredClone(a);
  if (d.tipo === "afirmacao") {
    const x = b.afirmacoes.find((y) => y.numero === d.numero)!;
    Object.assign(
      x,
      d.estado === "incorreta"
        ? { estado: d.estado, comentario: d.comentario }
        : d.estado === "aprofundar"
          ? { estado: d.estado, pedido: d.pedido }
          : { estado: d.estado },
    );
  } else if (d.tipo === "pergunta") {
    Object.assign(b.perguntas.find((p) => p.numero === d.numero)!, d);
  } else if (d.tipo === "termo") {
    b.termos.find((t) => t.numero === d.numero)!.estado = d.estado;
  } else if (d.tipo === "sugestao") {
    b.sugestoes.find((s) => s.numero === d.numero)!.estado = d.estado;
  }
  return b;
}

function montar(parcial: Partial<Analise> = {}, ocupado = false) {
  const decisoes: DecisaoRevisao[] = [];
  const aoAbrirEvidencia = vi.fn<(e: unknown, o: OrigemEvidencia) => void>();
  const aoEnviar = vi.fn();
  function Tela() {
    const [analise, setAnalise] = useState<Analise>(
      vistaFicticia(
        {},
        { estado: "concluida", afirmacoes: AFIRMACOES, ...parcial },
      ).analise,
    );
    const revisao: Revisao = {
      ocupado,
      respostas: { disponivel: true, aoEnviar },
      aoDecidir: (d) => {
        decisoes.push(d);
        setAnalise((a) => aplicar(a, d));
        return Promise.resolve();
      },
    };
    return (
      <DocumentoAoVivo
        vista={vistaFicticia({}, analise)}
        semCabecalho
        aoAbrirEvidencia={aoAbrirEvidencia}
        revisao={revisao}
      />
    );
  }
  render(<Tela />);
  return { decisoes, aoAbrirEvidencia, aoEnviar };
}

const cartao = (n: number) => screen.getByTestId(`afirmacao-${n}`);
const tecla = (key: string, init: KeyboardEventInit = {}, alvo: Element = document.body) =>
  fireEvent.keyDown(alvo, { key, ...init });

describe("resumoRevisao", () => {
  it("conta decididas (confirmada, incorreta) e pendentes (sem decisão, a aprofundar, a revalidar), pela regra da aprovação", () => {
    expect(resumir(AFIRMACOES)).toEqual({
      total: 6,
      confirmadas: 1,
      incorretas: 1,
      aprofundar: 1,
      pendentes: 4,
      decididas: 2,
    });
  });

  it("cada filtro bate com o estado das afirmações", () => {
    expect(contagemPorFiltro(AFIRMACOES)).toEqual({
      todos: 6,
      pendentes: 4,
      confirmadas: 1,
      incorretas: 1,
      aprofundar: 1,
    });
    expect(passaNoFiltro(AFIRMACOES[5], "pendentes")).toBe(true);
    expect(passaNoFiltro(AFIRMACOES[4], "pendentes")).toBe(true);
    expect(passaNoFiltro(AFIRMACOES[2], "pendentes")).toBe(false);
  });
});

describe("Faixa de revisão", () => {
  it("mostra N de M decididos, a barra dividida e os atalhos", () => {
    montar();
    const faixa = screen.getByRole("region", { name: "Progresso da revisão" });
    expect(within(faixa).getByRole("status")).toHaveTextContent("2 de 6 decididos");
    const barra = within(faixa).getByRole("progressbar", { name: "Afirmações decididas" });
    expect(barra).toHaveAttribute("aria-valuenow", "2");
    expect(barra).toHaveAttribute("aria-valuemax", "6");
    expect(barra.querySelectorAll("[data-segmento]")).toHaveLength(4);
    expect(barra.querySelector('[data-segmento="confirmadas"]')).toHaveStyle({ flexGrow: "1" });
    expect(barra.querySelector('[data-segmento="pendentes"]')).toHaveStyle({ flexGrow: "3" });
    const dica = within(faixa).getByLabelText("Atalhos de teclado");
    for (const t of ["J", "K", "C", "X", "A"]) expect(within(dica).getByText(t)).toBeInTheDocument();
  });

  it("atualiza o progresso e as contagens ao decidir", async () => {
    montar();
    fireEvent.click(within(cartao(2)).getByRole("button", { name: "Confirmar" }));
    await waitFor(() =>
      expect(screen.getByRole("status", { name: "" })).toHaveTextContent("3 de 6 decididos"),
    );
    expect(screen.getByRole("button", { name: /^A decidir/ })).toHaveTextContent("3");
    expect(screen.getByRole("button", { name: /^Confirmadas/ })).toHaveTextContent("2");
  });

  it("não aparece sem revisão (análise aprovada ou só leitura)", () => {
    render(
      <DocumentoAoVivo
        vista={vistaFicticia({}, { estado: "aprovada", afirmacoes: AFIRMACOES })}
        semCabecalho
        aoAbrirEvidencia={vi.fn()}
      />,
    );
    expect(screen.queryByRole("region", { name: "Progresso da revisão" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Confirmar" })).not.toBeInTheDocument();
    // O cartão continua mostrando o estado, só leitura.
    expect(screen.getByText("confirmada por você")).toBeInTheDocument();
  });
});

describe("Filtros", () => {
  it("A decidir mostra as sem decisão, a aprofundar e a revalidar; os outros filtram por resultado", () => {
    montar();
    expect(screen.getByRole("button", { name: /^Todos/ })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: /^A decidir/ }));
    expect(screen.getByRole("button", { name: /^A decidir/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getAllByTestId(/^afirmacao-/).map((e) => e.dataset.testid)).toEqual([
      "afirmacao-1",
      "afirmacao-2",
      "afirmacao-5",
      "afirmacao-6",
    ]);

    fireEvent.click(screen.getByRole("button", { name: /^Incorretas/ }));
    expect(screen.getAllByTestId(/^afirmacao-/).map((e) => e.dataset.testid)).toEqual([
      "afirmacao-4",
    ]);
    const faixa = within(screen.getByRole("group", { name: "Filtrar afirmações" }));
    fireEvent.click(faixa.getByRole("button", { name: /^Aprofundar/ }));
    expect(screen.getAllByTestId(/^afirmacao-/).map((e) => e.dataset.testid)).toEqual([
      "afirmacao-5",
    ]);
    fireEvent.click(screen.getByRole("button", { name: /^Confirmadas/ }));
    expect(screen.getAllByTestId(/^afirmacao-/).map((e) => e.dataset.testid)).toEqual([
      "afirmacao-3",
    ]);
    // A seção sem nada no filtro avisa, em vez de sumir.
    expect(screen.getByText("Nenhuma afirmação desta seção neste filtro.")).toBeInTheDocument();
  });

  it("a afirmação decidida sai do filtro A decidir na hora", async () => {
    montar();
    fireEvent.click(screen.getByRole("button", { name: /^A decidir/ }));
    fireEvent.click(within(cartao(2)).getByRole("button", { name: "Confirmar" }));
    await waitFor(() => expect(screen.queryByTestId("afirmacao-2")).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: /^A decidir/ })).toHaveTextContent("3");
  });
});

describe("Sumário", () => {
  it("lista as seções com as pendências e ✓ quando resolvida", () => {
    montar({
      afirmacoes: [
        AFIRMACAO_MULTA,
        afirma(2, { estado: "confirmada" }),
        afirma(3, { secao: "regras_encontradas" }),
        afirma(4, { secao: "regras_encontradas", estado: "a-revalidar" }),
      ],
    });
    const sumario = screen.getByRole("navigation", { name: "Sumário da análise" });
    expect(within(sumario).getByRole("button", { name: "Resposta: 1 pendência" })).toBeInTheDocument();
    expect(
      within(sumario).getByRole("button", { name: "Regras encontradas: 2 pendências" }),
    ).toBeInTheDocument();
    // Sem perguntas nem propostas: as entradas existem, sem marca de pendência.
    expect(within(sumario).getByRole("button", { name: "Perguntas" })).toBeInTheDocument();
  });

  it("marca ✓ na seção sem pendências", () => {
    montar({ afirmacoes: [afirma(1, { estado: "confirmada" }), afirma(2, { estado: "incorreta", comentario: "x" })] });
    const sumario = screen.getByRole("navigation", { name: "Sumário da análise" });
    const item = within(sumario).getByRole("button", { name: "Resposta: resolvida" });
    expect(item).toHaveAttribute("data-resolvida", "true");
    expect(item.querySelector("svg")).not.toBeNull();
  });

  it("clicar rola até a seção e leva o foco ao título", () => {
    const rolar = vi.fn();
    HTMLElement.prototype.scrollIntoView = rolar;
    montar();
    const sumario = screen.getByRole("navigation", { name: "Sumário da análise" });
    fireEvent.click(within(sumario).getByRole("button", { name: /^Regras encontradas/ }));
    const titulo = screen.getByRole("heading", { name: "Regras encontradas" });
    expect(rolar).toHaveBeenCalled();
    expect(rolar.mock.contexts[rolar.mock.contexts.length - 1]).toBe(titulo);
    expect(titulo).toHaveFocus();
  });

  it("conta as perguntas abertas e as propostas pendentes", () => {
    montar({
      perguntas: [
        { numero: 1, texto: "Vale para PJ?", estado: "aberta", registrada_ms: 1 },
        { numero: 2, texto: "E sábado?", estado: "descartada", registrada_ms: 2 },
      ],
      termos: [{ numero: 1, termo: "Carência", definicao: "Dias.", estado: "proposto" }],
    });
    const sumario = screen.getByRole("navigation", { name: "Sumário da análise" });
    expect(within(sumario).getByRole("button", { name: "Perguntas: 1 pendência" })).toBeInTheDocument();
    expect(
      within(sumario).getByRole("button", { name: "Propostas para a base: 1 pendência" }),
    ).toBeInTheDocument();
  });
});

describe("Cartão de afirmação", () => {
  it("mostra número, selo de confiança colorido, texto e ações com rótulo", () => {
    montar();
    const c = within(cartao(1));
    expect(c.getByTitle("Afirmação 1")).toHaveTextContent("1");
    expect(c.getByText("confirmado")).toHaveAttribute("data-tone", "done");
    expect(within(cartao(2)).getByText("inferido")).toHaveAttribute("data-tone", "running");
    expect(within(cartao(6)).getByText("a confirmar")).toHaveAttribute("data-tone", "attention");
    expect(c.getByText("Passada a carência, a multa é de 2%.")).toBeInTheDocument();
    expect(c.getByRole("button", { name: "Confirmar" })).toHaveTextContent("Confirmar");
    expect(c.getByRole("button", { name: "Incorreta" })).toHaveTextContent("Incorreta");
    expect(c.getByRole("button", { name: "Aprofundar" })).toHaveTextContent("Aprofundar");
    // Editar é só o ícone.
    expect(c.getByRole("button", { name: "Editar texto" })).toHaveTextContent("");
    expect(c.getByText("sem decisão")).toBeInTheDocument();
  });

  it("evidências viram chips arquivo:linhas que abrem o código", () => {
    const { aoAbrirEvidencia } = montar();
    fireEvent.click(within(cartao(1)).getByRole("button", { name: "03_pkg_cobranca.sql:71-72" }));
    expect(aoAbrirEvidencia).toHaveBeenCalledWith(
      AFIRMACAO_MULTA.evidencias[0],
      "afirmacao-1",
    );
  });

  it("depois da decisão mostra o selo do resultado, o comentário e Desfazer", async () => {
    const { decisoes } = montar();
    const c = within(cartao(4));
    expect(c.getByText("marcada como incorreta")).toHaveAttribute("data-tone", "error");
    expect(c.getByText("Não vale.")).toBeInTheDocument();
    expect(within(cartao(5)).getByText("Mostre o perfil.")).toBeInTheDocument();
    expect(within(cartao(6)).getByText("a revalidar: o código mudou")).toBeInTheDocument();
    expect(c.queryByText("sem decisão")).not.toBeInTheDocument();

    fireEvent.click(c.getByRole("button", { name: "Desfazer decisão" }));
    await waitFor(() =>
      expect(decisoes[decisoes.length - 1]).toEqual({ tipo: "afirmacao", numero: 4, estado: "registrada" }),
    );
    await waitFor(() => expect(within(cartao(4)).getByText("sem decisão")).toBeInTheDocument());
    expect(within(cartao(4)).queryByRole("button", { name: "Desfazer decisão" })).toBeNull();
  });

  it("confirmar, marcar incorreta com comentário e aprofundar seguem como antes", async () => {
    const { decisoes } = montar();
    fireEvent.click(within(cartao(2)).getByRole("button", { name: "Confirmar" }));
    await waitFor(() => expect(within(cartao(2)).getByText("confirmada por você")).toBeInTheDocument());
    fireEvent.click(within(cartao(1)).getByRole("button", { name: "Incorreta" }));
    const campo = within(cartao(1)).getByLabelText(/Por que está incorreta/);
    fireEvent.change(campo, { target: { value: "Errada." } });
    fireEvent.click(within(cartao(1)).getByRole("button", { name: "Marcar como incorreta" }));
    await waitFor(() =>
      expect(decisoes[decisoes.length - 1]).toEqual({
        tipo: "afirmacao",
        numero: 1,
        estado: "incorreta",
        comentario: "Errada.",
      }),
    );
  });
});

describe("Atalhos de teclado", () => {
  it("J e K navegam entre as afirmações do filtro, com o foco no cartão", () => {
    montar();
    tecla("j");
    expect(cartao(1)).toHaveFocus();
    expect(cartao(1)).toHaveAttribute("aria-current", "true");
    tecla("j");
    expect(cartao(2)).toHaveFocus();
    tecla("k");
    expect(cartao(1)).toHaveFocus();
    // No começo, K fica onde está.
    tecla("k");
    expect(cartao(1)).toHaveFocus();
    // Respeitam o filtro: só as a decidir (1, 2, 5 e 6).
    fireEvent.click(screen.getByRole("button", { name: /^A decidir/ }));
    tecla("j");
    tecla("j");
    expect(cartao(5)).toHaveFocus();
    tecla("j");
    expect(cartao(6)).toHaveFocus();
    tecla("j");
    expect(cartao(6)).toHaveFocus();
  });

  it("C confirma a afirmação atual e passa para a próxima", async () => {
    const { decisoes } = montar();
    tecla("j");
    tecla("c");
    await waitFor(() =>
      expect(decisoes).toEqual([{ tipo: "afirmacao", numero: 1, estado: "confirmada" }]),
    );
    await waitFor(() => expect(cartao(2)).toHaveAttribute("aria-current", "true"));
    expect(cartao(2)).toHaveFocus();
    // Já confirmada: C não repete a decisão.
    tecla("j");
    expect(cartao(3)).toHaveFocus();
    tecla("c");
    expect(decisoes).toHaveLength(1);
  });

  it("X abre o comentário da incorreta e A abre o diálogo de aprofundar", () => {
    montar();
    tecla("j");
    tecla("x");
    const campo = within(cartao(1)).getByLabelText(/Por que está incorreta/);
    expect(campo).toHaveFocus();
    fireEvent.click(within(cartao(1)).getByRole("button", { name: "Cancelar" }));
    tecla("j");
    tecla("a");
    expect(screen.getByRole("dialog", { name: "Aprofundar a afirmação 2" })).toBeInTheDocument();
  });

  it("sem afirmação atual, C, X e A não fazem nada", () => {
    const { decisoes } = montar();
    tecla("c");
    tecla("x");
    tecla("a");
    expect(decisoes).toHaveLength(0);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/Por que está incorreta/)).not.toBeInTheDocument();
  });

  it("não disparam com o foco em campo de texto, no terminal ou com modificador", () => {
    const { decisoes } = montar({
      perguntas: [{ numero: 1, texto: "Vale para PJ?", estado: "aberta", registrada_ms: 1 }],
    });
    tecla("j");
    expect(cartao(1)).toHaveFocus();

    const resposta = screen.getByLabelText("Resposta");
    resposta.focus();
    tecla("j", {}, resposta);
    tecla("c", {}, resposta);
    expect(resposta).toHaveFocus();

    // O xterm digita num textarea escondido dentro do terminal.
    const terminal = document.createElement("div");
    terminal.className = "xterm";
    const escondido = document.createElement("textarea");
    terminal.appendChild(escondido);
    document.body.appendChild(terminal);
    escondido.focus();
    tecla("j", {}, escondido);
    tecla("c", {}, escondido);
    // E o terminal sem textarea (a área do xterm tem o foco).
    const area = document.createElement("div");
    area.className = "xterm-screen";
    terminal.appendChild(area);
    tecla("c", {}, area);
    terminal.remove();

    // Ctrl, Cmd e Alt ficam para o navegador.
    document.body.focus();
    tecla("c", { ctrlKey: true });
    tecla("j", { metaKey: true });
    tecla("c", { altKey: true });

    expect(decisoes).toHaveLength(0);
    expect(cartao(1)).toHaveAttribute("aria-current", "true");
  });

  it("não disparam com um diálogo aberto", () => {
    const { decisoes } = montar();
    tecla("j");
    tecla("a");
    const dialogo = screen.getByRole("dialog");
    tecla("c", {}, within(dialogo).getByRole("button", { name: "Cancelar" }));
    expect(decisoes).toHaveLength(0);
  });
});

describe("Perguntas para o negócio", () => {
  const PERGUNTAS: Analise["perguntas"] = [
    {
      numero: 1,
      texto: "A carência vale para PJ?",
      estado: "aberta",
      registrada_ms: 1,
      para: { slug: "maria-silva", nome: "Maria Silva", papel: "PO" },
    },
    { numero: 2, texto: "E aos sábados?", estado: "aberta", registrada_ms: 2 },
  ];

  it("mostra para quem perguntar, vindo do catálogo; sem pessoa, só o texto", () => {
    montar({ perguntas: PERGUNTAS });
    const p1 = screen.getByTestId("pergunta-1");
    expect(within(p1).getByText("Maria Silva")).toBeInTheDocument();
    expect(p1).toHaveTextContent("Para: Maria Silva (PO)");
    const p2 = screen.getByTestId("pergunta-2");
    expect(p2).not.toHaveTextContent("Para:");
    expect(within(p2).getByText("E aos sábados?")).toBeInTheDocument();
  });

  it("responde no próprio cartão, descarta e envia as respostas ao agente", async () => {
    const { decisoes, aoEnviar } = montar({ perguntas: PERGUNTAS });
    const p1 = screen.getByTestId("pergunta-1");
    const salvar = within(p1).getByRole("button", { name: "Salvar resposta" });
    expect(salvar).toBeDisabled();
    fireEvent.change(within(p1).getByLabelText("Resposta"), { target: { value: "Vale, sim." } });
    fireEvent.click(salvar);
    await waitFor(() =>
      expect(decisoes).toEqual([
        { tipo: "pergunta", numero: 1, estado: "respondida", resposta: "Vale, sim." },
      ]),
    );
    expect(await within(p1).findByText(/Resposta: Vale, sim\./)).toBeInTheDocument();

    fireEvent.click(
      within(screen.getByTestId("pergunta-2")).getByRole("button", { name: "Descartar" }),
    );
    await waitFor(() =>
      expect(decisoes[decisoes.length - 1]).toEqual({ tipo: "pergunta", numero: 2, estado: "descartada" }),
    );

    const enviar = await screen.findByRole("button", { name: "Enviar resposta ao agente" });
    fireEvent.click(enviar);
    expect(aoEnviar).toHaveBeenCalledWith([expect.objectContaining({ numero: 1 })]);
  });
});

describe("Propostas para a base", () => {
  it("reúne termos do glossário e sugestões de alteração, com Aceitar e Recusar", async () => {
    const { decisoes } = montar({
      termos: [{ numero: 1, termo: "Carência", definicao: "Dias sem multa.", estado: "proposto" }],
      sugestoes: [
        {
          numero: 1,
          titulo: "Ajustar a regra RN-COB-001",
          arquivo: "regras/RN-COB-001.md",
          trecho_atual: "a",
          trecho_proposto: "b",
          motivo: "A regra não cita a carência.",
          impacto: "baixo",
          estado: "proposta",
          registrada_ms: 1,
        },
      ],
    });
    const secao = screen.getByRole("region", { name: "Propostas para a base" });
    expect(within(secao).getByText("termo do glossário")).toBeInTheDocument();
    expect(within(secao).getByText("alteração sugerida")).toBeInTheDocument();
    const termo = within(secao).getByRole("group", { name: "Revisão do termo Carência" });
    expect(within(termo).getByRole("button", { name: "Aceitar" })).toHaveTextContent("Aceitar");
    expect(within(termo).getByRole("button", { name: "Recusar" })).toBeInTheDocument();
    expect(within(termo).getByRole("button", { name: "Editar definição" })).toBeInTheDocument();

    fireEvent.click(within(termo).getByRole("button", { name: "Aceitar" }));
    await waitFor(() =>
      expect(decisoes[0]).toEqual({ tipo: "termo", numero: 1, estado: "aceito" }),
    );
    fireEvent.click(
      within(
        within(secao).getByRole("group", { name: "Revisão da sugestão 1" }),
      ).getByRole("button", { name: "Recusar" }),
    );
    await waitFor(() =>
      expect(decisoes[decisoes.length - 1]).toEqual({ tipo: "sugestao", numero: 1, estado: "recusada" }),
    );
    // Recusada, a sugestão sai da lista.
    await waitFor(() =>
      expect(within(secao).queryByText("A regra não cita a carência.")).not.toBeInTheDocument(),
    );
  });
});
