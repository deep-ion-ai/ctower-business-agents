import type {
  ApiReuniao,
  AtaVista,
  ConfirmacaoParticipante,
  PreviaTranscript,
  ReuniaoCriada,
} from "../reunioes";

/** Reunião fictícia do "Faturamento Exemplo": o cancelamento de fatura antiga. */
export function previaFicticia(parcial: Partial<PreviaTranscript> = {}): PreviaTranscript {
  return {
    formato: "texto_teams",
    falas: 12,
    falantes: [
      { nome: "Silva, Maria", falas: 5 },
      { nome: "João Pereira", falas: 4 },
      { nome: "Ana Souza", falas: 3 },
    ],
    semFalante: 0,
    avisos: [
      {
        codigo: "texto_antes_da_primeira_fala",
        mensagem: "2 linha(s) antes da primeira fala foram descartadas.",
      },
    ],
    podeCriar: true,
    ...parcial,
  };
}

export function ataFicticia(parcial: Partial<AtaVista> = {}): AtaVista {
  return {
    id: "R-0001",
    demanda: "D-0001",
    data: "2026-11-14",
    status: "rascunho",
    objetivo: null,
    resumo: null,
    participantes: [
      {
        nome: "Silva, Maria",
        papel: null,
        pessoa: "maria-silva",
        catalogo: {
          pessoa: { slug: "maria-silva", nome: "Maria Silva" },
          papel: "PO",
          reuniao: "D-0001/R-0000",
        },
        candidatos: [],
        proposta: null,
      },
      {
        nome: "João Pereira",
        papel: null,
        pessoa: null,
        catalogo: null,
        candidatos: [],
        proposta: {
          papel: "Desenvolvedor",
          justificativa: "Explica onde fica a trava no código.",
          citacoes: [
            {
              falante: "João Pereira",
              inicio: "00:00:30",
              trecho: "a trava dos 30 dias só existe na tela de retaguarda",
              status: "conferida",
            },
          ],
        },
      },
      {
        nome: "Maria S",
        papel: null,
        pessoa: null,
        catalogo: null,
        candidatos: [],
        proposta: null,
      },
    ],
    itens: [],
    documento: "faturamento-exemplo/demandas/D-0001-parcelamento-pj/reunioes/R-0001.md",
    papeis: [
      { nome: "PO", descricao: "Dono do produto" },
      { nome: "Gerente", descricao: null },
    ],
    transcript: { apagaEm: "2026-12-14", venceEmBreve: false },
    ...parcial,
  };
}

export const DECISAO = {
  id: "D-1",
  tipo: "decisao" as const,
  texto: "A trava dos 30 dias vale também no banco",
  responsavel: null,
  prazo: null,
  referencias: [],
  citacoes: [
    {
      falante: "Silva, Maria",
      inicio: "00:01:40",
      trecho: "vamos levar a trava para o banco",
      status: "conferida",
    },
  ],
  evidencias: ["db/03_pkg_cobranca.sql:71-72"],
};

/** API falsa da reunião. */
export function apiReuniaoFalsa(ata: AtaVista = ataFicticia()) {
  const api = {
    escolherArquivo: vi.fn((): Promise<string | null> =>
      Promise.resolve("/home/analista/reuniao-01-cancelamento.vtt"),
    ),
    previaTranscript: vi.fn((): Promise<PreviaTranscript> => Promise.resolve(previaFicticia())),
    criarReuniao: vi.fn(
      (): Promise<ReuniaoCriada> =>
        Promise.resolve({
          reuniao: "R-0001",
          demanda: "D-0001",
          analise: {
            id: "A-0005",
            titulo: "Reunião R-0001 · Parcelamento PJ",
            fluxo: "reuniao",
            estado: "criada",
            atualizadaMs: 5,
          },
          promptInicial: "Inicie a análise A-0005 pelo ctower com reuniao R-0001",
        }),
    ),
    ataReuniao: vi.fn((): Promise<AtaVista> => Promise.resolve(ata)),
    confirmarParticipantes: vi.fn(
      (_p: string, _r: string, _c: ConfirmacaoParticipante[]): Promise<AtaVista> =>
        Promise.resolve(ata),
    ),
  } satisfies ApiReuniao;
  return api;
}
