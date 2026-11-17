import type { ApiRastreabilidade, Dependentes, Matriz } from "../rastreabilidade";

/** Dados fictícios do "Faturamento Exemplo" para as telas da rastreabilidade (#121). */
export const PROJETO_BASE = "faturamento-exemplo";
const SERVICO = "backend/src/main/java/exemplo/faturamento/ParcelamentoService.java";

export function matrizFicticia(): Matriz {
  const regra = (id: string, titulo: string, status = "confirmada") => ({
    id,
    titulo,
    status,
    documento: `${PROJETO_BASE}/regras/${id}.md`,
  });
  const ct1 = {
    id: "CT-0001",
    titulo: "Pessoa física pede parcelamento",
    analise: "A-0003",
    historia: "HU-0001",
    situacao: "pendente",
    aRevalidar: false,
    aConfirmar: false,
  };
  return {
    projeto: PROJETO_BASE,
    demanda: {
      id: "D-0001",
      titulo: "Parcelamento PJ",
      status: "aberta",
      documento: `${PROJETO_BASE}/demandas/D-0001-parcelamento-pj/demanda.md`,
    },
    analises: [
      {
        id: "A-0001",
        titulo: "Impacto do parcelamento",
        fluxo: "impacto",
        estado: "em-andamento",
        documento: `${PROJETO_BASE}/analises/A-0001.md`,
        origem: null,
        demanda: "D-0001",
      },
    ],
    historias: [
      {
        id: "HU-0001",
        titulo: "Parcelar fatura de pessoa física",
        analise: "A-0002",
        situacao: "aceita",
        aprovada: true,
        aConfirmar: false,
        analises: ["A-0001", "A-0002"],
        criterios: [],
        afirmacoes: ["A-0001/1"],
        regras: [regra("RN-FAT-007", "Parcelamento só para PJ")],
        evidencias: [
          {
            arquivo: SERVICO,
            linhaInicial: 30,
            linhaFinal: 32,
            aRevalidar: true,
            de: "A-0001/1",
          },
        ],
        cenarios: [ct1],
        alertas: ["evidencia-a-revalidar"],
      },
      {
        id: "HU-0002",
        titulo: "Limitar a 3 parcelas",
        analise: "A-0002",
        situacao: "pendente",
        aprovada: false,
        aConfirmar: false,
        analises: ["A-0001", "A-0002"],
        criterios: [],
        afirmacoes: ["A-0001/2"],
        regras: [regra("RN-FAT-008", "Quantidade de parcelas")],
        evidencias: [],
        cenarios: [],
        alertas: ["historia-sem-cenario"],
      },
      {
        id: "HU-0003",
        titulo: "Mostrar parcelas no relatório",
        analise: "A-0002",
        situacao: "pendente",
        aprovada: false,
        aConfirmar: false,
        analises: ["A-0002"],
        criterios: [],
        afirmacoes: [],
        regras: [regra("RN-FAT-011", "Composição das parcelas")],
        evidencias: [],
        cenarios: [{ ...ct1, id: "CT-0002", historia: "HU-0003" }],
        alertas: [],
      },
    ],
    regras: [
      {
        regra: regra("RN-FAT-009", "Valor mínimo da parcela"),
        analises: ["A-0001"],
        historias: [],
        evidencias: [],
        cenarios: [],
        alertas: ["regra-sem-historia"],
      },
    ],
    lacunas: [
      {
        alerta: "evidencia-a-revalidar",
        alvo: "HU-0001",
        descricao: "HU-0001 está aprovada e depende de evidência a revalidar.",
        relacionados: ["A-0001/1"],
      },
      {
        alerta: "regra-sem-historia",
        alvo: "RN-FAT-009",
        descricao: "RN-FAT-009 aparece em A-0001 e não entrou em nenhuma história.",
        relacionados: ["A-0001"],
      },
      {
        alerta: "historia-sem-cenario",
        alvo: "HU-0002",
        descricao: "HU-0002 não tem cenário de teste.",
        relacionados: ["A-0002"],
      },
    ],
  };
}

export function dependentesFicticios(alvo: Dependentes["alvo"]): Dependentes {
  const daRegra = alvo.tipo === "regra";
  return {
    projeto: PROJETO_BASE,
    alvo,
    regra: daRegra
      ? {
          id: "RN-FAT-007",
          titulo: "Parcelamento só para PJ",
          status: "a-revalidar",
          documento: `${PROJETO_BASE}/regras/RN-FAT-007.md`,
        }
      : null,
    arquivos: daRegra ? [SERVICO] : [],
    regras: daRegra
      ? []
      : [
          {
            id: "RN-FAT-008",
            titulo: "Quantidade de parcelas",
            status: "confirmada",
            documento: `${PROJETO_BASE}/regras/RN-FAT-008.md`,
          },
        ],
    demandas: [
      {
        id: "D-0001",
        titulo: "Parcelamento PJ",
        status: "aberta",
        documento: `${PROJETO_BASE}/demandas/D-0001-parcelamento-pj/demanda.md`,
      },
    ],
    analises: [
      {
        id: "A-0002",
        titulo: "Histórias do parcelamento",
        fluxo: "historias",
        estado: "em-andamento",
        documento: `${PROJETO_BASE}/analises/A-0002.md`,
        origem: "A-0001",
        demanda: "D-0001",
      },
    ],
    criterios: [
      {
        historia: "HU-0001",
        criterio: "CA-1",
        texto: "são geradas 3 parcelas",
        analise: "A-0002",
        situacao: "aceito",
        aRevalidar: daRegra,
        aConfirmar: false,
      },
    ],
    cenarios: daRegra
      ? [
          {
            id: "CT-0001",
            titulo: "Pessoa física pede parcelamento",
            analise: "A-0003",
            historia: "HU-0001",
            situacao: "pendente",
            aRevalidar: false,
            aConfirmar: false,
          },
        ]
      : [],
  };
}

export function apiRastreabilidadeFalsa() {
  const api = {
    matriz: vi.fn(async () => matrizFicticia()),
    dependentes: vi.fn(async (_p: string, alvo: Dependentes["alvo"]) =>
      dependentesFicticios(alvo),
    ),
    escolherDestino: vi.fn(async (): Promise<string | null> => "/tmp/matriz-D-0001.xlsx"),
    exportar: vi.fn(async (_p: string, _d: string, _f: string, destino: string) => destino),
  } satisfies ApiRastreabilidade;
  return api;
}
