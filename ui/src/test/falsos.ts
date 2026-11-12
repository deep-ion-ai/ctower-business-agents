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
import type { EstadoClientes } from "../clientes";
import type { ApiConversa, OpcoesConversa, ResultadoDesktop } from "../conversa";
import type {
  ApiPrimeiroUso,
  AvaliacaoBase,
  ClientePreferido,
  PedidoProjeto,
  PreviaProjeto,
  ResumoInventario,
  ProjetoCadastrado,
  Situacao as SituacaoPrimeiroUso,
} from "../primeiroUso";
import type { ApiTerminal, EventoTerminal, PedidoClaude, Tamanho } from "../terminal";
import type { TelaTerminal } from "../telaXterm";

/** Estado de análise do sistema fictício "Faturamento Exemplo". */
export function analiseFicticia(parcial: Partial<Analise> = {}): Analise {
  return {
    id: "A-0001",
    projeto: "faturamento-exemplo",
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
        projetos: [{ id: "faturamento-exemplo", nome: "Faturamento Exemplo" }],
      }),
    ),
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
  projeto: string;
  pedido: PedidoClaude;
  aoEvento: (e: EventoTerminal) => void;
  resolver: (id: number) => void;
}

/** API do terminal falsa: cada abertura espera o teste resolver. */
export function apiTerminalFalsa() {
  const aberturas: Abertura[] = [];
  const api = {
    abrir: vi.fn(
      (projeto: string, pedido: PedidoClaude, _t: Tamanho, aoEvento: Abertura["aoEvento"]) =>
        new Promise<number>((resolver) => {
          aberturas.push({ projeto, pedido, aoEvento, resolver });
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

/** Clientes de IA prontos (Claude Code instalado e logado, Desktop instalado). */
export function clientesProntos(parcial: Partial<EstadoClientes> = {}): EstadoClientes {
  return {
    plataforma: "macOs",
    claudeCode: {
      instalacao: {
        estado: "encontrado",
        caminho: "/Users/analista/.local/bin/claude",
        origem: "path",
        versao: "2.1.0",
      },
      login: { estado: "logado", metodo: "claude.ai", provedor: "firstParty" },
      orientacao: [],
    },
    claudeDesktop: {
      instalacao: { estado: "encontrado", caminho: "/Applications/Claude.app" },
      orientacao: [{ texto: "Confira no próprio Claude Desktop se você entrou." }],
    },
    ...parcial,
  };
}

/** Inventário do sistema fictício "Faturamento Exemplo". */
export function inventarioFicticio(parcial: Partial<ResumoInventario> = {}): ResumoInventario {
  return {
    analisados: 14,
    bytesAnalisados: 48_000,
    ignorados: 1,
    porLinguagem: [
      { chave: "sql", rotulo: "SQL", quantidade: 3 },
      { chave: "delphi", rotulo: "Delphi", quantidade: 2 },
      { chave: "cobol", rotulo: "COBOL", quantidade: 1 },
    ],
    porMotivo: [{ chave: "segredo", rotulo: "possível segredo", quantidade: 1 }],
    itensIgnorados: [
      {
        caminho: ".env",
        pasta: false,
        motivo: "segredo",
        rotulo: "possível segredo",
        regra: ".env",
        ajustavel: true,
      },
    ],
    duracaoMs: 12,
    ...parcial,
  };
}

export function situacaoInicial(parcial: Partial<SituacaoPrimeiroUso> = {}): SituacaoPrimeiroUso {
  return {
    base: null,
    baseSugerida: "/Users/analista/Documents/ctower-business",
    projetos: [],
    cliente: null,
    primeiroUsoConcluido: false,
    ...parcial,
  };
}

/** API do primeiro uso falsa: tudo dá certo, salvo o que o teste mudar. */
export function apiPrimeiroUsoFalsa() {
  const api = {
    situacao: vi.fn((): Promise<SituacaoPrimeiroUso> => Promise.resolve(situacaoInicial())),
    estadoClientes: vi.fn((): Promise<EstadoClientes> => Promise.resolve(clientesProntos())),
    escolherCliente: vi.fn((_c: ClientePreferido) => Promise.resolve()),
    avaliarBase: vi.fn((pasta: string): Promise<AvaliacaoBase> =>
      Promise.resolve({ caminho: pasta, estado: "nova", recusa: null, git: false }),
    ),
    configurarBase: vi.fn((pasta: string) => Promise.resolve(pasta)),
    previaProjeto: vi.fn((p: PedidoProjeto): Promise<PreviaProjeto> =>
      Promise.resolve({
        id: "faturamento-exemplo",
        nome: p.nome,
        pastas: p.pastas.map((caminho, i) => ({
          caminho,
          prefixo: i === 0 ? null : `pasta-${i + 1}`,
          branch: "main",
          git: true,
        })),
        exclusoes: p.exclusoes,
        inclusoes: p.inclusoes,
        jaCadastrado: false,
        inventario: inventarioFicticio(),
      }),
    ),
    cadastrarProjeto: vi.fn((p: PedidoProjeto): Promise<ProjetoCadastrado> =>
      Promise.resolve({
        id: "faturamento-exemplo",
        nome: p.nome,
        pastas: [],
        inventario: inventarioFicticio(),
      }),
    ),
    concluir: vi.fn(() => Promise.resolve()),
    abrirLink: vi.fn((_url: string) => Promise.resolve()),
    escolherPasta: vi.fn((_t: string, _i?: string): Promise<string | null> =>
      Promise.resolve("/codigo/faturamento-exemplo"),
    ),
    verificarAmbiente: vi.fn((): Promise<AmbienteClaude> =>
      Promise.resolve({ pronto: true, pendencias: [] }),
    ),
    planejarPreparo: vi.fn((): Promise<PlanoPreparo> =>
      Promise.resolve({ diretorio: "/codigo", alteracoes: [], gravado: false }),
    ),
    prepararAmbiente: vi.fn((): Promise<PlanoPreparo> =>
      Promise.resolve({ diretorio: "/codigo", alteracoes: [], gravado: true }),
    ),
  } satisfies ApiPrimeiroUso;
  return api;
}

/** Opções da conversa do sistema fictício: os três clientes, sem preferência. */
export function opcoesFicticias(parcial: Partial<OpcoesConversa> = {}): OpcoesConversa {
  return {
    plataforma: "macOs",
    preferido: null,
    opcoes: [
      { cliente: "terminalEmbutido", disponivel: true },
      { cliente: "claudeDesktop", disponivel: true },
      { cliente: "terminalDoAnalista", disponivel: true },
    ],
    sessao: null,
    pedido: "Inicie a análise A-0001 pelo ctower",
    linkDesktop:
      "claude://code/new?folder=/codigo/faturamento-exemplo&q=Inicie%20a%20an%C3%A1lise%20A-0001%20pelo%20ctower",
    linkTerminal:
      "claude-cli://open?cwd=/codigo/faturamento-exemplo&q=Inicie%20a%20an%C3%A1lise%20A-0001%20pelo%20ctower",
    comandoTerminal: "cd /codigo/faturamento-exemplo && claude 'Inicie a análise A-0001 pelo ctower'",
    comandoRetomada: null,
    ...parcial,
  };
}

/** API dos clientes da conversa falsa: tudo dá certo, salvo o que o teste mudar. */
export function apiConversaFalsa(opcoes: OpcoesConversa = opcoesFicticias()) {
  const api = {
    opcoes: vi.fn((_s: string, _a: string) => Promise.resolve(opcoes)),
    abrirPorLink: vi.fn(
      (_s: string, _a: string, _c: "claudeDesktop" | "terminalDoAnalista") => Promise.resolve(),
    ),
    retomarNoDesktop: vi.fn(
      (_s: string, _a: string): Promise<ResultadoDesktop> =>
        Promise.resolve({
          aberto: true,
          codigo: 0,
          mensagem: "Opening session sessao-1 in Claude Desktop",
        }),
    ),
    passouAoDesktop: vi.fn((_s: string, _a: string) => Promise.resolve()),
    copiar: vi.fn((_t: string) => Promise.resolve()),
  } satisfies ApiConversa;
  return api;
}
