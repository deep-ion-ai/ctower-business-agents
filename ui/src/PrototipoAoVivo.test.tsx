import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Revisao } from "./AcoesRevisao";
import type { TelaPrototipo } from "./analise";
import { PrototipoAoVivo } from "./PrototipoAoVivo";
import { comEstado, type ApiVisualizador } from "./visualizadorPrototipo";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));

/** HTML de uma tela como o app grava (#385), com os quatro estados. */
function html(tela: string): string {
  return (
    `<!doctype html><html lang="pt-BR"><head><meta charset="utf-8">` +
    `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'">` +
    `<title>${tela}</title></head><body data-tela="${tela}">` +
    `<header class="prototipo-cabecalho"><nav class="prototipo-estados">` +
    `<a href="#estado-vazio">vazio</a> <a href="#estado-erro">erro de validação</a></nav></header>` +
    `<main class="prototipo-tela">` +
    `<section id="estado-vazio" data-estado="vazio"><p data-elemento="E-1">Número de parcelas</p></section>` +
    `<section id="estado-preenchido" data-estado="preenchido"><p>3 parcelas</p></section>` +
    `<section id="estado-erro" data-estado="erro"><p class="erro">Parcela abaixo do mínimo</p></section>` +
    `<section id="estado-sucesso" data-estado="sucesso"><p class="sucesso">Parcelamento registrado</p></section>` +
    `</main></body></html>`
  );
}

/** Telas fictícias do "Faturamento Exemplo" (parcelamento para pessoa física). */
function telas(): TelaPrototipo[] {
  return [
    {
      numero: 1,
      nome: "Parcelar fatura",
      mudanca: "alterada",
      motivo: "O parcelamento passa a aceitar pessoa física",
      fundamentos: [{ tipo: "regra", id: "RN-FAT-008", titulo: "Quantidade de parcelas" }],
      elementos: [
        {
          numero: 1,
          tipo: "campo",
          nome: "Número de parcelas",
          descricao: "De 1 a 3",
          fundamentos: [{ tipo: "regra", id: "RN-FAT-008", titulo: "Quantidade de parcelas" }],
          criterios: [
            {
              analise: "A-0002",
              historia: "HU-0001",
              criterio: 2,
              texto: "Dado uma fatura PF, quando pede 4 parcelas, então o sistema recusa",
            },
          ],
          registrado_ms: 1,
        },
        {
          numero: 2,
          tipo: "mensagem",
          nome: "Parcela abaixo do mínimo",
          registrado_ms: 1,
        },
      ],
      html: "analises/A-0003/prototipo/TL-1.html",
      registrada_ms: 1,
    },
    {
      numero: 2,
      nome: "Consultar parcelas",
      mudanca: "nova",
      motivo: "O cliente acompanha as parcelas geradas",
      elementos: [],
      registrada_ms: 2,
    },
  ];
}

function api(): ApiVisualizador & { htmlDaTela: ReturnType<typeof vi.fn> } {
  return {
    htmlDaTela: vi.fn((_p: string, _a: string, n: number) => Promise.resolve(html(`TL-${n}`))),
  };
}

function revisao(): Revisao {
  return { ocupado: false, aoDecidir: vi.fn().mockResolvedValue(undefined) };
}

function montar(props: Partial<Parameters<typeof PrototipoAoVivo>[0]> = {}) {
  const a = api();
  const r = revisao();
  const vista = render(
    <PrototipoAoVivo
      projeto="faturamento-exemplo"
      analise="A-0003"
      versao={1}
      telas={telas()}
      origem="A-0001"
      api={a}
      revisao={r}
      {...props}
    />,
  );
  return { api: a, revisao: r, ...vista };
}

async function quadro(): Promise<HTMLIFrameElement> {
  return (await screen.findByTitle(/Protótipo de TL-/)) as HTMLIFrameElement;
}

describe("PrototipoAoVivo", () => {
  it("abre a tela num iframe isolado, sem script", async () => {
    const { api } = montar();
    const f = await quadro();
    expect(f.getAttribute("sandbox")).toBe("");
    expect(f.getAttribute("sandbox")).not.toContain("allow-scripts");
    expect(f.getAttribute("srcdoc")).toContain('data-tela="TL-1"');
    expect(f.getAttribute("src")).toBeNull();
    expect(api.htmlDaTela).toHaveBeenCalledWith("faturamento-exemplo", "A-0003", 1);
  });

  it("lista as telas com nova ou alterada, motivo e a confirmar", async () => {
    montar();
    await quadro();
    const lista = screen.getByRole("navigation", { name: "Telas do protótipo" });
    const itens = within(lista).getAllByRole("button");
    expect(itens).toHaveLength(2);
    expect(itens[0]).toHaveTextContent("TL-1");
    expect(itens[0]).toHaveTextContent("alterada");
    expect(itens[0]).toHaveAttribute("aria-current", "true");
    expect(itens[1]).toHaveTextContent("Consultar parcelas");
    expect(itens[1]).toHaveTextContent("nova");
    expect(itens[1]).toHaveTextContent("a confirmar");
    expect(itens[1]).toHaveTextContent("O cliente acompanha as parcelas geradas");
  });

  it("mostra os elementos com a origem ou a confirmar", async () => {
    montar();
    await quadro();
    const lista = screen.getByRole("list", { name: "Elementos de TL-1" });
    const e1 = within(lista).getByRole("listitem", { name: "E-1: Número de parcelas" });
    expect(e1).toHaveTextContent("RN-FAT-008");
    expect(e1).toHaveTextContent("HU-0001/CA-2");
    expect(e1).not.toHaveTextContent("a confirmar");
    const e2 = within(lista).getByRole("listitem", { name: "E-2: Parcela abaixo do mínimo" });
    expect(e2).toHaveTextContent("a confirmar");
  });

  it("troca de tela; tela sem HTML mostra o aviso", async () => {
    const { api } = montar();
    await quadro();
    fireEvent.click(screen.getByRole("button", { name: /TL-2/ }));
    expect(screen.getByRole("status")).toHaveTextContent(
      "A tela TL-2 ainda não tem protótipo HTML",
    );
    expect(screen.queryByTitle(/Protótipo de TL-/)).toBeNull();
    expect(api.htmlDaTela).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("article", { name: "TL-2: Consultar parcelas" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /TL-1/ }));
    expect((await quadro()).getAttribute("srcdoc")).toContain('data-tela="TL-1"');
  });

  it("escolhe o estado mostrado sem liberar script", async () => {
    montar();
    let f = await quadro();
    expect(f.getAttribute("srcdoc")).not.toContain("visualizador-estado");
    fireEvent.click(screen.getByRole("button", { name: "Erro de validação" }));
    f = await quadro();
    const doc = f.getAttribute("srcdoc") ?? "";
    expect(doc).toContain(
      '.prototipo-tela section[data-estado]:not([data-estado="erro"]) { display: none !important; }',
    );
    expect(doc.indexOf("visualizador-estado")).toBeLessThan(doc.indexOf("</head>"));
    expect(f.getAttribute("sandbox")).toBe("");
    expect(screen.getByRole("button", { name: "Erro de validação" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.click(screen.getByRole("button", { name: "Todos" }));
    expect((await quadro()).getAttribute("srcdoc")).not.toContain("visualizador-estado");
  });

  it("mostra a falha ao ler o HTML", async () => {
    const a: ApiVisualizador = {
      htmlDaTela: vi.fn().mockRejectedValue("o protótipo aponta para fora da pasta"),
    };
    montar({ api: a });
    expect(await screen.findByRole("alert")).toHaveTextContent("fora da pasta");
  });

  it("pede ajuste numa tela específica e desfaz o pedido", async () => {
    const { revisao: r, rerender } = montar();
    await quadro();
    fireEvent.click(screen.getByRole("button", { name: /TL-2/ }));
    fireEvent.click(screen.getByRole("button", { name: "Pedir ajuste" }));
    const form = screen.getByRole("form", { name: "Pedir ajuste em TL-2" });
    const enviar = within(form).getByRole("button", { name: "Pedir o ajuste" });
    expect(enviar).toBeDisabled();
    fireEvent.change(within(form).getByLabelText("O que ajustar"), {
      target: { value: " Mostrar o total das parcelas " },
    });
    fireEvent.click(enviar);
    await waitFor(() =>
      expect(r.aoDecidir).toHaveBeenCalledWith({
        tipo: "tela",
        numero: 2,
        decisao: "ajustar",
        pedido: "Mostrar o total das parcelas",
      }),
    );

    const comAjuste = telas();
    comAjuste[1].ajuste_pedido = "Mostrar o total das parcelas";
    rerender(
      <PrototipoAoVivo
        projeto="faturamento-exemplo"
        analise="A-0003"
        versao={2}
        telas={comAjuste}
        api={api()}
        revisao={r}
      />,
    );
    expect(await screen.findByText(/· 1 ajuste pedido/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /TL-2/ })).toHaveTextContent("ajuste pedido");
    fireEvent.click(screen.getByRole("button", { name: "Desfazer o pedido" }));
    await waitFor(() =>
      expect(r.aoDecidir).toHaveBeenLastCalledWith({ tipo: "tela", numero: 2, decisao: "desfazer" }),
    );
  });

  it("sem revisão (aprovada), só leitura", async () => {
    montar({ revisao: undefined });
    await quadro();
    expect(screen.queryByRole("button", { name: "Pedir ajuste" })).toBeNull();
  });

  it("sem telas, avisa", () => {
    montar({ telas: [] });
    expect(screen.getByText("Nenhuma tela registrada ainda.")).toBeInTheDocument();
  });
});

describe("comEstado", () => {
  it("põe o estilo do estado antes do fim do head, e nada sem estado", () => {
    const h = html("TL-1");
    expect(comEstado(h, null)).toBe(h);
    const comErro = comEstado(h, "sucesso");
    expect(comErro).toContain('[data-estado="sucesso"] { display: block !important; }');
    expect(comErro).toMatch(/visualizador-estado[\s\S]*<\/head>/);
    expect(comEstado("<p>sem head</p>", "vazio").startsWith("<style")).toBe(true);
  });
});
