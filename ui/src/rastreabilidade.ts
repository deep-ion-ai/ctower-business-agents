import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

/**
 * Rastreabilidade (#121): a matriz da demanda e a consulta reversa por regra ou
 * arquivo. Espelha `cba_kb::indice::rastro` (campos em camelCase) e os comandos de
 * `ui/src-tauri/src/rastreabilidade.rs`. Tudo é derivado do índice da base: nada
 * aqui é editado à mão.
 */

export type Alerta =
  | "evidencia-a-revalidar"
  | "regra-sem-historia"
  | "historia-sem-cenario"
  | "historia-sem-regra";

export const ROTULO_ALERTA: Record<Alerta, string> = {
  "evidencia-a-revalidar": "evidência a revalidar",
  "regra-sem-historia": "regra afetada sem história",
  "historia-sem-cenario": "sem cenário",
  "historia-sem-regra": "sem regra",
};

/** Título do cartão de cada lacuna no painel lateral. */
export const TITULO_LACUNA: Record<Alerta, string> = {
  "evidencia-a-revalidar": "Evidência mudou sob história aprovada",
  "regra-sem-historia": "Regra afetada sem história",
  "historia-sem-cenario": "História sem cenário",
  "historia-sem-regra": "História sem regra",
};

export interface DemandaRastro {
  id: string;
  titulo: string;
  status: string | null;
  documento: string | null;
}

export interface AnaliseRastro {
  id: string;
  titulo: string;
  fluxo: string | null;
  estado: string | null;
  documento: string | null;
  origem: string | null;
  demanda: string | null;
}

export interface RegraRastro {
  id: string;
  titulo: string;
  status: string | null;
  documento: string | null;
}

export interface EvidenciaRastro {
  arquivo: string;
  linhaInicial: number;
  linhaFinal: number;
  aRevalidar: boolean;
  /** A afirmação (`A-0001/2`), a regra ou o item de onde a evidência veio. */
  de: string;
}

export interface CriterioRastro {
  historia: string;
  criterio: string;
  texto: string;
  analise: string;
  situacao: string | null;
  aRevalidar: boolean;
  aConfirmar: boolean;
}

export interface CenarioRastro {
  id: string;
  titulo: string;
  analise: string;
  historia: string | null;
  situacao: string | null;
  aRevalidar: boolean;
  aConfirmar: boolean;
}

export interface LinhaHistoria {
  id: string;
  titulo: string;
  analise: string;
  situacao: string | null;
  aprovada: boolean;
  aConfirmar: boolean;
  analises: string[];
  criterios: CriterioRastro[];
  afirmacoes: string[];
  regras: RegraRastro[];
  evidencias: EvidenciaRastro[];
  cenarios: CenarioRastro[];
  alertas: Alerta[];
}

export interface LinhaRegra {
  regra: RegraRastro;
  analises: string[];
  historias: string[];
  evidencias: EvidenciaRastro[];
  cenarios: CenarioRastro[];
  alertas: Alerta[];
}

export interface Lacuna {
  alerta: Alerta;
  alvo: string;
  descricao: string;
  relacionados: string[];
}

export interface Matriz {
  projeto: string;
  demanda: DemandaRastro;
  analises: AnaliseRastro[];
  historias: LinhaHistoria[];
  regras: LinhaRegra[];
  lacunas: Lacuna[];
}

export type AlvoReverso = { tipo: "regra"; valor: string } | { tipo: "arquivo"; valor: string };

export interface Dependentes {
  projeto: string;
  alvo: AlvoReverso;
  regra: RegraRastro | null;
  /** Arquivos das evidências da regra (consulta por regra). */
  arquivos: string[];
  /** Regras com evidência no arquivo (consulta por arquivo). */
  regras: RegraRastro[];
  demandas: DemandaRastro[];
  analises: AnaliseRastro[];
  criterios: CriterioRastro[];
  cenarios: CenarioRastro[];
}

export type FormatoMatriz = "xlsx" | "word";

export const EXTENSAO_MATRIZ: Record<FormatoMatriz, string> = { xlsx: "xlsx", word: "docx" };

export interface ApiRastreabilidade {
  matriz(projeto: string, demanda: string): Promise<Matriz>;
  /** `projeto` é a pasta do projeto na base. */
  dependentes(projeto: string, alvo: AlvoReverso): Promise<Dependentes>;
  /** Diálogo de salvar; `null` se o analista cancelou. */
  escolherDestino(nome: string, formato: FormatoMatriz): Promise<string | null>;
  /** Exporta e devolve o caminho gravado. */
  exportar(
    projeto: string,
    demanda: string,
    formato: FormatoMatriz,
    destino: string,
  ): Promise<string>;
}

// As funções são `async` para devolverem sempre uma promessa, mesmo com o
// `invoke` substituído nos testes.
export const apiRastreabilidadeTauri: ApiRastreabilidade = {
  matriz: async (projeto, demanda) => invoke("matriz_demanda_app", { projeto, demanda }),
  dependentes: async (projeto, alvo) => invoke("dependentes_base_app", { projeto, alvo }),
  escolherDestino: async (nome, formato) =>
    save({
      title: "Exportar a matriz de rastreabilidade",
      defaultPath: `${nome}.${EXTENSAO_MATRIZ[formato]}`,
      filters: [
        formato === "xlsx"
          ? { name: "Planilha do Excel", extensions: ["xlsx"] }
          : { name: "Documento do Word", extensions: ["docx"] },
      ],
    }),
  exportar: async (projeto, demanda, formato, destino) =>
    invoke("exportar_matriz_app", { projeto, demanda, formato, destino }),
};

/** `arquivo:10-12` (ou `arquivo:10`). */
export function rotuloEvidencia(e: EvidenciaRastro): string {
  return e.linhaInicial === e.linhaFinal
    ? `${e.arquivo}:${e.linhaInicial}`
    : `${e.arquivo}:${e.linhaInicial}-${e.linhaFinal}`;
}
