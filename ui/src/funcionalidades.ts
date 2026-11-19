import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { AnaliseCriada, AnaliseResumo, EstadoAnalise, Fluxo } from "./analise";
import type { StatusRegra } from "./conhecimento";

/**
 * Mapa de funcionalidades do projeto (#208): um cadastro do projeto, fora das
 * análises. Espelho dos comandos de `ui/src-tauri/src/funcionalidades.rs`
 * (campos em camelCase).
 */

/** Natureza de um ponto de entrada (`cba_index::TipoEntrada`). */
export type TipoEntrada = "tela" | "endpoint" | "lote" | "rotina_banco" | "servico" | "integracao";

export const NOME_TIPO_ENTRADA: Record<TipoEntrada, string> = {
  tela: "tela",
  endpoint: "endpoint",
  lote: "lote",
  rotina_banco: "rotina de banco",
  servico: "serviço",
  integracao: "integração",
};

export type MotivoSemFuncionalidade = "codigo_morto" | "utilitario" | "outro";

export const NOME_MOTIVO: Record<MotivoSemFuncionalidade, string> = {
  codigo_morto: "código morto",
  utilitario: "utilitário",
  outro: "outro motivo",
};

export type SituacaoFuncionalidade = "pendente" | "aceita" | "descartada" | "juntada";

export interface PontoDaFuncionalidade {
  nome: string;
  chave: string | null;
  tipo: TipoEntrada | null;
  arquivo: string;
  linhaInicial: number;
  linhaFinal: number;
  trecho: string;
  /** Fora da lista do mapa estrutural: só a evidência o sustenta. */
  foraDaLista: boolean;
}

export interface Funcionalidade {
  id: string;
  nome: string;
  area: string;
  areaNome: string;
  descricao: string;
  atores: string[];
  pontos: PontoDaFuncionalidade[];
  dadosLidos: string[];
  dadosGravados: string[];
  relacionadas: string[];
  situacao: SituacaoFuncionalidade;
  juntadaEm: string | null;
  editada: boolean;
  /** Documento negocial aprovado, relativo à raiz da base. */
  documento: string | null;
  /** Análise de explicação ligada (a mais recente), se houver. */
  explicacao: AnaliseResumo | null;
  /** Nenhum ponto de entrada dela existe mais no código. */
  semPontoDeEntrada: boolean;
}

export interface AreaMapa {
  area: string;
  nome: string;
  funcionalidades: Funcionalidade[];
}

export interface PontoSemTratamento {
  chave: string;
  tipo: TipoEntrada;
  nome: string;
  arquivo: string;
  linha: number;
}

export interface PontoJustificado {
  chave: string;
  tipo: TipoEntrada;
  nome: string;
  arquivo: string;
  motivo: MotivoSemFuncionalidade;
  justificativa: string;
}

export interface Completude {
  total: number;
  semTratamento: PontoSemTratamento[];
}

export interface MapaFuncionalidades {
  areas: AreaMapa[];
  /** Descartadas e juntadas. */
  fechadas: Funcionalidade[];
  total: number;
  pendentes: number;
  documentadas: number;
  justificados: PontoJustificado[];
  /** Sem ela, o mapa estrutural não pôde ser lido (`erroCompletude`). */
  completude: Completude | null;
  erroCompletude: string | null;
  /** `funcionalidades/mapa.md`, relativo à base, quando existe. */
  documento: string | null;
  versao: number;
}

/** Decisão do analista (`cba_tools::mapear::RevisaoDoMapa`). */
export type RevisaoDoMapa =
  | { tipo: "decidir"; funcionalidade: string; decisao: "aceitar" | "descartar" | "desfazer" }
  | { tipo: "editar"; funcionalidade: string; nome: string; area: string; descricao: string }
  | { tipo: "juntar"; origem: string; destino: string }
  | {
      tipo: "dividir";
      funcionalidade: string;
      /** Índices (a partir de 0) dos pontos de entrada que vão para a nova. */
      pontos: number[];
      nome: string;
      descricao: string;
    }
  | { tipo: "desfazer-justificativa"; ponto: string }
  | { tipo: "ligar-demanda"; funcionalidade: string; demanda: string; ligar: boolean };

/** Painel da funcionalidade (`painel_funcionalidade.rs`). */
export type Ligacao = "explica" | "funcionalidade" | "demanda" | "codigo";

export const NOME_LIGACAO: Record<Ligacao, string> = {
  explica: "explica esta",
  funcionalidade: "aberta aqui",
  demanda: "pela demanda",
  codigo: "pelo código",
};

export interface AnaliseNoPainel extends AnaliseResumo {
  ligacao: Ligacao;
  demanda: string | null;
  perguntasAbertas: number;
}

export interface PerguntaNoPainel {
  analise: string;
  numero: number;
  texto: string;
  destinatario: string | null;
  demanda: string | null;
}

export interface EtapaDemanda {
  analise: string;
  estado: EstadoAnalise;
}

export interface DemandaNoPainel {
  id: string;
  titulo: string;
  aberta: boolean;
  criado: string;
  ligadaNoPainel: boolean;
  impacto: EtapaDemanda | null;
  historias: EtapaDemanda | null;
  cenarios: EtapaDemanda | null;
  perguntas: PerguntaNoPainel[];
}

export interface RegraNoPainel {
  id: string;
  titulo: string;
  /** Enunciado completo (a seção "Enunciado" do documento). */
  enunciado: string;
  status: StatusRegra;
  caminho: string;
}

export interface PainelFuncionalidade {
  funcionalidade: Funcionalidade;
  analises: AnaliseNoPainel[];
  demandas: DemandaNoPainel[];
  regras: RegraNoPainel[];
  perguntas: PerguntaNoPainel[];
  aRevisar: string[];
  relacionadas: { id: string; nome: string }[];
  atividade: { quandoMs: number; texto: string }[];
  demandasDoProjeto: { id: string; titulo: string }[];
}

export interface ApiMapa {
  mapa: (projeto: string) => Promise<MapaFuncionalidades>;
  revisar: (projeto: string, revisao: RevisaoDoMapa) => Promise<MapaFuncionalidades>;
  /** Pedido que abre a sessão de mapeamento no terminal do Claude. */
  pedido: (pasta: string | null, area: string | null, orientacao: string | null) => Promise<string>;
  explicar: (projeto: string, funcionalidade: string) => Promise<AnaliseCriada>;
  painel: (projeto: string, funcionalidade: string) => Promise<PainelFuncionalidade>;
  /** Análise de um fluxo aberta pelo painel, com a funcionalidade guardada nela. */
  criarAnalise: (
    projeto: string,
    funcionalidade: string,
    fluxo: Fluxo,
    texto: string | null,
    demanda: string | null,
  ) => Promise<AnaliseCriada>;
  /** Diálogo de salvar; `null` se o analista cancelar. */
  escolherDestino: (formato: "markdown" | "word") => Promise<string | null>;
  /** Exporta o documento do mapa e devolve o caminho gravado. */
  exportar: (projeto: string, destino: string, formato: "markdown" | "word") => Promise<string>;
}

export const apiMapaTauri: ApiMapa = {
  mapa: (projeto) => invoke("mapa_funcionalidades_projeto", { projeto }),
  revisar: (projeto, revisao) => invoke("revisar_mapa_projeto", { projeto, revisao }),
  pedido: (pasta, area, orientacao) => invoke("pedido_de_mapeamento", { pasta, area, orientacao }),
  explicar: (projeto, funcionalidade) =>
    invoke("explicar_funcionalidade_app", { projeto, funcionalidade }),
  painel: (projeto, funcionalidade) =>
    invoke("painel_funcionalidade_app", { projeto, funcionalidade }),
  criarAnalise: (projeto, funcionalidade, fluxo, texto, demanda) =>
    invoke("criar_analise_da_funcionalidade_app", {
      projeto,
      funcionalidade,
      fluxo,
      texto,
      demanda,
    }),
  escolherDestino: (formato) =>
    save({
      title: "Exportar o mapa de funcionalidades",
      defaultPath: formato === "word" ? "mapa-funcionalidades.docx" : "mapa-funcionalidades.md",
      filters: [
        formato === "word"
          ? { name: "Documento do Word", extensions: ["docx"] }
          : { name: "Markdown", extensions: ["md"] },
      ],
    }),
  exportar: (projeto, destino, formato) =>
    invoke("exportar_mapa_projeto", { projeto, destino, formato }),
};
