import type { AmbienteClaude, PlanoPreparo, Situacao } from "../analise";
import type {
  ApiConhecimento,
  Busca,
  DocumentoAberto,
  Inicio,
  MapaVista,
  ResumoRevalidacao,
  VisaoProjeto,
} from "../conhecimento";

/** Dados do sistema fictício "Faturamento Exemplo" depois de uma análise aprovada. */
export const PROJETO = "faturamento-exemplo";
export const REGRA = `${PROJETO}/regras/RN-COB-001.md`;
export const ANALISE = `${PROJETO}/analises/A-0001.md`;
export const TERMO = `${PROJETO}/glossario/carencia.md`;

export function inicioFicticio(parcial: Partial<Inicio> = {}): Inicio {
  const multa = {
    projeto: PROJETO,
    projetoNome: "Faturamento Exemplo",
    id: "A-0001",
    titulo: "Multa por atraso",
    fluxo: "perguntar" as const,
    estado: "aprovada" as const,
    atualizadaMs: 1,
    sessao: null,
  };
  const impacto = {
    ...multa,
    id: "A-0002",
    titulo: "Parcelamento para pessoa jurídica",
    fluxo: "impacto" as const,
    estado: "em-andamento" as const,
    atualizadaMs: 2,
    sessao: { id: "s1", estado: "aguardando-analista" as const, cliente: "claude-code" as const },
  };
  return {
    base: "/base",
    projetos: [
      {
        id: PROJETO,
        nome: "Faturamento Exemplo",
        analises: 2,
        regras: 1,
        aRevalidar: 1,
        termos: 1,
        erro: null,
      },
    ],
    emAndamento: [impacto],
    recentes: [impacto, multa],
    ...parcial,
  };
}

export function visaoFicticia(parcial: Partial<VisaoProjeto> = {}): VisaoProjeto {
  return {
    id: PROJETO,
    nome: "Faturamento Exemplo",
    documento: `${PROJETO}/projeto.md`,
    regras: [
      {
        id: "RN-COB-002",
        titulo: "Juros contam a partir do primeiro dia",
        area: "cobranca",
        status: "a-revalidar",
        caminho: `${PROJETO}/regras/RN-COB-002.md`,
        origem: ANALISE,
        evidencias: 1,
      },
      {
        id: "RN-COB-001",
        titulo: "Passada a carência, a multa é de 2% sobre o valor original",
        area: "cobranca",
        status: "confirmada",
        caminho: REGRA,
        origem: ANALISE,
        evidencias: 1,
      },
      {
        id: "RN-PAR-001",
        titulo: "Parcelamento em até 3 vezes",
        area: "parcelamento",
        status: "confirmada",
        caminho: `${PROJETO}/regras/RN-PAR-001.md`,
        origem: null,
        evidencias: 1,
      },
    ],
    termos: [
      {
        termo: "Carência",
        sinonimos: ["prazo de tolerância"],
        area: "cobranca",
        caminho: TERMO,
        origem: ANALISE,
      },
    ],
    areas: [
      {
        id: "cobranca",
        nome: "Cobrança",
        sigla: "COB",
        declarada: true,
        regras: 2,
        aRevalidar: 1,
        termos: 1,
        memoria: null,
      },
      {
        id: "parcelamento",
        nome: "Parcelamento",
        sigla: null,
        declarada: false,
        regras: 1,
        aRevalidar: 0,
        termos: 0,
        memoria: null,
      },
    ],
    problemas: [],
    ...parcial,
  };
}

export function buscaFicticia(): Busca {
  return {
    total: 2,
    deslocamento: 0,
    qualquerPalavra: false,
    resultados: [
      {
        caminho: REGRA,
        projeto: PROJETO,
        tipo: "regra",
        identificador: "RN-COB-001",
        titulo: "Passada a carência, a multa é de 2% sobre o valor original",
        area: "cobranca",
        status: "confirmada",
        trecho: "Passada a carência, a **multa** é de 2%",
      },
      {
        caminho: ANALISE,
        projeto: PROJETO,
        tipo: "pergunta",
        identificador: "A-0001",
        titulo: "Multa por atraso",
        area: null,
        status: "aprovada",
        trecho: "Como é calculada a **multa** por atraso?",
      },
    ],
  };
}

export function documentoFicticio(caminho: string): DocumentoAberto {
  if (caminho === ANALISE) {
    return {
      caminho,
      projeto: PROJETO,
      tipo: "pergunta",
      titulo: "Multa por atraso",
      status: "aprovada",
      campos: [{ chave: "id", valor: "A-0001" }],
      markdown: "# A-0001: Multa por atraso\n\n## Resposta\n\nPassada a carência, a multa é de 2%.\n",
      ligacoes: [
        { caminho: REGRA, titulo: "Passada a carência…", campo: "origem", sai: false, existe: true },
      ],
      analise: { projeto: PROJETO, id: "A-0001" },
      problema: null,
    };
  }
  return {
    caminho,
    projeto: PROJETO,
    tipo: "regra",
    titulo: "Passada a carência, a multa é de 2% sobre o valor original",
    status: "confirmada",
    campos: [
      { chave: "tipo", valor: "regra" },
      { chave: "id", valor: "RN-COB-001" },
      { chave: "area", valor: "cobranca" },
    ],
    markdown: [
      "# RN-COB-001",
      "",
      "Passada a carência, a **multa** é de 2% sobre o valor original.",
      "",
      "Veja a [análise de origem](../analises/A-0001.md) e o [termo](../glossario/carencia.md).",
      "Site externo: [exemplo](https://exemplo.invalido/pagina).",
      "",
      '<script>alert("x")</script>',
      "",
    ].join("\n"),
    ligacoes: [
      { caminho: ANALISE, titulo: "Multa por atraso", campo: "origem", sai: true, existe: true },
    ],
    analise: null,
    problema: null,
  };
}

/** API falsa das telas do início, do projeto e da base, com a vigia controlada pelo teste. */
export function apiConhecimentoFalsa() {
  const avisos: (() => void)[] = [];
  const parar = vi.fn();
  const revalidacao: ResumoRevalidacao = {
    evidencias: 3,
    desatualizadas: 1,
    regras: [{ id: "RN-COB-002", titulo: "Juros", antes: "confirmada", depois: "a-revalidar" }],
    analises: ["A-0001"],
    problemas: [],
    mudou: true,
    duracaoMs: 5,
  };
  const mapaRaiz: MapaVista = {
    geradoEm: 1_700_000_000,
    modulo: {
      caminho: "",
      arquivos: 14,
      falhas: 0,
      linguagens: [{ chave: "sql", rotulo: "SQL", quantidade: 3 }],
      simbolos: [{ chave: "tabela", rotulo: "tabela", quantidade: 6 }],
      subpastas: [
        { caminho: "db", arquivos: 3 },
        { caminho: "backend", arquivos: 8 },
      ],
      arquivosDiretos: ["README.md"],
      dependencias: [{ de: "backend", para: "db", referencias: 12 }],
    },
  };
  const api = {
    situacao: vi.fn(
      (): Promise<Situacao> =>
        Promise.resolve({
          base: "/base",
          projetos: [{ id: PROJETO, nome: "Faturamento Exemplo" }],
        }),
    ),
    inicio: vi.fn((): Promise<Inicio> => Promise.resolve(inicioFicticio())),
    criarAnalise: vi.fn((_s: string, fluxo: "perguntar" | "explicar" | "impacto") =>
      Promise.resolve({
        analise: {
          id: "A-0003",
          titulo: "Parcelamento",
          fluxo,
          estado: "criada" as const,
          atualizadaMs: 3,
        },
        promptInicial: "Inicie a análise A-0003 pelo ctower",
      }),
    ),
    criarAnaliseDeDemanda: vi.fn((_s: string, fluxo: "perguntar" | "explicar" | "impacto") =>
      Promise.resolve({
        analise: {
          id: "A-0004",
          titulo: "Multa por atraso",
          fluxo,
          estado: "criada" as const,
          atualizadaMs: 4,
        },
        promptInicial: "Inicie a análise A-0004 pelo ctower",
      }),
    ),
    visaoProjeto: vi.fn((): Promise<VisaoProjeto> => Promise.resolve(visaoFicticia())),
    revalidar: vi.fn((): Promise<ResumoRevalidacao> => Promise.resolve(revalidacao)),
    mapa: vi.fn(
      (_s: string, modulo?: string): Promise<MapaVista | null> =>
        Promise.resolve(
          modulo
            ? {
                ...mapaRaiz,
                modulo: {
                  ...mapaRaiz.modulo,
                  caminho: modulo,
                  arquivos: 3,
                  subpastas: [],
                  arquivosDiretos: [`${modulo}/03_pkg_cobranca.sql`],
                  dependencias: [],
                },
              }
            : mapaRaiz,
        ),
    ),
    mapear: vi.fn((): Promise<MapaVista> => Promise.resolve(mapaRaiz)),
    buscar: vi.fn((): Promise<Busca> => Promise.resolve(buscaFicticia())),
    abrirDocumento: vi.fn((caminho: string) => Promise.resolve(documentoFicticio(caminho))),
    vigiarBase: vi.fn((f: () => void) => {
      avisos.push(f);
      return Promise.resolve(parar);
    }),
    verificarAmbiente: vi.fn(
      (): Promise<AmbienteClaude> => Promise.resolve({ pronto: true, pendencias: [] }),
    ),
    planejarPreparo: vi.fn(
      (): Promise<PlanoPreparo> =>
        Promise.resolve({ diretorio: "/codigo", alteracoes: [], gravado: false }),
    ),
    prepararAmbiente: vi.fn(
      (): Promise<PlanoPreparo> =>
        Promise.resolve({ diretorio: "/codigo", alteracoes: [], gravado: true }),
    ),
  } satisfies ApiConhecimento;
  const avisar = () => avisos.forEach((f) => f());
  return { api, avisar, parar };
}
