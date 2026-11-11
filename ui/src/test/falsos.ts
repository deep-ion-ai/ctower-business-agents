import type {
  Afirmacao,
  AmbienteClaude,
  AnaliseResumo,
  AvisoAnalise,
  PlanoPreparo,
  PromptPronto,
  Situacao,
  ApiAnalise,
  Analise,
  VistaAnalise,
} from "../analise";
import type { ApiTerminal, EventoTerminal, PedidoClaude, Tamanho } from "../terminal";
import type { TelaTerminal } from "../telaXterm";

/** Estado de análise do sistema fictício "Faturamento Exemplo". */
export function analiseFicticia(parcial: Partial<Analise> = {}): Analise {
  return {
    id: "A-0001",
    sistema: "faturamento-exemplo",
    fluxo: "perguntar",
    titulo: "Multa por atraso",
    entrada: "Como é calculada a multa por atraso de pagamento?",
    estado: "em-andamento",
    documento: "analises/A-0001.md",
    versao: 1,
    criada_ms: 1,
    atualizada_ms: 1,
    sessoes: [],
    afirmacoes: [],
    perguntas: [],
    sugestoes: [],
    termos: [],
    ...parcial,
  };
}

export const AFIRMACAO_MULTA: Afirmacao = {
  numero: 1,
  secao: "resposta",
  texto: "Passada a carência, a multa é de 2% sobre o valor original.",
  confianca: "confirmado",
  estado: "registrada",
  evidencias: [
    {
      arquivo: "db/03_pkg_cobranca.sql",
      linhas: "71-72",
      trecho: "FN_PARAMETRO('PERC_MULTA_ATRASO') / 100, 2)",
      hash: "abc",
      estado: "valida",
    },
  ],
  registrada_ms: 2,
};

export function vistaFicticia(
  parcial: Partial<VistaAnalise> = {},
  analise: Partial<Analise> = {},
): VistaAnalise {
  return {
    analise: analiseFicticia(analise),
    documento: "/base/faturamento-exemplo/analises/A-0001.md",
    sessao: null,
    promptRetomada: "Retome a análise A-0001 pelo ctower",
    ...parcial,
  };
}

/** API da tela da análise falsa, com a vigia controlada pelo teste. */
export function apiAnaliseFalsa(vista: VistaAnalise = vistaFicticia()) {
  const avisos: ((a: AvisoAnalise) => void)[] = [];
  const parar = vi.fn();
  const api = {
    situacao: vi.fn((): Promise<Situacao> =>
      Promise.resolve({
        base: "/base",
        sistemas: [{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }],
      }),
    ),
    configurarBase: vi.fn((pasta: string) => Promise.resolve(pasta)),
    cadastrarSistema: vi.fn((nome: string) => Promise.resolve({ id: "faturamento-exemplo", nome })),
    verificarAmbiente: vi.fn((): Promise<AmbienteClaude> =>
      Promise.resolve({ pronto: true, pendencias: [] }),
    ),
    planejarPreparo: vi.fn((): Promise<PlanoPreparo> =>
      Promise.resolve({ diretorio: "/codigo", alteracoes: [], gravado: false }),
    ),
    prepararAmbiente: vi.fn((): Promise<PlanoPreparo> =>
      Promise.resolve({ diretorio: "/codigo", alteracoes: [], gravado: true }),
    ),
    listarAnalises: vi.fn((): Promise<AnaliseResumo[]> => Promise.resolve([])),
    criarAnalise: vi.fn(() =>
      Promise.resolve({
        analise: {
          id: "A-0001",
          titulo: "Multa por atraso",
          fluxo: "perguntar" as const,
          estado: "criada" as const,
          atualizadaMs: 1,
        },
        promptInicial: "Inicie a análise A-0001 pelo ctower",
      }),
    ),
    carregarAnalise: vi.fn(() => Promise.resolve(vista)),
    trechoEvidencia: vi.fn((_s: string, arquivo: string, inicio: number, fim: number) =>
      Promise.resolve({
        arquivo,
        inicio,
        fim,
        primeira: inicio - 2,
        linhas: Array.from({ length: fim - inicio + 5 }, (_, i) => `linha ${inicio - 2 + i}`),
        total: 200,
      }),
    ),
    vigiar: vi.fn((_s: string, f: (a: AvisoAnalise) => void) => {
      avisos.push(f);
      return Promise.resolve(parar);
    }),
    promptsProntos: vi.fn((): Promise<PromptPronto[]> => Promise.resolve([])),
  } satisfies ApiAnalise;
  const avisar = (a: AvisoAnalise) => avisos.forEach((f) => f(a));
  return { api, avisar, parar };
}

export interface Abertura {
  sistema: string;
  pedido: PedidoClaude;
  aoEvento: (e: EventoTerminal) => void;
  resolver: (id: number) => void;
}

/** API do terminal falsa: cada abertura espera o teste resolver. */
export function apiTerminalFalsa() {
  const aberturas: Abertura[] = [];
  const api = {
    abrir: vi.fn(
      (sistema: string, pedido: PedidoClaude, _t: Tamanho, aoEvento: Abertura["aoEvento"]) =>
        new Promise<number>((resolver) => {
          aberturas.push({ sistema, pedido, aoEvento, resolver });
        }),
    ),
    escrever: vi.fn(() => Promise.resolve()),
    redimensionar: vi.fn(() => Promise.resolve()),
    enviarPrompt: vi.fn(() => Promise.resolve()),
    encerrar: vi.fn(() => Promise.resolve()),
  } satisfies ApiTerminal;
  return { api, aberturas };
}

export function telaFalsa(): TelaTerminal {
  return {
    abrirEm: vi.fn(),
    escrever: vi.fn(),
    aoDigitar: vi.fn(),
    ajustar: vi.fn(() => ({ linhas: 24, colunas: 80 })),
    limpar: vi.fn(),
    focar: vi.fn(),
    descartar: vi.fn(),
  };
}
