import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { VistaAnalise } from "./analise";

/**
 * Revisão, aprovação e exportação da análise (#37): espelho de
 * `cba_tools::revisao` e dos comandos de `ui/src-tauri/src/revisao.rs`.
 */

/** Decisão do analista (`cba_tools::revisao::Decisao`). */
export type DecisaoRevisao =
  | { tipo: "afirmacao"; numero: number; estado: "confirmada" | "aprofundar" | "registrada" }
  | { tipo: "afirmacao"; numero: number; estado: "incorreta"; comentario: string }
  | { tipo: "editar-afirmacao"; numero: number; texto: string }
  | { tipo: "pergunta"; numero: number; estado: "respondida"; resposta: string }
  | { tipo: "pergunta"; numero: number; estado: "descartada" }
  | { tipo: "sugestao"; numero: number; estado: "aceita" | "recusada" }
  | { tipo: "termo"; numero: number; estado: "aceito" | "recusado" };

export interface Area {
  id: string;
  nome: string;
  sigla?: string | null;
}

export interface Pendencia {
  tipo: "analise" | "afirmacao" | "termo";
  numero: number | null;
  motivo: string;
}

export interface RegraPrevista {
  afirmacao: number;
  /** Título derivado do texto, que o analista pode trocar. */
  titulo: string;
  texto: string;
}

/** Se a análise pode ser aprovada e o que a aprovação fará. */
export interface Prontidao {
  estado: "criada" | "em-andamento" | "concluida" | "aprovada";
  podeAprovar: boolean;
  pendencias: Pendencia[];
  avisos: string[];
  areas: Area[];
  /** Mais de uma área no `sistema.md`: a área precisa ser escolhida. */
  areaObrigatoria: boolean;
  regras: RegraPrevista[];
  incorretas: number[];
  termosAceitos: string[];
}

/** Escolhas da aprovação (`cba_tools::aprovacao::OpcoesAprovacao`). */
export interface OpcoesAprovacao {
  area: string | null;
  afirmacoes: Record<number, { area: string | null; titulo: string | null }>;
}

export type Acao = "criada" | "atualizada" | "sem-mudanca";

export interface AprovacaoFeita {
  jaAprovada: boolean;
  regras: { id: string; arquivo: string; area: string; afirmacoes: number[]; acao: Acao }[];
  termos: { termo: string; arquivo: string; acao: Acao }[];
  avisos: string[];
}

export type FormatoExportacao = "markdown" | "word";

export interface OpcoesExportacao {
  /** Notas de rodapé e seção de evidências. */
  evidencias: boolean;
  /** Perguntas em aberto para o solicitante. */
  perguntas: boolean;
  /** Apêndice com os trechos de código citados. */
  trechos: boolean;
}

export interface PedidoExportacao {
  destino: string;
  formato: FormatoExportacao;
  opcoes: OpcoesExportacao;
}

export const EXTENSAO: Record<FormatoExportacao, string> = { markdown: "md", word: "docx" };

/** Operações da revisão. A interface usa {@link apiRevisaoTauri}; os testes, uma falsa. */
export interface ApiRevisao {
  revisar(sistema: string, analise: string, decisao: DecisaoRevisao): Promise<VistaAnalise>;
  prontidao(sistema: string, analise: string): Promise<Prontidao>;
  aprovar(sistema: string, analise: string, opcoes: OpcoesAprovacao): Promise<AprovacaoFeita>;
  /** Diálogo de salvar do sistema; `null` se o analista cancelar. */
  escolherDestino(nome: string, formato: FormatoExportacao): Promise<string | null>;
  /** Exporta e devolve o caminho gravado. */
  exportar(sistema: string, analise: string, pedido: PedidoExportacao): Promise<string>;
}

export const apiRevisaoTauri: ApiRevisao = {
  revisar: (sistema, analise, decisao) =>
    invoke("revisar_analise_sistema", { sistema, analise, decisao }),
  prontidao: (sistema, analise) => invoke("prontidao_aprovacao_sistema", { sistema, analise }),
  aprovar: (sistema, analise, opcoes) =>
    invoke("aprovar_analise_sistema", { sistema, analise, opcoes }),
  escolherDestino: (nome, formato) =>
    save({
      title: "Exportar a análise",
      defaultPath: `${nome}.${EXTENSAO[formato]}`,
      filters: [
        formato === "word"
          ? { name: "Documento do Word", extensions: ["docx"] }
          : { name: "Markdown", extensions: ["md"] },
      ],
    }),
  exportar: (sistema, analise, pedido) =>
    invoke("exportar_analise_sistema", { sistema, analise, pedido }),
};
