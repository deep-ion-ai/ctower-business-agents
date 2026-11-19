import type {
  ApiMapa,
  Funcionalidade,
  MapaFuncionalidades,
  PainelFuncionalidade,
} from "../funcionalidades";

/** Mapa de funcionalidades falso (#208) para os testes da tela e do painel. */

export function funcionalidade(f: Partial<Funcionalidade> & Pick<Funcionalidade, "id" | "nome">): Funcionalidade {
  return {
    area: "cobranca",
    areaNome: "Cobrança",
    descricao: `${f.nome}.`,
    atores: [],
    pontos: [
      {
        nome: "PKG_COBRANCA.FN_CALCULAR_MULTA",
        chave: "db/03_pkg_cobranca.sql#PKG_COBRANCA.FN_CALCULAR_MULTA",
        tipo: "rotina_banco",
        arquivo: "db/03_pkg_cobranca.sql",
        linhaInicial: 10,
        linhaFinal: 10,
        trecho: "FUNCTION FN_CALCULAR_MULTA",
        foraDaLista: false,
      },
      {
        nome: "EncargosService.calcular",
        chave: "backend/E.java#EncargosService.calcular",
        tipo: "servico",
        arquivo: "backend/E.java",
        linhaInicial: 5,
        linhaFinal: 5,
        trecho: "public void calcular()",
        foraDaLista: false,
      },
    ],
    dadosLidos: [],
    dadosGravados: [],
    relacionadas: [],
    situacao: "pendente",
    juntadaEm: null,
    editada: false,
    documento: null,
    explicacao: null,
    semPontoDeEntrada: false,
    ...f,
  };
}

export const MAPA: MapaFuncionalidades = {
  areas: [
    {
      area: "cancelamento",
      nome: "Cancelamento",
      funcionalidades: [
        funcionalidade({
          id: "F-CAN-001",
          nome: "Cancelar fatura",
          area: "cancelamento",
          areaNome: "Cancelamento",
          situacao: "aceita",
          documento: "faturamento-exemplo/funcionalidades/cancelamento/cancelar-fatura/negocial.md",
          explicacao: {
            id: "A-0002",
            titulo: "F-CAN-001 · Cancelar fatura",
            fluxo: "explicar",
            estado: "aprovada",
            atualizadaMs: 1,
          },
        }),
      ],
    },
    {
      area: "cobranca",
      nome: "Cobrança",
      funcionalidades: [funcionalidade({ id: "F-COB-001", nome: "Calcular encargos" })],
    },
  ],
  fechadas: [],
  total: 2,
  pendentes: 1,
  documentadas: 1,
  justificados: [
    {
      chave: "db/03_pkg_cobranca.sql#PKG_COBRANCA.FN_PARAMETRO",
      tipo: "rotina_banco",
      nome: "PKG_COBRANCA.FN_PARAMETRO",
      arquivo: "db/03_pkg_cobranca.sql",
      motivo: "utilitario",
      justificativa: "Lê parâmetros.",
    },
  ],
  completude: {
    total: 15,
    semTratamento: [
      {
        chave: "db/04_pkg_situacao.sql#PKG_SITUACAO.PRC_SUSPENDER_COBRANCA",
        tipo: "rotina_banco",
        nome: "PKG_SITUACAO.PRC_SUSPENDER_COBRANCA",
        arquivo: "db/04_pkg_situacao.sql",
        linha: 40,
      },
    ],
  },
  erroCompletude: null,
  documento: "faturamento-exemplo/funcionalidades/mapa.md",
  versao: 3,
};

export function apiFalsa(
  mapa: MapaFuncionalidades = MAPA,
  painel: (id: string) => PainelFuncionalidade = painelDe,
) {
  const api: ApiMapa = {
    issues: vi.fn().mockResolvedValue({
      issues: [],
      excedeuTeto: false,
      desde: "2026-05-19",
      funcionalidades: [],
    }),
    vincular: vi.fn().mockResolvedValue({
      criadas: [],
      reaproveitadas: [],
      vinculosNovos: 0,
      vinculosExistentes: 0,
      avisos: [],
    }),
    mapa: vi.fn().mockResolvedValue(mapa),
    revisar: vi.fn().mockResolvedValue({ ...mapa, pendentes: 0 }),
    pedido: vi.fn().mockResolvedValue("Mapeie as funcionalidades do projeto pelo ctower"),
    explicar: vi.fn().mockResolvedValue({
      analise: {
        id: "A-0003",
        titulo: "F-COB-001 · Calcular encargos",
        fluxo: "explicar",
        estado: "criada",
        atualizadaMs: 1,
      },
      promptInicial: "Inicie a análise A-0003",
    }),
    painel: vi.fn((_projeto: string, id: string) => Promise.resolve(painel(id))),
    criarAnalise: vi.fn().mockResolvedValue({
      analise: {
        id: "A-0009",
        titulo: "Pergunta",
        fluxo: "perguntar",
        estado: "criada",
        atualizadaMs: 1,
      },
      promptInicial: "Inicie a análise A-0009",
    }),
    escolherDestino: vi.fn().mockResolvedValue("/tmp/mapa.md"),
    exportar: vi.fn().mockResolvedValue("/tmp/mapa.md"),
  };
  return api;
}


/** Painel da funcionalidade do MAPA, com análises, uma demanda e regras. */
export function painelDe(id: string): PainelFuncionalidade {
  const f =
    MAPA.areas.flatMap((a) => a.funcionalidades).find((x) => x.id === id) ?? MAPA.areas[0].funcionalidades[0];
  return {
    funcionalidade: f,
    analises: [
      {
        id: "A-0019",
        titulo: "D-0004 · impacto em F-CAN-001",
        fluxo: "impacto",
        estado: "em-andamento",
        atualizadaMs: Date.UTC(2026, 10, 18, 12),
        ligacao: "funcionalidade",
        demanda: "D-0004",
        perguntasAbertas: 1,
      },
      {
        id: "A-0008",
        titulo: "Quem pode cancelar uma fatura vencida?",
        fluxo: "perguntar",
        estado: "concluida",
        atualizadaMs: Date.UTC(2026, 10, 5, 12),
        ligacao: "codigo",
        demanda: null,
        perguntasAbertas: 0,
      },
    ],
    demandas: [
      {
        id: "D-0004",
        titulo: "Permitir cancelamento parcial",
        aberta: true,
        criado: "2026-11-14",
        ligadaNoPainel: true,
        impacto: { analise: "A-0019", estado: "em-andamento" },
        historias: null,
        cenarios: null,
        perguntas: [
          {
            analise: "A-0019",
            numero: 1,
            texto: "O estorno parcial gera nova fatura?",
            destinatario: "marina",
            demanda: "D-0004",
          },
        ],
      },
    ],
    regras: [
      {
        id: "RN-CAN-001",
        titulo: "Só fatura aberta",
        enunciado: "Só fatura com situação aberta pode ser cancelada pelo atendente.",
        status: "confirmada",
        caminho: "faturamento-exemplo/regras/RN-CAN-001.md",
      },
      {
        id: "RN-CAN-003",
        titulo: "Cancelar estorna as parcelas.",
        enunciado: "Cancelar estorna as parcelas **ainda não pagas**:\n- as vencidas;\n- as a vencer.",
        status: "a-revalidar",
        caminho: "faturamento-exemplo/regras/RN-CAN-003.md",
      },
    ],
    perguntas: [
      {
        analise: "A-0019",
        numero: 1,
        texto: "O estorno parcial gera nova fatura?",
        destinatario: "marina",
        demanda: "D-0004",
      },
    ],
    aRevisar: ["A-0008"],
    relacionadas: [{ id: "F-COB-001", nome: "Calcular encargos" }],
    atividade: [{ quandoMs: Date.UTC(2026, 10, 18, 12), texto: "A-0019 criada (análise de impacto)" }],
    demandasDoProjeto: [
      { id: "D-0004", titulo: "Permitir cancelamento parcial" },
      { id: "D-0007", titulo: "Bloquear cancelamento após remessa" },
    ],
  };
}
