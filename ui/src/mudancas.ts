import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

/** O que mudou no código entre dois pontos do repositório (#124): conta, sem IA. */

export type Classe = "inalterado" | "reformatado" | "movido" | "novo" | "alterado" | "removido";

export const ROTULO_CLASSE: Record<Classe, string> = {
  inalterado: "trecho sem mudança",
  reformatado: "só reformatado",
  movido: "trecho movido",
  novo: "trecho novo",
  alterado: "trecho alterado",
  removido: "trecho removido",
};

/** A mudança pode ter mudado o comportamento. */
export function mudou(c: Classe): boolean {
  return c === "alterado" || c === "removido";
}

export type SituacaoArquivo = "adicionado" | "modificado" | "renomeado" | "removido";

export interface Ponto {
  referencia: string;
  commit: string | null;
  analise?: { id: string; titulo: string };
}

export interface ArquivoAlterado {
  arquivo: string;
  situacao: SituacaoArquivo;
  anterior?: string;
}

export interface ArquivoComEvidencias extends ArquivoAlterado {
  regras: string[];
  termos: string[];
  funcionalidades: string[];
  analises: string[];
}

export interface TrechoAfetado {
  arquivo: string;
  linhas: string;
  linhas_novas?: string;
  classe: Classe;
  diff: string;
}

export interface RegraAfetada {
  id: string;
  titulo: string;
  area: string;
  status: string;
  documento: string;
  classe: Classe;
  trechos: TrechoAfetado[];
}

export interface GrupoSemEvidencia {
  /** Ausente quando o grupo é a pasta. */
  area?: string;
  nome: string;
  arquivos: ArquivoAlterado[];
}

export interface RelatorioDeMudancas {
  projeto: string;
  de: Ponto;
  ate: Ponto;
  commits: { hash: string; data: string; assunto: string }[];
  arquivos_alterados: number;
  com_evidencias: ArquivoComEvidencias[];
  regras: RegraAfetada[];
  termos: { termo: string; analise: string; classe: Classe }[];
  funcionalidades: { id: string; nome: string; area: string; area_nome: string; classe: Classe }[];
  analises: { id: string; titulo: string; estado: string; itens: unknown[] }[];
  sem_evidencias: GrupoSemEvidencia[];
  problemas: string[];
  duracao_ms: number;
}

export interface PropostaDeRegra {
  numero: number;
  regra: string;
  comportamento_mudou: boolean;
  explicacao: string;
  titulo?: string;
  enunciado?: string;
  situacao: "pendente" | "aplicada" | "descartada";
}

export interface RegraAtualizada {
  id: string;
  documento: string;
  versoes_anteriores: number;
  avisos: string[];
}

export interface ApiMudancas {
  relatorio(projeto: string, de: string | null, ate: string | null): Promise<RelatorioDeMudancas>;
  /** Diálogo de salvar; `null` se o analista cancelar. */
  escolherDestino(): Promise<string | null>;
  /** Grava a nota de mudanças e devolve o caminho gravado. */
  exportarNota(projeto: string, de: string | null, ate: string | null, destino: string): Promise<string>;
  /** O pedido que inicia "explicar mudanças" com o Claude. */
  pedido(de: string | null, ate: string | null): Promise<string>;
  propostas(projeto: string): Promise<PropostaDeRegra[]>;
  confirmar(
    projeto: string,
    numero: number,
    titulo: string | null,
    enunciado: string | null,
  ): Promise<RegraAtualizada>;
  descartar(projeto: string, numero: number): Promise<void>;
}

export const apiMudancasTauri: ApiMudancas = {
  relatorio: (projeto, de, ate) => invoke("mudancas_projeto_app", { projeto, de, ate }),
  escolherDestino: () =>
    save({
      title: "Exportar a nota de mudanças",
      defaultPath: "nota-de-mudancas.md",
      filters: [{ name: "Markdown", extensions: ["md"] }],
    }),
  exportarNota: (projeto, de, ate, destino) =>
    invoke("exportar_nota_de_mudancas_app", { projeto, de, ate, destino }),
  pedido: (de, ate) => invoke("pedido_de_explicacao_app", { de, ate }),
  propostas: (projeto) => invoke("propostas_de_mudanca_app", { projeto }),
  confirmar: (projeto, numero, titulo, enunciado) =>
    invoke("confirmar_proposta_de_mudanca_app", { projeto, numero, titulo, enunciado }),
  descartar: (projeto, numero) => invoke("descartar_proposta_de_mudanca_app", { projeto, numero }),
};
