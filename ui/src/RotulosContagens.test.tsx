import { fireEvent, render, screen, within } from "@testing-library/react";
import type { Afirmacao, SecaoDocumento, VistaAnalise } from "./analise";
import { DocumentoAoVivo } from "./DocumentoAoVivo";
import type { Revisao } from "./AcoesRevisao";
import { PainelAnalise } from "./PainelAnalise";
import type { ApiRevisao, Pendencia, Prontidao } from "./revisao";
import { contarPendencias } from "./resumoRevisao";
import { ordenarSecoes, tituloDaSecao } from "./secoesAnalise";
import {
  AFIRMACAO_MULTA,
  apiAnaliseFalsa,
  apiConversaFalsa,
  apiTerminalFalsa,
  telaFalsa,
  vistaFicticia,
} from "./test/falsos";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
vi.mock("./telaXterm", () => ({ criarTelaXterm: vi.fn() }));

/** Seções do documento negocial (`explicar`), como o Rust manda na vista. */
const SECOES_FUNCIONALIDADE: SecaoDocumento[] = [
  { marcador: "objetivo", titulo: "Objetivo" },
  { marcador: "atores_e_telas", titulo: "Atores e telas" },
  { marcador: "fluxo_principal", titulo: "Fluxo principal" },
  { marcador: "fluxos_alternativos", titulo: "Fluxos alternativos" },
  { marcador: "excecoes", titulo: "Exceções" },
  { marcador: "fluxo_processo", titulo: "Fluxo do processo" },
  { marcador: "regras", titulo: "Regras de negócio" },
  { marcador: "dados", titulo: "Dados lidos e gravados" },
  { marcador: "modelo_dados", titulo: "Modelo de dados conceitual" },
  { marcador: "integracoes", titulo: "Integrações" },
  { marcador: "perguntas", titulo: "Perguntas em aberto" },
];

const afirma = (numero: number, secao: string, parcial: Partial<Afirmacao> = {}): Afirmacao =>
  ({
    numero,
    secao,
    texto: `Afirmação número ${numero}.`,
    confianca: "inferido",
    estado: "registrada",
    evidencias: [],
    registrada_ms: numero,
    ...parcial,
  }) as Afirmacao;

const revisao: Revisao = {
  ocupado: false,
  respostas: { disponivel: false, aoEnviar: vi.fn() },
  aoDecidir: () => Promise.resolve(),
};

/** Um marcador cru (`atores_e_telas`) em algum texto visível. */
const MARCADOR_CRU = /\b[a-z]+_[a-z_]+\b/;

describe("títulos das seções (#222)", () => {
  it("usa o título do documento; sem ele, o marcador legível, nunca com sublinhado", () => {
    expect(tituloDaSecao("atores_e_telas", SECOES_FUNCIONALIDADE)).toBe("Atores e telas");
    expect(tituloDaSecao("excecoes", SECOES_FUNCIONALIDADE)).toBe("Exceções");
    // Sem as seções da vista (resposta antiga): os títulos conhecidos.
    expect(tituloDaSecao("regras_encontradas")).toBe("Regras encontradas");
    // Marcador desconhecido: legível.
    expect(tituloDaSecao("demanda_como_entendida")).toBe("Demanda como entendida");
    expect(tituloDaSecao("secao_nova", SECOES_FUNCIONALIDADE)).not.toMatch(/_/);
  });

  it("ordena as seções como o template; as de fora vêm depois, na ordem de chegada", () => {
    const grupos: [string, number][] = [
      ["regras", 1],
      ["outra_b", 2],
      ["objetivo", 3],
      ["outra_a", 4],
      ["excecoes", 5],
    ];
    expect(ordenarSecoes(grupos, SECOES_FUNCIONALIDADE).map(([s]) => s)).toEqual([
      "objetivo",
      "excecoes",
      "regras",
      "outra_b",
      "outra_a",
    ]);
  });

  it("títulos e sumário da análise mostram o título do .md, sem id com sublinhado", () => {
    const vista = vistaFicticia(
      { secoes: SECOES_FUNCIONALIDADE },
      {
        fluxo: "explicar",
        estado: "concluida",
        afirmacoes: [
          afirma(1, "regras"),
          afirma(2, "atores_e_telas"),
          afirma(3, "fluxos_alternativos", { estado: "confirmada" }),
          afirma(4, "excecoes"),
          afirma(5, "integracoes"),
        ],
      },
    );
    render(
      <DocumentoAoVivo vista={vista} semCabecalho aoAbrirEvidencia={vi.fn()} revisao={revisao} />,
    );
    const titulos = screen.getAllByRole("heading", { level: 4 }).map((h) => h.textContent);
    // Na ordem do template, como no `.md`.
    expect(titulos).toEqual([
      "Atores e telas",
      "Fluxos alternativos",
      "Exceções",
      "Regras de negócio",
      "Integrações",
    ]);
    const sumario = screen.getByRole("navigation", { name: "Sumário da análise" });
    expect(within(sumario).getByRole("button", { name: "Atores e telas: 1 pendência" })).toBeInTheDocument();
    expect(within(sumario).getByRole("button", { name: "Fluxos alternativos: resolvida" })).toBeInTheDocument();
    const artigo = screen.getByRole("article", { name: "Documento da análise" });
    // O caminho do documento é o único texto com arquivo; o resto não mostra marcador.
    const textos = Array.from(artigo.querySelectorAll("h3, h4, nav button, .faixa-revisao"))
      .map((e) => e.textContent ?? "")
      .join("\n");
    expect(textos).not.toMatch(MARCADOR_CRU);
  });

  it("o sumário rola até a seção descontando a faixa fixa", () => {
    const rolar = vi.fn();
    HTMLElement.prototype.scrollIntoView = rolar;
    const altura = vi
      .spyOn(HTMLElement.prototype, "getBoundingClientRect")
      .mockReturnValue({ height: 92 } as DOMRect);
    const vista = vistaFicticia(
      { secoes: SECOES_FUNCIONALIDADE },
      { fluxo: "explicar", estado: "concluida", afirmacoes: [afirma(1, "regras")] },
    );
    render(
      <DocumentoAoVivo vista={vista} semCabecalho aoAbrirEvidencia={vi.fn()} revisao={revisao} />,
    );
    const sumario = screen.getByRole("navigation", { name: "Sumário da análise" });
    fireEvent.click(within(sumario).getByRole("button", { name: /^Regras de negócio/ }));
    const titulo = screen.getByRole("heading", { name: "Regras de negócio" });
    expect(titulo.style.scrollMarginTop).toBe("100px");
    expect(rolar.mock.contexts[rolar.mock.contexts.length - 1]).toBe(titulo);
    altura.mockRestore();
  });
});

const NAO_CONCLUIDA: Pendencia = {
  tipo: "analise",
  numero: null,
  motivo: "o agente ainda não concluiu a análise (concluir_analise)",
};
const SEM_AFIRMACOES: Pendencia = {
  tipo: "analise",
  numero: null,
  motivo: "não há afirmações registradas",
};

describe("conta única das pendências (#222)", () => {
  it("conta o que espera a decisão do analista, não o que é da análise como um todo", () => {
    // A análise recém-criada: nada a decidir ainda.
    expect(contarPendencias([NAO_CONCLUIDA, SEM_AFIRMACOES])).toBe(0);
    expect(
      contarPendencias([
        { tipo: "afirmacao", numero: 1, motivo: "afirmação 1: sem decisão" },
        { tipo: "afirmacao", numero: 2, motivo: "afirmação 2: aprofundamento pedido" },
        { tipo: "termo", numero: 1, motivo: "termo \"Carência\": aceite ou recuse" },
        { tipo: "historia", numero: 3, motivo: "HU-0003: sem decisão" },
      ]),
    ).toBe(4);
  });
});

function apiRevisao(p: Prontidao): ApiRevisao {
  return {
    revisar: vi.fn(),
    prontidao: vi.fn(() => Promise.resolve(p)),
    aprovar: vi.fn(),
    escolherDestino: vi.fn(),
    exportar: vi.fn(),
  };
}

const PRONTIDAO: Prontidao = {
  estado: "concluida",
  podeAprovar: false,
  pendencias: [],
  avisos: [],
  areas: [],
  areaObrigatoria: false,
  regras: [],
  incorretas: [],
  termosAceitos: [],
};

function abrir(vista: VistaAnalise, prontidao: Prontidao) {
  const analise = apiAnaliseFalsa(vista);
  render(
    <PainelAnalise
      api={analise.api}
      projeto="faturamento-exemplo"
      analise="A-0001"
      abertura={{ tipo: "reabrir" }}
      aoVoltar={vi.fn()}
      apiTerminal={apiTerminalFalsa().api}
      apiConversa={apiConversaFalsa()}
      apiRevisao={apiRevisao(prontidao)}
      criarTela={() => telaFalsa()}
    />,
  );
}

describe("cabeçalho, faixa e diálogo com o mesmo número (#222)", () => {
  it("afirmações sem decisão e um termo proposto: o mesmo número nos três lugares", async () => {
    const afirmacoes = [
      { ...AFIRMACAO_MULTA, numero: 1 },
      afirma(2, "resposta"),
      afirma(3, "resposta", { estado: "aprofundar", pedido: "Mais" }),
      afirma(4, "resposta", { estado: "confirmada" }),
    ];
    abrir(
      vistaFicticia({}, { estado: "concluida", afirmacoes }),
      {
        ...PRONTIDAO,
        pendencias: [
          { tipo: "afirmacao", numero: 1, motivo: "afirmação 1: sem decisão" },
          { tipo: "afirmacao", numero: 2, motivo: "afirmação 2: sem decisão" },
          { tipo: "afirmacao", numero: 3, motivo: "afirmação 3: aprofundamento pedido" },
          { tipo: "termo", numero: 1, motivo: "termo \"Carência\": aceite ou recuse" },
        ],
      },
    );
    const barra = await screen.findByRole("banner");
    expect(
      await within(barra).findByRole("status", { name: "Situação da aprovação" }),
    ).toHaveTextContent("Aguardando sua revisão · 4 pendentes");
    const faixa = screen.getByRole("region", { name: "Progresso da revisão" });
    expect(within(faixa).getByRole("status")).toHaveTextContent(
      "1 de 4 decididos · 4 pendentes (inclui 1 termo do glossário)",
    );
    fireEvent.click(within(barra).getByRole("button", { name: "Aprovar…" }));
    const dialogo = screen.getByRole("dialog", { name: "Aprovar a análise" });
    expect(
      await within(dialogo).findByRole("button", { name: "Revisar as 4 pendentes" }),
    ).toBeInTheDocument();
  });

  it("análise recém-criada, sem afirmações: nenhuma pendente no cabeçalho", async () => {
    abrir(vistaFicticia({}, { estado: "criada", afirmacoes: [] }), {
      ...PRONTIDAO,
      estado: "criada",
      pendencias: [NAO_CONCLUIDA, SEM_AFIRMACOES],
    });
    const barra = await screen.findByRole("banner");
    await screen.findByRole("button", { name: "Aprovar…" });
    // A prontidão chegou (o botão existe) e o selo não fala em pendentes.
    await new Promise((r) => setTimeout(r, 0));
    expect(within(barra).queryByText(/pendente/)).not.toBeInTheDocument();
  });
});
