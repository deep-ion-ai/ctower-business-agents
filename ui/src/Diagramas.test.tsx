import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Diagrama } from "./analise";
import { DocumentoAoVivo } from "./DocumentoAoVivo";
import { ExportacaoAnalise } from "./ExportacaoAnalise";
import { elementoDoId } from "./mermaid";
import { PreviaMarkdown } from "./PreviaMarkdown";
import type { ApiRevisao } from "./revisao";
import { vistaFicticia } from "./test/falsos";

// O Mermaid de verdade precisa de layout (getBBox), que o jsdom não tem: o teste
// troca a renderização por um SVG com os mesmos `data-id` do flowchart.
const desenhar = vi.hoisted(() => vi.fn());
vi.mock("./mermaid", async (original) => ({
  ...(await original<typeof import("./mermaid")>()),
  desenharMermaid: desenhar,
}));

const SVG =
  '<svg viewBox="0 0 200 80"><g class="node default" id="diagrama-mermaid-1-flowchart-n1-0"><text>Aberta</text></g>' +
  '<g class="node default" id="diagrama-mermaid-1-flowchart-n2-1"><text>Cancelada</text></g>' +
  '<g class="edgeLabel"><g class="label" data-id="L_n1_n2_0"><text>cancelar</text></g></g></svg>';

const CODIGO = 'flowchart LR\n  n1(["Aberta"])\n  n2(["Cancelada"])\n  n1 -->|"cancelar"| n2';

beforeEach(() => {
  desenhar.mockReset();
  desenhar.mockImplementation((codigo: string) =>
    codigo.includes("-->|") ? Promise.resolve(SVG) : Promise.reject(new Error("Parse error on line 2:\n...")),
  );
});

const evidencia = (arquivo: string, linhas: string) => ({
  arquivo,
  linhas,
  trecho: "x",
  hash: "h",
  estado: "valida" as const,
});

const DIAGRAMA: Diagrama = {
  numero: 1,
  tipo: "estados",
  titulo: "Ciclo de vida da fatura",
  secao: "resposta",
  nos: [
    {
      chave: "ABERTA",
      rotulo: "Aberta",
      papel: "inicio",
      evidencias: [evidencia("db/01_tabelas.sql", "28-28")],
    },
    { chave: "CANCELADA", rotulo: "Cancelada", papel: "fim" },
  ],
  arestas: [
    {
      de: "ABERTA",
      para: "CANCELADA",
      rotulo: "cancelar",
      confianca: "confirmado",
      evidencias: [evidencia("db/03_pkg_cobranca.sql", "111-124")],
    },
  ],
  registrada_ms: 3,
};

describe("diagramas Mermaid (#118)", () => {
  it("lê o elemento pelo data-id do flowchart", () => {
    expect(elementoDoId("n3")).toEqual({ tipo: "no", indice: 2 });
    expect(elementoDoId("L_n1_n2_0")).toEqual({ tipo: "aresta", de: 0, para: 1, ocorrencia: 0 });
    expect(elementoDoId("L-n2-n1-1")).toEqual({ tipo: "aresta", de: 1, para: 0, ocorrencia: 1 });
    expect(elementoDoId("diagrama-mermaid-4-flowchart-n12-11")).toEqual({ tipo: "no", indice: 11 });
    expect(elementoDoId("diagrama-mermaid-4")).toBeNull();
    expect(elementoDoId(null)).toBeNull();
  });

  it("o bloco mermaid do Markdown vira diagrama, com o código em Ver Mermaid", async () => {
    render(<PreviaMarkdown texto={"# T\n\n```mermaid\n" + CODIGO + "\n```\n"} aceitos={[]} />);
    const figura = await screen.findByRole("figure", { name: "Diagrama" });
    await waitFor(() => expect(figura.querySelector("svg")).not.toBeNull());
    expect(desenhar).toHaveBeenCalledWith(`${CODIGO}\n`, "escuro");
    fireEvent.click(within(figura).getByRole("button", { name: "Ver Mermaid" }));
    expect(within(figura).getByText(/flowchart LR/)).toBeInTheDocument();
  });

  it("Mermaid inválido mostra aviso e código, e o resto do documento aparece", async () => {
    render(
      <PreviaMarkdown
        texto={"Antes.\n\n```mermaid\nflowchart LR\n  a -->\n```\n\nDepois.\n"}
        aceitos={[]}
      />,
    );
    const aviso = await screen.findByRole("alert");
    expect(aviso).toHaveTextContent("Diagrama não renderizado.");
    expect(aviso).toHaveTextContent("Parse error on line 2:");
    expect(aviso).toHaveTextContent("a -->");
    expect(screen.getByText("Antes.")).toBeInTheDocument();
    expect(screen.getByText("Depois.")).toBeInTheDocument();
  });

  it("no documento ao vivo, clicar num estado ou transição abre a evidência", async () => {
    const aoAbrirEvidencia = vi.fn();
    const vista = vistaFicticia(
      { diagramas: [{ numero: 1, mermaid: CODIGO }] },
      { diagramas: [DIAGRAMA] },
    );
    render(<DocumentoAoVivo vista={vista} aoAbrirEvidencia={aoAbrirEvidencia} />);
    const bloco = screen.getByTestId("diagrama-1");
    expect(within(bloco).getByText(/Ciclo de vida da fatura/)).toBeInTheDocument();
    const figura = await within(bloco).findByRole("figure", { name: "Ciclo de vida da fatura" });
    await waitFor(() => expect(figura.querySelector("svg")).not.toBeNull());

    fireEvent.click(within(figura).getByText("cancelar"));
    expect(aoAbrirEvidencia).toHaveBeenLastCalledWith(
      DIAGRAMA.arestas![0].evidencias![0],
      "diagrama-1",
    );
    const linhas = within(bloco).getAllByRole("row");
    expect(linhas[3]).toHaveAttribute("aria-selected", "true");

    fireEvent.click(within(figura).getByText("Aberta"));
    expect(aoAbrirEvidencia).toHaveBeenLastCalledWith(DIAGRAMA.nos[0].evidencias![0], "diagrama-1");
    expect(linhas[1]).toHaveAttribute("aria-selected", "true");

    // Nó sem evidência só seleciona a linha.
    aoAbrirEvidencia.mockClear();
    fireEvent.click(within(figura).getByText("Cancelada"));
    expect(aoAbrirEvidencia).not.toHaveBeenCalled();
    expect(linhas[2]).toHaveAttribute("aria-selected", "true");

    // Os botões da tabela também abrem o trecho.
    fireEvent.click(within(bloco).getByRole("button", { name: "db/03_pkg_cobranca.sql:111-124" }));
    expect(aoAbrirEvidencia).toHaveBeenCalledWith(
      DIAGRAMA.arestas![0].evidencias![0],
      "diagrama-1",
    );
  });

  it("exportação em Word leva as figuras desenhadas e avisa das que falharam", async () => {
    const exportar = vi.fn((_p: string, _a: string, pedido: { destino: string }) =>
      Promise.resolve(pedido.destino),
    );
    const api = {
      revisar: vi.fn(),
      prontidao: vi.fn(),
      aprovar: vi.fn(),
      escolherDestino: vi.fn(() => Promise.resolve<string | null>("/home/analista/A-0001.docx")),
      exportar,
      diagramasExportacao: vi.fn(() => Promise.resolve([CODIGO, "flowchart LR\n  a -->"])),
    } satisfies ApiRevisao;
    const desenharPng = vi.fn((codigo: string) =>
      codigo === CODIGO
        ? Promise.resolve({ png: new Uint8Array([137, 80, 78, 71]), largura: 200, altura: 80 })
        : Promise.reject(new Error("Parse error")),
    );
    render(
      <ExportacaoAnalise
        api={api}
        projeto="faturamento-exemplo"
        analise="A-0001"
        aprovada={false}
        desenharPng={desenharPng}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Exportar…" }));
    await waitFor(() =>
      expect(exportar).toHaveBeenCalledWith("faturamento-exemplo", "A-0001", {
        destino: "/home/analista/A-0001.docx",
        formato: "word",
        opcoes: { evidencias: true, perguntas: true, trechos: true, codigo_diagramas: true },
        figuras: [{ codigo: CODIGO, png: [137, 80, 78, 71], largura: 200, altura: 80 }],
      }),
    );
    expect(api.diagramasExportacao).toHaveBeenCalledWith("faturamento-exemplo", "A-0001");
    expect(await screen.findByText(/Um diagrama não pôde ser desenhado/)).toBeInTheDocument();

    // Markdown mantém o bloco: nada é desenhado.
    desenharPng.mockClear();
    fireEvent.click(screen.getByLabelText("Markdown (.md)"));
    expect(screen.getByLabelText("Código dos diagramas (anexo do Word)")).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Exportar…" }));
    await waitFor(() => expect(exportar).toHaveBeenCalledTimes(2));
    expect(exportar.mock.lastCall?.[2]).not.toHaveProperty("figuras");
    expect(desenharPng).not.toHaveBeenCalled();
  });

  it("ciclo de vida: condição e efeitos, achados e Virar pergunta (#119)", async () => {
    const ciclo: Diagrama = {
      ...DIAGRAMA,
      nos: [
        ...DIAGRAMA.nos,
        {
          chave: "PROTESTADA",
          rotulo: "Protestada",
          evidencias: [evidencia("db/04_pkg_situacao.sql", "9-10")],
        },
      ],
      arestas: [{ ...DIAGRAMA.arestas![0], condicao: "sem pagamento", efeitos: "grava o log" }],
    };
    const achado = {
      id: "inalcancavel:PROTESTADA",
      tipo: "inalcancavel" as const,
      estado: "PROTESTADA",
      texto: "O estado Protestada (`PROTESTADA`) é inalcançável.",
      evidencias: [evidencia("db/04_pkg_situacao.sql", "9-10")],
    };
    const aoDecidir = vi.fn(() => Promise.resolve());
    const vista = vistaFicticia(
      { diagramas: [{ numero: 1, mermaid: CODIGO, achados: [achado] }] },
      { diagramas: [ciclo], estado: "concluida" },
    );
    const { rerender } = render(
      <DocumentoAoVivo
        vista={vista}
        aoAbrirEvidencia={vi.fn()}
        revisao={{ aoDecidir, ocupado: false }}
      />,
    );
    const bloco = screen.getByTestId("diagrama-1");
    expect(within(bloco).getByRole("columnheader", { name: "Condição" })).toBeInTheDocument();
    expect(within(bloco).getByRole("cell", { name: "sem pagamento" })).toBeInTheDocument();
    expect(within(bloco).getByRole("cell", { name: "grava o log" })).toBeInTheDocument();
    const item = within(bloco).getByTestId("achado-inalcancavel:PROTESTADA");
    expect(item).toHaveTextContent("Estado inalcançável");

    fireEvent.click(within(item).getByRole("button", { name: "Virar pergunta" }));
    const campo = within(item).getByLabelText("Pergunta para o solicitante");
    expect(campo).toHaveValue(achado.texto);
    fireEvent.change(campo, { target: { value: "  A fatura ainda pode ser protestada?  " } });
    fireEvent.click(within(item).getByRole("button", { name: "Registrar pergunta" }));
    await waitFor(() =>
      expect(aoDecidir).toHaveBeenCalledWith({
        tipo: "virar-pergunta",
        texto: "A fatura ainda pode ser protestada?",
        origem: { tipo: "achado-diagrama", diagrama: 1, achado: "inalcancavel:PROTESTADA" },
      }),
    );

    // Depois de registrada, o achado mostra a pergunta e não oferece o botão de novo.
    const comPergunta = vistaFicticia(
      { diagramas: [{ numero: 1, mermaid: CODIGO, achados: [achado] }] },
      {
        diagramas: [ciclo],
        estado: "concluida",
        perguntas: [
          {
            numero: 1,
            texto: "A fatura ainda pode ser protestada?",
            estado: "aberta",
            registrada_ms: 4,
            origem: { tipo: "achado-diagrama", diagrama: 1, achado: "inalcancavel:PROTESTADA" },
            evidencias: [evidencia("db/04_pkg_situacao.sql", "9-10")],
          },
        ],
      },
    );
    rerender(
      <DocumentoAoVivo
        vista={comPergunta}
        aoAbrirEvidencia={vi.fn()}
        revisao={{ aoDecidir, ocupado: false }}
      />,
    );
    const marcado = screen.getByTestId("achado-inalcancavel:PROTESTADA");
    expect(marcado).toHaveTextContent("virou a pergunta 1");
    expect(within(marcado).queryByRole("button", { name: "Virar pergunta" })).toBeNull();
    expect(screen.getByText(/de um achado do D-1 · db\/04_pkg_situacao.sql:9-10/)).toBeInTheDocument();
  });
});
