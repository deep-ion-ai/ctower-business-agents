import type { SituacaoProtecao } from "../analise";
import {
  CONFLITO,
  type AjusteExclusoes,
  type AmbienteDetalhado,
  type ApiConfiguracoes,
  type AvaliacaoTroca,
  type AvisoTexto,
  type Configuracoes,
  type EstadoTexto,
  type LeituraTexto,
  type ModoTroca,
  type SalvoTexto,
  type TextoEditavel,
  type TrocaBase,
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

/** Texto padrão de cada arquivo do catálogo falso. */
export function padraoFalso(t: TextoEditavel): string {
  return `<!--\nInstrução: use {{id}} e {{titulo}}.\n-->\n# {{id}} · ${t.rotulo}\n\n## Corpo\n\n{{corpo}}\n`;
}

const VARIAVEIS_FALSAS = [
  { nome: "id", descricao: "Identificador." },
  { nome: "titulo", descricao: "Título." },
  { nome: "corpo", descricao: "Corpo do documento." },
];

/** Os avisos que o app daria: marcador que o texto não aceita (só fora das regras). */
function avisosFalsos(t: TextoEditavel, texto: string): AvisoTexto[] {
  if (t.grupo === "regrasDoProjeto") return [];
  const avisos: AvisoTexto[] = [];
  texto.split("\n").forEach((linha, i) => {
    for (const m of linha.matchAll(/\{\{([a-z0-9_]+)\}\}/g)) {
      if (!VARIAVEIS_FALSAS.some((v) => v.nome === m[1])) {
        avisos.push({
          linha: i + 1,
          variavel: m[1],
          ausente: false,
          mensagem: `marcador desconhecido {{${m[1]}}}`,
        });
      }
    }
  });
  return avisos;
}

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
  // Os arquivos da base: `texto` nulo é "sem cópia" (vale o padrão do app).
  let contador = 0;
  const arquivos = new Map(
    TEXTOS.map((t) => [
      t.arquivo,
      {
        texto:
          t.estado === "padrao"
            ? null
            : t.estado === "editado"
              ? `${padraoFalso(t)}\nAcréscimo do analista.\n`
              : padraoFalso(t),
        versao: `v${++contador}`,
      },
    ]),
  );
  const catalogo = (arquivo: string) => {
    const t = TEXTOS.find((x) => x.arquivo === arquivo);
    if (!t) throw new Error(`${arquivo} não é um texto editável da base`);
    return t;
  };
  const estadoDe = (t: TextoEditavel): EstadoTexto => {
    const a = arquivos.get(t.arquivo)!;
    if (a.texto === null) return "padrao";
    return a.texto === padraoFalso(t) ? "igualAoPadrao" : "editado";
  };
  const versaoDe = (arquivo: string) =>
    arquivos.get(arquivo)!.texto === null ? "ausente" : arquivos.get(arquivo)!.versao;
  const leitura = (arquivo: string): LeituraTexto => {
    const t = catalogo(arquivo);
    return {
      grupo: t.grupo,
      id: t.id,
      rotulo: t.rotulo,
      arquivo,
      texto: arquivos.get(arquivo)!.texto ?? padraoFalso(t),
      padrao: padraoFalso(t),
      estado: estadoDe(t),
      variaveis: t.grupo === "regrasDoProjeto" ? [] : VARIAVEIS_FALSAS,
      versao: versaoDe(arquivo),
    };
  };
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
    textos: vi.fn(
      (): Promise<TextoEditavel[]> =>
        Promise.resolve(TEXTOS.map((t) => ({ ...t, estado: estadoDe(t) }))),
    ),
    abrirTexto: vi.fn((arquivo: string) => {
      const t = catalogo(arquivo);
      const a = arquivos.get(arquivo)!;
      // Como o app: abrir um texto sem cópia cria a cópia do padrão.
      if (a.texto === null) arquivos.set(arquivo, { texto: padraoFalso(t), versao: `v${++contador}` });
      return Promise.resolve(`${BASE}/${arquivo}`);
    }),
    lerTexto: vi.fn((arquivo: string): Promise<LeituraTexto> => Promise.resolve(leitura(arquivo))),
    validarTexto: vi.fn(
      (arquivo: string, texto: string): Promise<AvisoTexto[]> =>
        Promise.resolve(avisosFalsos(catalogo(arquivo), texto)),
    ),
    salvarTexto: vi.fn((arquivo: string, texto: string, versaoLida: string): Promise<SalvoTexto> => {
      const t = catalogo(arquivo);
      if (versaoDe(arquivo) !== versaoLida) {
        return Promise.reject(
          `${CONFLITO} ${arquivo} mudou desde que foi aberto (editado fora do app?); recarregue para ver a versão atual`,
        );
      }
      arquivos.set(arquivo, { texto, versao: `v${++contador}` });
      return Promise.resolve({
        estado: estadoDe(t),
        versao: versaoDe(arquivo),
        avisos: avisosFalsos(t, texto),
      });
    }),
    restaurarTexto: vi.fn((arquivo: string): Promise<LeituraTexto> => {
      const t = catalogo(arquivo);
      arquivos.set(arquivo, {
        texto: t.grupo === "regrasDoProjeto" ? padraoFalso(t) : null,
        versao: `v${++contador}`,
      });
      return Promise.resolve(leitura(arquivo));
    }),
    situacaoProtecao: vi.fn((_s: string) => Promise.resolve(protecao(false))),
    mudarProtecao: vi.fn((_s: string, ligada: boolean) => Promise.resolve(protecao(ligada))),
  } satisfies ApiConfiguracoes;
  /** Simula outro programa gravando o arquivo (muda a versão: o próximo salvar conflita). */
  const editarFora = (arquivo: string, texto: string) => {
    arquivos.set(arquivo, { texto, versao: `v${++contador}` });
  };
  return Object.assign(api, { editarFora, textoNaBase: (arquivo: string) => arquivos.get(arquivo)!.texto });
}
