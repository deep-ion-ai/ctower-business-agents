import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { AnaliseCriada, TipoEntrada } from "./analise";

/**
 * Mapa e catálogo de funcionalidades (#208): espelho dos comandos de
 * `ui/src-tauri/src/funcionalidades.rs` (campos em camelCase).
 */

export type SituacaoNoCatalogo = "nova" | "mantida" | "sem-ponto-de-entrada";

export const NOME_SITUACAO_CATALOGO: Record<SituacaoNoCatalogo, string> = {
  nova: "nova",
  mantida: "mantida",
  "sem-ponto-de-entrada": "sem ponto de entrada",
};

export interface FuncionalidadeCatalogo {
  id: string;
  nome: string;
  descricao: string;
  atores: string[];
  pontos: string[];
  situacao: SituacaoNoCatalogo;
  /** Documento negocial aprovado, relativo à raiz do projeto na base. */
  documento: string | null;
  /** Análise de explicação ligada, se houver. */
  explicacao: string | null;
}

export interface AreaCatalogo {
  area: string;
  nome: string;
  funcionalidades: FuncionalidadeCatalogo[];
}

export interface Catalogo {
  areas: AreaCatalogo[];
  total: number;
  documentadas: number;
  /** `funcionalidades/catalogo.md`, relativo à base, quando existe. */
  documento: string | null;
  origem: string | null;
}

export interface PontoSemTratamento {
  chave: string;
  tipo: TipoEntrada;
  nome: string;
  arquivo: string;
  linha: number;
}

export interface Completude {
  total: number;
  semTratamento: PontoSemTratamento[];
}

export interface ApiFuncionalidades {
  catalogo: (projeto: string) => Promise<Catalogo>;
  explicar: (projeto: string, funcionalidade: string) => Promise<AnaliseCriada>;
  completude: (projeto: string, analise: string) => Promise<Completude>;
  criarMapeamento: (
    projeto: string,
    pasta: string | null,
    area: string | null,
    entrada: string | null,
  ) => Promise<AnaliseCriada>;
  /** Diálogo de salvar; `null` se o analista cancelar. */
  escolherDestino: (formato: "markdown" | "word") => Promise<string | null>;
  /** Exporta o catálogo e devolve o caminho gravado. */
  exportar: (projeto: string, destino: string, formato: "markdown" | "word") => Promise<string>;
}

export const apiFuncionalidadesTauri: ApiFuncionalidades = {
  catalogo: (projeto) => invoke("catalogo_funcionalidades_projeto", { projeto }),
  explicar: (projeto, funcionalidade) =>
    invoke("explicar_funcionalidade_app", { projeto, funcionalidade }),
  completude: (projeto, analise) => invoke("completude_mapa_analise", { projeto, analise }),
  criarMapeamento: (projeto, pasta, area, entrada) =>
    invoke("criar_analise_de_mapeamento_app", { projeto, pasta, area, entrada }),
  escolherDestino: (formato) =>
    save({
      title: "Exportar o catálogo de funcionalidades",
      defaultPath: formato === "word" ? "catalogo-funcionalidades.docx" : "catalogo-funcionalidades.md",
      filters: [
        formato === "word"
          ? { name: "Documento do Word", extensions: ["docx"] }
          : { name: "Markdown", extensions: ["md"] },
      ],
    }),
  exportar: (projeto, destino, formato) =>
    invoke("exportar_catalogo_projeto", { projeto, destino, formato }),
};
