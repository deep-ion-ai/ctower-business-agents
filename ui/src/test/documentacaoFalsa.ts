import type {
  ApiDocumentacao,
  CoberturaDoTipo,
  ColunaDeDocumentacao,
  DocumentoDoTipo,
  LinhaDeDocumentacao,
  PropostaVista,
  TabelaDeDocumentacao,
} from "../documentacaoMapa";

/** Visão "Documentação" do mapa falsa (#462): RUP com o manual habilitado. */

export const COLUNAS: ColunaDeDocumentacao[] = [
  { tipo: "caso-de-uso", nome: "Especificação de caso de uso", geracao: "agente" },
  { tipo: "catalogo-regras", nome: "Catálogo de regras de negócio", geracao: "app" },
  { tipo: "especificacao-suplementar", nome: "Especificação suplementar", geracao: "app" },
  { tipo: "manual", nome: "Manual do usuário", geracao: "agente" },
];

const SEM: DocumentoDoTipo = {
  tipo: "",
  estado: { estado: "sem_documento" },
  acao: "Gerar versão inicial",
};

export const sem = (tipo: string): DocumentoDoTipo => ({ ...SEM, tipo });

export function linha(
  funcionalidade: string,
  nome: string,
  documentos: DocumentoDoTipo[] = COLUNAS.map((c) => sem(c.tipo)),
): LinhaDeDocumentacao {
  return {
    funcionalidade,
    nome,
    area: "cobranca",
    areaNome: "Cobrança",
    situacao: "aceita",
    documentos,
  };
}

/** Conta as células como `cba_tools::cobertura::cobertura_por_tipo`. */
export function contarCobertura(
  colunas: ColunaDeDocumentacao[],
  linhas: LinhaDeDocumentacao[],
): CoberturaDoTipo[] {
  return colunas.map((c, i) => {
    const estados = linhas.map((l) => l.documentos[i].estado.estado);
    const n = (e: string) => estados.filter((x) => x === e).length;
    return {
      tipo: c.tipo,
      nome: c.nome,
      funcionalidades: linhas.length,
      comDocumento: linhas.length - n("sem_documento"),
      emDia: n("em_dia"),
      desatualizados: n("desatualizado"),
      propostasPendentes: n("proposta_pendente"),
      semDocumento: n("sem_documento"),
    };
  });
}

export function tabelaDe(linhas: LinhaDeDocumentacao[]): TabelaDeDocumentacao {
  return {
    metodologia: "rup",
    colunas: COLUNAS,
    linhas,
    cobertura: contarCobertura(COLUNAS, linhas),
  };
}

/** F-FAT-001 com caso de uso em dia, catálogo com proposta e manual desatualizado; as outras sem. */
export function tabelaInicial(): TabelaDeDocumentacao {
  return tabelaDe([
    linha("F-FAT-001", "Cancelar fatura", [
      {
        tipo: "caso-de-uso",
        caminho: "funcionalidades/cobranca/cancelar-fatura/caso-de-uso.md",
        versao: "1.3",
        estado: { estado: "em_dia", versao: "1.3" },
      },
      {
        tipo: "catalogo-regras",
        caminho: "funcionalidades/cobranca/cancelar-fatura/catalogo-regras.md",
        estado: { estado: "proposta_pendente", versao: "1.0", revisada: false },
        acao: "Revisar proposta",
      },
      sem("especificacao-suplementar"),
      {
        tipo: "manual",
        caminho: "funcionalidades/cobranca/cancelar-fatura/manual.md",
        versao: "1.0",
        estado: { estado: "desatualizado", versao: "1.0", arquivos: ["db/04_pkg_situacao.sql"] },
        acao: "Propor atualização",
      },
    ]),
    linha("F-FAT-002", "Calcular multa"),
    linha("F-FAT-003", "Atualizar contato"),
  ]);
}

export const PROPOSTA: PropostaVista = {
  caminho: "funcionalidades/cobranca/cancelar-fatura/catalogo-regras.md",
  tipo: "catalogo-regras",
  nomeDoTipo: "Catálogo de regras de negócio",
  funcionalidade: "F-FAT-001",
  publicada: null,
  proposta: {
    versao: "1.0",
    texto: "# Catálogo de regras de negócio: Cancelar fatura\n\n- RN-FAT-001",
    trechos: [
      { marca: "novo", texto: "# Catálogo de regras de negócio: Cancelar fatura" },
      { marca: "novo", texto: "- **RN-FAT-001**: Uma fatura paga não pode ser cancelada." },
    ],
    mudancas: "Versão inicial, gerada por engenharia reversa",
    criada_ms: 1,
    atualizada_ms: 1,
  },
};

export function apiDocumentacaoFalsa(tabela = tabelaInicial()) {
  const estado = { tabela };
  const api = {
    tabela: vi.fn(() => Promise.resolve(estado.tabela)),
    gerar: vi.fn(),
    proposta: vi.fn(() => Promise.resolve(PROPOSTA)),
    marcarRevisada: vi.fn(() =>
      Promise.resolve({ ...PROPOSTA.proposta, revisada: { por: "ana", em_ms: 2 } }),
    ),
    publicar: vi.fn(() => Promise.resolve({})),
    descartar: vi.fn(() => Promise.resolve()),
    proporAtualizacao: vi.fn(() => Promise.resolve(PROPOSTA.caminho)),
  } satisfies ApiDocumentacao;
  return { api, estado };
}
