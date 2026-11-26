import { invoke } from "@tauri-apps/api/core";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Busca, Encontrado } from "./conhecimento";
import { TelaBase } from "./TelaBase";
import {
  ANALISE,
  ATA,
  PROJETO,
  REGRA,
  TERMO,
  apiConhecimentoFalsa,
  visaoFicticia,
} from "./test/conhecimentoFalso";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

const PROJETOS = [{ id: PROJETO, nome: "Faturamento Exemplo" }];

const encontrado = (parcial: Partial<Encontrado> & Pick<Encontrado, "caminho">): Encontrado => ({
  projeto: PROJETO,
  tipo: "regra",
  identificador: null,
  titulo: "",
  area: null,
  status: "confirmada",
  trecho: "",
  ...parcial,
});

/** Três regras em duas áreas (uma a revalidar), um termo, uma análise e uma ata. */
function base(): Busca {
  const resultados: Encontrado[] = [
    encontrado({
      caminho: REGRA,
      identificador: "RN-COB-001",
      titulo: "**Carência.** Enquanto os dias forem menores, não há multa",
      area: "cobranca",
      trecho: "…a **multa** é de 2%…",
    }),
    encontrado({
      caminho: `${PROJETO}/regras/RN-COB-002.md`,
      identificador: "RN-COB-002",
      titulo: "Juros contam a partir do primeiro dia",
      area: "cobranca",
      status: "a-revalidar",
    }),
    encontrado({
      caminho: `${PROJETO}/regras/RN-PAR-001.md`,
      identificador: "RN-PAR-001",
      titulo: "Parcelamento em até 3 vezes",
      area: "multas-por-atraso",
    }),
    encontrado({
      caminho: TERMO,
      tipo: "termo",
      titulo: "Carência",
      area: "cobranca",
      status: null,
    }),
    encontrado({
      caminho: ANALISE,
      tipo: "pergunta",
      identificador: "A-0001",
      titulo: "Multa por atraso",
      status: "aprovada",
    }),
    encontrado({
      caminho: ATA,
      tipo: "ata",
      titulo: "Validar as regras do parcelamento",
      status: "aprovada",
    }),
  ];
  return { resultados, total: resultados.length, deslocamento: 0, qualquerPalavra: false };
}

function montar(opcoes: { documentoInicial?: string; vazia?: boolean } = {}) {
  const falsa = apiConhecimentoFalsa();
  falsa.api.buscar.mockImplementation(() =>
    Promise.resolve(
      opcoes.vazia ? { resultados: [], total: 0, deslocamento: 0, qualquerPalavra: false } : base(),
    ),
  );
  const navegar = vi.fn();
  render(
    <TelaBase
      api={falsa.api}
      projetos={PROJETOS}
      projetoInicial={PROJETO}
      documentoInicial={opcoes.documentoInicial}
      navegar={navegar}
    />,
  );
  return { ...falsa, navegar };
}

const aba = (nome: RegExp | string) => screen.findByRole("tab", { name: nome });

async function buscar(texto: string) {
  fireEvent.change(screen.getByRole("searchbox", { name: "Buscar" }), {
    target: { value: texto },
  });
  fireEvent.click(screen.getByRole("button", { name: "Buscar" }));
}

describe("TelaBase (#328)", () => {
  it("tem as abas Regras, Glossário e Documentos, com a contagem de cada uma", async () => {
    montar();
    expect(await aba(/^Regras\s*3$/)).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: /^Glossário\s*1$/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /^Documentos\s*2$/ })).toBeInTheDocument();
  });

  it("a busca vale para as três abas e mostra quantos resultados há em cada uma", async () => {
    const { api } = montar();
    await aba(/^Regras/);
    await buscar("multa");
    await waitFor(() =>
      expect(api.buscar).toHaveBeenCalledWith("multa", {
        projeto: PROJETO,
        tipos: [],
        deslocamento: 0,
      }),
    );
    const resultados = screen.getByRole("region", { name: "Resultados" });
    expect(
      await within(resultados).findByText(/3 resultados em Regras para "multa"/),
    ).toBeInTheDocument();
    // As palavras encontradas ficam destacadas.
    expect(resultados.querySelectorAll("mark")).toHaveLength(1);

    fireEvent.click(screen.getByRole("tab", { name: /^Glossário/ }));
    expect(within(resultados).getByRole("button", { name: "Carência" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: /^Documentos/ }));
    expect(
      within(resultados).getByRole("button", { name: "Multa por atraso" }),
    ).toBeInTheDocument();
  });

  it("agrupa por área com o nome legível e põe a regra a revalidar primeiro", async () => {
    montar();
    const resultados = await screen.findByRole("region", { name: "Resultados" });
    const cobranca = await within(resultados).findByRole("heading", { name: /^Cobrança/ });
    // Área não declarada no projeto.md: o identificador vira nome legível.
    expect(
      within(resultados).getByRole("heading", { name: /^Multas por atraso/ }),
    ).toBeInTheDocument();
    expect(resultados).not.toHaveTextContent("multas-por-atraso");
    const itens = within(cobranca.closest(".grupo-base") as HTMLElement).getAllByRole("listitem");
    expect(itens[0]).toHaveTextContent("RN-COB-002");
    expect(itens[0]).toHaveAttribute("data-status", "a-revalidar");
    expect(within(itens[0]).getByText("a revalidar")).toHaveAttribute("data-tom", "atencao");
    expect(within(itens[1]).getByText("confirmada")).toHaveAttribute("data-tom", "concluido");
  });

  it("mostra o título limpo, sem a marcação de Markdown", async () => {
    montar();
    const resultados = await screen.findByRole("region", { name: "Resultados" });
    expect(await within(resultados).findByRole("button", { name: "Carência" })).toBeInTheDocument();
    expect(resultados).not.toHaveTextContent("**");
  });

  it("filtra por área e por situação, com “A revalidar” sempre à vista", async () => {
    montar();
    const resultados = await screen.findByRole("region", { name: "Resultados" });
    await within(resultados).findByRole("heading", { name: /^Cobrança/ });

    const chip = screen.getByRole("button", { name: /^A revalidar\s*1$/ });
    fireEvent.click(chip);
    expect(chip).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("combobox", { name: "Situação" })).toHaveValue("a-revalidar");
    expect(within(resultados).getAllByRole("listitem")).toHaveLength(1);
    expect(within(resultados).getByText("RN-COB-002")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Limpar filtros" }));
    fireEvent.change(screen.getByRole("combobox", { name: "Área" }), {
      target: { value: "multas-por-atraso" },
    });
    expect(within(resultados).getAllByRole("listitem")).toHaveLength(1);
    expect(within(resultados).getByText("RN-PAR-001")).toBeInTheDocument();
    // A contagem das abas acompanha o filtro.
    expect(screen.getByRole("tab", { name: /^Regras\s*1$/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /^Glossário\s*0$/ })).toBeInTheDocument();
  });

  it("busca sem resultado leva a “Perguntar ao agente”", async () => {
    const { navegar } = montar({ vazia: true });
    await buscar("zebrafish");
    const resultados = screen.getByRole("region", { name: "Resultados" });
    expect(
      await within(resultados).findByText(/Nada encontrado em Regras para "zebrafish"/),
    ).toBeInTheDocument();
    fireEvent.click(within(resultados).getByRole("button", { name: 'Perguntar: "zebrafish"' }));
    expect(navegar).toHaveBeenCalledWith({
      tipo: "analise",
      projeto: PROJETO,
      pergunta: "zebrafish",
    });
  });

  it("abre a regra ao lado da lista: o texto primeiro e os detalhes do arquivo fechados", async () => {
    vi.mocked(invoke).mockResolvedValue({
      projeto: PROJETO,
      alvo: { tipo: "regra", valor: "RN-COB-001" },
      regra: null,
      arquivos: ["backend/src/main/java/exemplo/CobrancaService.java"],
      regras: [],
      demandas: [],
      analises: [],
      criterios: [],
      cenarios: [],
    });
    const { api, navegar } = montar();
    const resultados = await screen.findByRole("region", { name: "Resultados" });
    fireEvent.click(await within(resultados).findByRole("button", { name: "Carência" }));
    const doc = await screen.findByRole("article", {
      name: "Passada a carência, a multa é de 2% sobre o valor original",
    });
    expect(api.abrirDocumento).toHaveBeenCalledWith(REGRA);
    // O Markdown aparece como texto, nunca como elemento.
    expect(within(doc).getByText("multa", { selector: "strong" })).toBeInTheDocument();
    expect(doc.querySelector("script")).toBeNull();
    expect(doc).toHaveTextContent('<script>alert("x")</script>');

    // Ordem: texto, "Onde está no código", "Se esta regra mudar", ligações, detalhes.
    const codigo = await within(doc).findByRole("region", { name: "Onde está no código" });
    const mudar = within(doc).getByRole("region", { name: "O que depende desta regra" });
    expect(within(mudar).getByRole("heading", { name: "Se esta regra mudar" })).toBeInTheDocument();
    const texto = doc.querySelector(".markdown")!;
    const detalhes = doc.querySelector("details.detalhes-arquivo") as HTMLDetailsElement;
    const antes = (a: Node, b: Node) =>
      Boolean(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);
    expect(antes(texto, codigo)).toBe(true);
    expect(antes(codigo, mudar)).toBe(true);
    expect(antes(mudar, detalhes)).toBe(true);
    // O caminho completo só aparece em "Detalhes do arquivo", fechado; no código, o nome.
    expect(detalhes.open).toBe(false);
    expect(within(detalhes).getByText(REGRA)).toBeInTheDocument();
    expect(within(codigo).getByText("CobrancaService.java")).toHaveAttribute(
      "title",
      "backend/src/main/java/exemplo/CobrancaService.java",
    );
    // A área vem com o nome legível, não com o identificador.
    expect(doc).toHaveTextContent("Cobrança");

    fireEvent.click(within(doc).getByRole("button", { name: "Abrir em aba" }));
    expect(navegar).toHaveBeenCalledWith({ tipo: "base", documento: REGRA });
    vi.mocked(invoke).mockReset();
  });

  it("divide a largura com a lista e não deixa nada estourar na horizontal", async () => {
    montar();
    const resultados = await screen.findByRole("region", { name: "Resultados" });
    fireEvent.click(await within(resultados).findByRole("button", { name: "Carência" }));
    await screen.findByRole("article");
    const colunas = document.querySelector(".base-colunas")!;
    expect(colunas.children).toHaveLength(2);
    expect(colunas.children[0]).toHaveClass("resultados-base");
    expect(colunas.children[1]).toHaveClass("documento-aberto");
  });

  it("a conferência das evidências fica no cabeçalho da Base", async () => {
    const { api } = montar();
    expect(await screen.findByText(/3 evidências conferidas às \d\d:\d\d/)).toBeInTheDocument();
    expect(api.revalidar).toHaveBeenCalledWith(PROJETO);
    // A revalidação mudou algo: a lista é relida.
    await waitFor(() => expect(api.visaoProjeto).toHaveBeenCalledTimes(2));
    expect(await screen.findByText(/passaram a revalidar: RN-COB-002/)).toBeInTheDocument();

    const botao = screen.getByRole("button", { name: "Conferir de novo" });
    await waitFor(() => expect(botao).toBeEnabled());
    fireEvent.click(botao);
    await waitFor(() => expect(api.revalidar).toHaveBeenCalledTimes(2));
  });

  it("abre e fecha “O que mudou no código” do cabeçalho", async () => {
    const falsa = apiConhecimentoFalsa();
    const relatorio = vi.fn().mockRejectedValue("informe `de`");
    Object.assign(falsa.api, { mudancas: { relatorio, propostas: vi.fn().mockResolvedValue([]) } });
    render(
      <TelaBase api={falsa.api} projetos={PROJETOS} projetoInicial={PROJETO} navegar={vi.fn()} />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "O que mudou no código" }));
    const painel = screen.getByRole("region", { name: "O que mudou no código" });
    await waitFor(() => expect(relatorio).toHaveBeenCalledWith(PROJETO, null, null));
    fireEvent.click(within(painel).getByRole("button", { name: "Fechar" }));
    expect(screen.queryByRole("region", { name: "O que mudou no código" })).toBeNull();
  });

  it("documento com problema aparece num aviso, com o erro em português e a ação de abrir", async () => {
    const falsa = apiConhecimentoFalsa();
    const caminho = `${PROJETO}/regras/RN-MUL-999.md`;
    falsa.api.visaoProjeto.mockResolvedValue(
      visaoFicticia({
        problemas: [{ caminho, erro: "did not find expected key while parsing a block mapping" }],
      }),
    );
    const navegar = vi.fn();
    render(
      <TelaBase api={falsa.api} projetos={PROJETOS} projetoInicial={PROJETO} navegar={navegar} />,
    );
    const regiao = await screen.findByRole("region", { name: "Documentos com problema" });
    const aviso = within(regiao).getByRole("alert");
    expect(aviso).toHaveTextContent("1 documento com problema");
    expect(aviso).toHaveTextContent("RN-MUL-999.md");
    expect(aviso).toHaveTextContent(/estrutura incompleta ou mal formada/);
    fireEvent.click(within(aviso).getByRole("button", { name: "Abrir para corrigir" }));
    expect(navegar).toHaveBeenCalledWith({ tipo: "base", documento: caminho });
  });

  it("relê a lista e o documento quando a base muda fora do app", async () => {
    const { api, avisar } = montar();
    const resultados = await screen.findByRole("region", { name: "Resultados" });
    fireEvent.click(await within(resultados).findByRole("button", { name: "Carência" }));
    await screen.findByRole("article");
    const buscas = api.buscar.mock.calls.length;
    const aberturas = api.abrirDocumento.mock.calls.length;
    await waitFor(() => expect(api.vigiarBase).toHaveBeenCalled());
    act(() => avisar());
    await waitFor(() => expect(api.buscar.mock.calls.length).toBeGreaterThan(buscas));
    await waitFor(() => expect(api.abrirDocumento.mock.calls.length).toBeGreaterThan(aberturas));
  });

  it("lê todas as páginas da busca, não só as 20 primeiras", async () => {
    const falsa = apiConhecimentoFalsa();
    const regras = Array.from({ length: 25 }, (_, i) =>
      encontrado({
        caminho: `${PROJETO}/regras/RN-X-${String(i).padStart(3, "0")}.md`,
        identificador: `RN-X-${String(i).padStart(3, "0")}`,
        titulo: `Regra ${i}`,
        area: "cobranca",
      }),
    );
    falsa.api.buscar.mockImplementation(((_t: string, filtros?: { deslocamento?: number }) => {
      const de = filtros?.deslocamento ?? 0;
      return Promise.resolve({
        resultados: regras.slice(de, de + 20),
        total: regras.length,
        deslocamento: de,
        qualquerPalavra: false,
      });
    }) as unknown as () => Promise<Busca>);
    render(
      <TelaBase api={falsa.api} projetos={PROJETOS} projetoInicial={PROJETO} navegar={vi.fn()} />,
    );
    expect(await screen.findByRole("tab", { name: /^Regras\s*25$/ })).toBeInTheDocument();
    expect(falsa.api.buscar).toHaveBeenCalledWith("", {
      projeto: PROJETO,
      tipos: [],
      deslocamento: 20,
    });
  });

  describe("aba de um documento", () => {
    it("mostra só o documento, sem a busca, e os links entre documentos são navegáveis", async () => {
      const { api, navegar } = montar({ documentoInicial: REGRA });
      const doc = await screen.findByRole("article", { name: /Passada a carência/ });
      expect(screen.queryByRole("search", { name: "Busca na base" })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Abrir em aba" })).not.toBeInTheDocument();
      expect(api.revalidar).not.toHaveBeenCalled();

      fireEvent.click(within(doc).getByRole("link", { name: "termo" }));
      await waitFor(() => expect(api.abrirDocumento).toHaveBeenLastCalledWith(TERMO));

      fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
      const regra = await screen.findByRole("article", { name: /Passada a carência/ });
      // Ligação pelo campo `origem`: a análise que originou a regra.
      const ligacoes = within(regra).getByRole("navigation", { name: "Ligações" });
      fireEvent.click(within(ligacoes).getByRole("button", { name: "Multa por atraso" }));
      const analise = await screen.findByRole("article", { name: "Multa por atraso" });
      expect(api.abrirDocumento).toHaveBeenLastCalledWith(ANALISE);
      expect(within(analise).getByText("Citado por")).toBeInTheDocument();

      fireEvent.click(within(analise).getByRole("button", { name: "Abrir na tela da análise" }));
      expect(navegar).toHaveBeenCalledWith({
        tipo: "analise",
        projeto: PROJETO,
        aberta: { id: "A-0001", abertura: { tipo: "reabrir" } },
      });
    });
  });
});
