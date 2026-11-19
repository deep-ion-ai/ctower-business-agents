import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { AnaliseCriada } from "./analise";

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
  /** Análise de explicação ligada, se houver. */
  explicacao: string | null;
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
  | { tipo: "desfazer-justificativa"; ponto: string };

export interface ApiMapa {
  mapa: (projeto: string) => Promise<MapaFuncionalidades>;
  revisar: (projeto: string, revisao: RevisaoDoMapa) => Promise<MapaFuncionalidades>;
  /** Pedido que abre a sessão de mapeamento no terminal do Claude. */
  pedido: (pasta: string | null, area: string | null, orientacao: string | null) => Promise<string>;
  explicar: (projeto: string, funcionalidade: string) => Promise<AnaliseCriada>;
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
