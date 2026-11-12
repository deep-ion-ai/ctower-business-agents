import type { SituacaoProtecao } from "../analise";
import type {
  AjusteExclusoes,
  AmbienteDetalhado,
  ApiConfiguracoes,
  AvaliacaoTroca,
  Configuracoes,
  ModoTroca,
  TextoEditavel,
  TrocaBase,
} from "../configuracoes";
import type { PreviaProjeto } from "../primeiroUso";
import type { Prontidao, Repositorios } from "../repositorios";
import { apiPrimeiroUsoFalsa, inventarioFicticio } from "./falsos";

export const BASE = "/Users/analista/Documents/ctower-business";
export const CODIGO = "/codigo/faturamento-exemplo";

export function configuracoesFicticias(parcial: Partial<Configuracoes> = {}): Configuracoes {
  return {
    cliente: "claudeDesktop",
    base: BASE,
    baseSugerida: BASE,
    projetos: [
      {
        id: "faturamento-exemplo",
        nome: "Faturamento Exemplo",
        pastas: [{ caminho: CODIGO, prefixo: null, existe: true }],
        exclusoes: ["docs/antigos/"],
        inclusoes: [],
        repositorios: { forge: null, tracker: null },
      },
    ],
    configClaude: {
      estado: "/Users/analista/.claude.json",
      settings: "/Users/analista/.claude/settings.json",
    },
    ...parcial,
  };
}

export const TEXTOS: TextoEditavel[] = [
  {
    grupo: "template",
    id: "analise-impacto",
    rotulo: "Análise de impacto",
    arquivo: "_templates/analise-impacto.md",
    estado: "editado",
  },
  {
    grupo: "roteiro",
    id: "perguntar",
    rotulo: "Perguntar ao sistema",
    arquivo: "_templates/roteiros/perguntar.md",
    estado: "padrao",
  },
  {
    grupo: "prompt",
    id: "aprofundar",
    rotulo: "Aprofundar",
    arquivo: "_templates/prompts/aprofundar.md",
    estado: "igualAoPadrao",
  },
  {
    grupo: "regrasDoProjeto",
    id: "faturamento-exemplo",
    rotulo: "Regras do projeto Faturamento Exemplo",
    arquivo: "faturamento-exemplo/regras-do-projeto.md",
    estado: "igualAoPadrao",
  },
];

function protecao(ligada: boolean): SituacaoProtecao {
  return {
    projeto: "faturamento-exemplo",
    ligada,
    completa: ligada,
    arquivo: "/Users/analista/.claude/settings.json",
    regras: [{ regra: `Edit(/${CODIGO}/**)`, presente: ligada, doApp: ligada }],
    negadasPeloHook: ["Bash", "PowerShell"],
  };
}

/** API das configurações falsa: tudo dá certo, salvo o que o teste mudar. */
export function apiConfiguracoesFalsa(cfg: Configuracoes = configuracoesFicticias()) {
  const base = apiPrimeiroUsoFalsa();
  const api = {
    ...base,
    configuracoes: vi.fn((): Promise<Configuracoes> => Promise.resolve(cfg)),
    salvarRepositorios: vi.fn(
      (_projeto: string, repositorios: Repositorios): Promise<Repositorios> =>
        Promise.resolve(repositorios),
    ),
    prontidaoRepositorios: vi.fn((): Promise<Prontidao> => Promise.resolve({ integracoes: [] })),
    verificarAmbiente: vi.fn(
      (): Promise<AmbienteDetalhado> =>
        Promise.resolve({
          pronto: true,
          pendencias: [],
          verificacao: {
            diretorio: CODIGO,
            chave: CODIGO,
            servidorMcp: "ok",
            hooks: [
              { evento: "SessionStart", situacao: "ok" },
              { evento: "Stop", situacao: "ok" },
            ],
            hooksDesativados: false,
            pronto: true,
          },
        }),
    ),
    avaliarTroca: vi.fn(
      (destino: string): Promise<AvaliacaoTroca> =>
        Promise.resolve({
          destino: { caminho: destino, estado: "nova", recusa: null, git: false },
          atual: cfg.base,
          arquivos: 12,
          bytes: 40_960,
          recusaMover: null,
          recusaReapontar: null,
          sessoesAbertas: 0,
        }),
    ),
    trocarBase: vi.fn(
      (destino: string, modo: ModoTroca): Promise<TrocaBase> =>
        Promise.resolve({
          modo,
          base: destino,
          anterior: cfg.base,
          copiados: modo === "mover" ? 12 : 0,
          origemApagada: modo === "mover",
          indexados: 5,
          avisos: [],
        }),
    ),
    previaExclusoes: vi.fn(
      (a: AjusteExclusoes): Promise<PreviaProjeto> =>
        Promise.resolve({
          id: a.projeto,
          nome: "Faturamento Exemplo",
          pastas: [],
          exclusoes: a.exclusoes,
          inclusoes: a.inclusoes,
          jaCadastrado: false,
          inventario: inventarioFicticio(),
        }),
    ),
    salvarExclusoes: vi.fn((a: AjusteExclusoes) =>
      Promise.resolve({
        id: a.projeto,
        nome: "Faturamento Exemplo",
        pastas: [],
        inventario: inventarioFicticio(),
      }),
    ),
    desfazerAmbiente: vi.fn((_s: string) =>
      Promise.resolve({
        protecaoTirada: [],
        preparo: { diretorio: CODIGO, alteracoes: [], gravado: true },
        avisos: [],
      }),
    ),
    descadastrar: vi.fn((_s: string) =>
      Promise.resolve({
        ambiente: {
          protecaoTirada: [],
          preparo: { diretorio: CODIGO, alteracoes: [], gravado: true },
          avisos: [],
        },
        documentos: `${BASE}/faturamento-exemplo`,
      }),
    ),
    textos: vi.fn((): Promise<TextoEditavel[]> => Promise.resolve(TEXTOS)),
    abrirTexto: vi.fn((arquivo: string) => Promise.resolve(`${BASE}/${arquivo}`)),
    situacaoProtecao: vi.fn((_s: string) => Promise.resolve(protecao(false))),
    mudarProtecao: vi.fn((_s: string, ligada: boolean) => Promise.resolve(protecao(ligada))),
  } satisfies ApiConfiguracoes;
  return api;
}
