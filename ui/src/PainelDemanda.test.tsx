import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { AvisoTranscriptGuardado, dataDoDia, PainelDemanda } from "./PainelDemanda";
import type { ApiPauta, Pauta } from "./pauta";
import type { ApiRegistros, RegistrosDaDemanda } from "./registros";
import {
  ATA,
  DEMANDA,
  PROJETO,
  apiConhecimentoFalsa,
  demandasFicticias,
  painelFicticio,
} from "./test/conhecimentoFalso";
import { apiRastreabilidadeFalsa, matrizFicticia } from "./test/rastreabilidadeFalsa";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

function pautaFicticia(total: number): Pauta {
  return {
    titulo: "Pauta da D-0001",
    grupos: [],
    total,
    antigas: 0,
    pessoas: [],
    papeis: [],
    premissas: [],
    geradaEm: "2026-11-26",
    emails: [],
  };
}

function apiPautaFalsa(total = 0): ApiPauta & { [k: string]: ReturnType<typeof vi.fn> } {
  return {
    pauta: vi.fn(async () => pautaFicticia(total)),
    definirDestinatario: vi.fn(async () => undefined),
    registrarResposta: vi.fn(),
    escolherDestino: vi.fn(async (nome: string, formato: string) => `/tmp/${nome}.${formato}`),
    exportar: vi.fn(async (_p, _d, _f, destino: string) => destino),
    copiar: vi.fn(async () => undefined),
  };
}

function apiRegistrosFalsa(propostas = 0): ApiRegistros {
  const aba: RegistrosDaDemanda = {
    registros: [],
    propostas: Array.from({ length: propostas }, (_, n) => ({
      referencia: `A-0003/${n + 1}`,
      categoria: "premissa" as const,
      fonte: { tipo: "analise" as const, id: "A-0003", titulo: "Parcelamento" },
      dados: { titulo: `Premissa ${n + 1}` },
    })),
    pessoas: [],
    pasta: `${PROJETO}/demandas/D-0001-parcelamento-pj/registros`,
  };
  return {
    registros: vi.fn(async () => aba),
    criar: vi.fn(),
    editar: vi.fn(),
    validarPremissa: vi.fn(),
    aceitarProposta: vi.fn(),
    recusarProposta: vi.fn(),
  };
}

function montar(
  falsa = apiConhecimentoFalsa(),
  extras: Partial<Parameters<typeof PainelDemanda>[0]> = {},
) {
  const navegar = vi.fn();
  render(
    <PainelDemanda
      api={falsa.api}
      projeto={PROJETO}
      demanda="D-0001"
      navegar={navegar}
      {...extras}
    />,
  );
  return { ...falsa, navegar };
}

async function escolherNoMenu(menu: string, item: string) {
  fireEvent.click(await screen.findByRole("button", { name: menu }));
  fireEvent.click(screen.getByRole("menuitem", { name: item }));
}

describe("PainelDemanda", () => {
  it("mostra o cabeçalho com a trilha, os metadados e a ação principal", async () => {
    const registros = apiRegistrosFalsa(0);
    const { api, navegar } = montar(apiConhecimentoFalsa(), { apiRegistros: registros });
    expect(await screen.findByRole("heading", { level: 1, name: "Parcelamento PJ" })).toBeInTheDocument();
    expect(api.painelDemanda).toHaveBeenCalledWith(PROJETO, "D-0001");

    const trilha = screen.getByRole("navigation", { name: "Trilha" });
    expect(trilha).toHaveTextContent("Demandas›D-0001");
    fireEvent.click(within(trilha).getByRole("button", { name: "Demandas" }));
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "demandas", projeto: PROJETO });

    expect(screen.getByText("aberta")).toHaveAttribute("data-tom", "andamento");
    // O tipo da demanda (#358), do frontmatter.
    expect(screen.getByTitle("Tipo da demanda")).toHaveTextContent("Nova funcionalidade");
    // Sem o link da issue, a chave fica só como texto.
    expect(screen.getByText("FAT-12")).toBeInTheDocument();
    expect(screen.getByText("criada em 01/11/2026")).toBeInTheDocument();

    // Uma ação principal; os outros tipos no menu "Nova análise".
    const acoes = screen.getByRole("group", { name: "Ações da demanda" });
    expect(within(acoes).getByRole("button", { name: "Analisar impacto" })).toHaveClass(
      "btn-primary",
    );
    fireEvent.click(within(acoes).getByRole("button", { name: "Nova análise" }));
    const tipos = screen.getByRole("menu", { name: "Nova análise" });
    expect(within(tipos).getAllByRole("menuitem").map((b) => b.textContent)).toEqual([
      "Perguntar ao sistema",
      "Explicar funcionalidade",
      "Analisar reunião",
      "Revisar requisito",
      "Histórias e critérios",
      "Cenários de teste",
      "Protótipo de telas",
    ]);

    await escolherNoMenu("Mais ações", "Abrir demanda.md");
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "base", documento: DEMANDA });
    // O caminho interno da pasta (nem a dos registros) não aparece na tela.
    await screen.findByRole("region", { name: "Premissas, decisões, riscos e restrições" });
    await waitFor(() => expect(registros.registros).toHaveBeenCalled());
    expect(screen.queryByText(/demandas\/D-0001/)).toBeNull();
  });

  it("abre a issue da chave no navegador quando a demanda veio do rastreador", async () => {
    const falsa = apiConhecimentoFalsa();
    const url = "https://github.com/o/r/issues/12";
    falsa.api.painelDemanda.mockResolvedValue(
      painelFicticio({ demanda: { ...demandasFicticias()[0], origem: url } }),
    );
    const abrirLink = vi.fn(async () => undefined);
    montar(falsa, { abrirLink });
    fireEvent.click(await screen.findByRole("button", { name: /FAT-12/ }));
    expect(abrirLink).toHaveBeenCalledWith(PROJETO, url);
  });

  it("mostra a linha do tempo em ordem, com data, link e selo", async () => {
    const { navegar } = montar();
    const linha = await screen.findByRole("region", { name: "Linha do tempo" });
    const eventos = Array.from(linha.querySelectorAll("[data-tipo]"));
    expect(eventos.map((i) => i.getAttribute("data-tipo"))).toEqual([
      "reuniao",
      "analise",
      "sessao",
      "reuniao",
    ]);
    expect(eventos[0]).toHaveTextContent("03/11/2026");
    expect(eventos[0]).toHaveTextContent("Reunião R-0001: Levantar o pedido");
    expect(within(eventos[0] as HTMLElement).getByText("ata rascunho")).toHaveAttribute(
      "data-tom",
      "atencao",
    );
    expect(within(eventos[3] as HTMLElement).getByText("ata aprovada")).toHaveAttribute(
      "data-tom",
      "concluido",
    );
    expect(within(eventos[1] as HTMLElement).getByText("em andamento")).toBeInTheDocument();
    expect(within(eventos[2] as HTMLElement).getByText("aguardando você")).toHaveAttribute(
      "title",
      expect.stringMatching(/Aguardando você/),
    );
    // Sem transcript guardado, nenhum aviso.
    expect(screen.queryByText(/Transcript guardado/)).toBeNull();

    fireEvent.click(
      within(eventos[1] as HTMLElement).getByRole("button", { name: "A-0003 · Analisar impacto" }),
    );
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      aberta: { id: "A-0003", abertura: { tipo: "reabrir" } },
    });
    fireEvent.click(
      within(eventos[3] as HTMLElement).getByRole("button", {
        name: "Reunião R-0002: Validar as regras do parcelamento",
      }),
    );
    expect(navegar).toHaveBeenLastCalledWith({ tipo: "base", documento: ATA });
  });

  it("mostra as pendências e as dúvidas em seções e resume as vazias no fim", async () => {
    montar();
    const pendencias = await screen.findByRole("region", { name: "Pendências" });
    expect(within(pendencias).getByText("P-1")).toBeInTheDocument();
    expect(pendencias).toHaveTextContent("Enviar a tabela de juros do parcelamento");
    expect(pendencias).toHaveTextContent("responsável: Ana (PO)");
    expect(pendencias).toHaveTextContent("prazo: 20/11/2026");
    expect(pendencias).toHaveTextContent("reunião R-0002 · 05/11/2026");
    const duvidas = screen.getByRole("region", { name: "Dúvidas em aberto" });
    expect(within(duvidas).getByText("DV-1")).toBeInTheDocument();
    // Sem próximos passos: nenhuma seção, só a frase no fim.
    expect(screen.queryByRole("region", { name: "Próximos passos" })).toBeNull();
    expect(screen.getByText("Sem próximos passos nas atas aprovadas.")).toBeInTheDocument();
  });

  it("junta numa frase só as pendências, as dúvidas e os próximos passos vazios", async () => {
    const falsa = apiConhecimentoFalsa();
    falsa.api.painelDemanda.mockResolvedValue(
      painelFicticio({ pendencias: [], duvidas: [], proximosPassos: [] }),
    );
    montar(falsa);
    expect(
      await screen.findByText("Sem pendências, dúvidas ou próximos passos nas atas aprovadas."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Pendências" })).toBeNull();
    expect(screen.queryByRole("region", { name: "Dúvidas em aberto" })).toBeNull();
  });

  it("mostra só os contadores com valor e cada um leva à sua seção", async () => {
    const falsa = apiConhecimentoFalsa();
    const painel = painelFicticio();
    falsa.api.painelDemanda.mockResolvedValue({
      ...painel,
      linhaDoTempo: painel.linhaDoTempo.map((i) =>
        i.tipo === "analise" ? { ...i, estado: "concluida" as const } : i,
      ),
    });
    const pauta = apiPautaFalsa(14);
    montar(falsa, {
      apiPauta: pauta,
      apiRegistros: apiRegistrosFalsa(0),
      apiRastreabilidade: apiRastreabilidadeFalsa(),
    });
    const resumo = await screen.findByRole("navigation", { name: "Resumo" });
    await waitFor(() => expect(within(resumo).getAllByRole("button")).toHaveLength(3));
    const contadores = within(resumo)
      .getAllByRole("button")
      .map((b) => b.textContent);
    // Sem propostas do agente, o contador delas não aparece.
    expect(contadores).toEqual([
      "14perguntas em aberto",
      "3lacunas na rastreabilidade",
      "1análise aguardando revisão",
    ]);
    expect(pauta.pauta).toHaveBeenCalledWith(PROJETO, "D-0001");

    fireEvent.click(within(resumo).getByRole("button", { name: /perguntas em aberto/ }));
    expect(document.activeElement).toHaveClass("painel-demanda-ancora");
    expect(document.activeElement).toHaveTextContent(/^Pauta de perguntas/);
    fireEvent.click(within(resumo).getByRole("button", { name: /lacunas na rastreabilidade/ }));
    expect(document.activeElement).toContainElement(
      screen.getByRole("region", { name: "Matriz de rastreabilidade" }),
    );
    fireEvent.click(within(resumo).getByRole("button", { name: /análise aguardando revisão/ }));
    expect(document.activeElement).toContainElement(
      screen.getByRole("region", { name: "Linha do tempo" }),
    );
  });

  it("conta as propostas do agente e leva aos registros", async () => {
    montar(apiConhecimentoFalsa(), { apiRegistros: apiRegistrosFalsa(2) });
    const botao = await screen.findByRole("button", {
      name: "2 propostas do agente para revisar",
    });
    expect(botao).toHaveAttribute("data-tom", "atencao");
    fireEvent.click(botao);
    expect(document.activeElement).toHaveClass("painel-demanda-ancora");
    expect(document.activeElement).toHaveTextContent(/^Premissas, decisões, riscos e restrições/);
  });

  it("sem nada a contar, não há resumo", async () => {
    montar();
    await screen.findByRole("region", { name: "Linha do tempo" });
    expect(screen.queryByRole("navigation", { name: "Resumo" })).toBeNull();
  });

  it("exporta a matriz e a pauta pelo menu ⋯", async () => {
    const rastreabilidade = apiRastreabilidadeFalsa();
    const pauta = apiPautaFalsa(2);
    montar(apiConhecimentoFalsa(), { apiRastreabilidade: rastreabilidade, apiPauta: pauta });
    await screen.findByRole("table", { name: "Matriz por história" });
    await screen.findByRole("button", { name: /perguntas em aberto/ });

    await escolherNoMenu("Mais ações", "Exportar a matriz para o Excel");
    expect(await screen.findByText("Matriz exportada em /tmp/matriz-D-0001.xlsx.")).toBeInTheDocument();
    expect(rastreabilidade.exportar).toHaveBeenCalledWith(
      PROJETO,
      "D-0001",
      "xlsx",
      "/tmp/matriz-D-0001.xlsx",
    );

    await escolherNoMenu("Mais ações", "Exportar a pauta para o Word");
    expect(await screen.findByText("Pauta exportada em /tmp/pauta-D-0001.word.")).toBeInTheDocument();
    expect(pauta.exportar).toHaveBeenCalledWith(PROJETO, "D-0001", "word", "/tmp/pauta-D-0001.word");
  });

  it("cria a análise já ligada à demanda", async () => {
    const { api, navegar } = montar();
    fireEvent.click(await screen.findByRole("button", { name: "Analisar impacto" }));
    const form = screen.getByRole("form", { name: "Analisar impacto" });
    expect(form).toHaveTextContent("A análise fica ligada à demanda D-0001.");
    // Sem escolha de projeto, issue ou outra demanda.
    expect(within(form).queryByLabelText(/Issue do projeto/)).toBeNull();
    expect(within(form).queryByLabelText("Demanda (opcional)")).toBeNull();
    fireEvent.change(within(form).getByLabelText("Texto da demanda"), {
      target: { value: "Permitir parcelamento em até 3 vezes." },
    });
    const iniciar = within(form).getByRole("button", { name: "Iniciar a análise" });
    await waitFor(() => expect(iniciar).toBeEnabled());
    fireEvent.click(iniciar);
    await waitFor(() =>
      expect(api.criarAnalise).toHaveBeenCalledWith(
        PROJETO,
        "impacto",
        "Permitir parcelamento em até 3 vezes.",
        undefined,
        "D-0001",
      ),
    );
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      aberta: {
        id: "A-0003",
        abertura: { tipo: "nova", promptInicial: "Inicie a análise A-0003 pelo ctower" },
      },
    });
  });

  it("abre o tipo escolhido em Nova análise", async () => {
    montar();
    await escolherNoMenu("Nova análise", "Perguntar ao sistema");
    const secao = screen.getByRole("region", { name: "Nova análise da demanda" });
    expect(within(secao).getByRole("form", { name: "Perguntar ao sistema" })).toBeInTheDocument();
  });

  it("encerra a demanda pelo menu ⋯ e, encerrada, não cria análise", async () => {
    const falsa = apiConhecimentoFalsa();
    const { api } = montar(falsa);
    await escolherNoMenu("Mais ações", "Encerrar a demanda");
    await waitFor(() => expect(api.encerrarDemanda).toHaveBeenCalledWith(PROJETO, "D-0001"));

    falsa.api.painelDemanda.mockResolvedValue(
      painelFicticio({ demanda: { ...demandasFicticias()[0], status: "encerrada" } }),
    );
    // Relê quando a base muda.
    falsa.avisar();
    const reabrir = await screen.findByRole("button", { name: "Reabrir a demanda" });
    expect(reabrir).toHaveClass("btn-primary");
    expect(screen.queryByRole("button", { name: "Analisar impacto" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Nova análise" })).toBeNull();
    expect(screen.getByText(/Reabra a demanda para criar/)).toBeInTheDocument();
  });

  it("mostra a falha ao ler a demanda", async () => {
    const falsa = apiConhecimentoFalsa();
    falsa.api.painelDemanda.mockRejectedValueOnce("a demanda D-0001 não existe");
    montar(falsa);
    expect(await screen.findByRole("alert")).toHaveTextContent("a demanda D-0001 não existe");
  });

  it("mostra a matriz de rastreabilidade da demanda (#121)", async () => {
    const rastreabilidade = apiRastreabilidadeFalsa();
    montar(apiConhecimentoFalsa(), { apiRastreabilidade: rastreabilidade });
    const secao = await screen.findByRole("region", { name: "Matriz de rastreabilidade" });
    expect(
      await within(secao).findByRole("table", { name: "Matriz por história" }),
    ).toHaveTextContent("HU-0001");
    expect(rastreabilidade.matriz).toHaveBeenCalledWith(PROJETO, "D-0001");
  });

  it("sem histórias, a matriz leva a gerar as histórias da demanda", async () => {
    const rastreabilidade = apiRastreabilidadeFalsa();
    rastreabilidade.matriz.mockResolvedValue({ ...matrizFicticia(), historias: [] });
    montar(apiConhecimentoFalsa(), { apiRastreabilidade: rastreabilidade });
    const secao = await screen.findByRole("region", { name: "Matriz de rastreabilidade" });
    fireEvent.click(await within(secao).findByRole("button", { name: "Gerar histórias e critérios" }));
    const nova = screen.getByRole("region", { name: "Nova análise da demanda" });
    expect(within(nova).getByRole("form", { name: "Histórias e critérios" })).toBeInTheDocument();
  });
});

describe("dataDoDia", () => {
  it("mostra a data no formato do Brasil e deixa o resto como veio", () => {
    expect(dataDoDia("2026-11-05")).toBe("05/11/2026");
    expect(dataDoDia("ontem")).toBe("ontem");
  });
});

describe("AvisoTranscriptGuardado", () => {
  it("mostra a data em que o transcript é apagado e destaca quando vence em breve", () => {
    const { rerender } = render(
      <AvisoTranscriptGuardado guardado={{ apagaEm: "2026-12-10", venceEmBreve: false }} />,
    );
    expect(screen.getByRole("note")).toHaveTextContent(
      "Transcript guardado sem aprovação, apagado em 2026-12-10.",
    );
    rerender(<AvisoTranscriptGuardado guardado={{ apagaEm: "2026-11-18", venceEmBreve: true }} />);
    expect(screen.getByRole("alert")).toHaveAttribute("data-tone", "attention");
  });
});
