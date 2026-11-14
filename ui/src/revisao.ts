import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { Gherkin, Narrativa, SituacaoItem, VistaAnalise } from "./analise";

/**
 * Revisão, aprovação e exportação da análise (#37): espelho de
 * `cba_tools::revisao` e dos comandos de `ui/src-tauri/src/revisao.rs`.
 */

/** Decisão do analista (`cba_tools::revisao::Decisao`). */
export type DecisaoRevisao =
  | { tipo: "afirmacao"; numero: number; estado: "confirmada" | "registrada" }
  /** As respostas destas perguntas foram enviadas ao agente. */
  | { tipo: "respostas-enviadas"; numeros: number[] }
  /** Aprofundamento, com o que o analista quer ver detalhado (opcional). */
  | { tipo: "afirmacao"; numero: number; estado: "aprofundar"; pedido?: string }
  | { tipo: "afirmacao"; numero: number; estado: "incorreta"; comentario: string }
  | { tipo: "editar-afirmacao"; numero: number; texto: string }
  | { tipo: "pergunta"; numero: number; estado: "respondida"; resposta: string }
  | { tipo: "pergunta"; numero: number; estado: "descartada" }
  | { tipo: "sugestao"; numero: number; estado: "aceita" | "recusada" }
  | { tipo: "termo"; numero: number; estado: "aceito" | "recusado" }
  | { tipo: "editar-termo"; numero: number; definicao: string }
  /** Achado do parecer de revisão: descartar, com o motivo, ou desfazer o descarte. */
  | { tipo: "achado"; numero: number; estado: "descartado"; motivo?: string }
  | { tipo: "achado"; numero: number; estado: "registrado" }
  /** Histórias (#115): aceitar, descartar, desfazer ou pedir a quebra em fatias. */
  | { tipo: "historia"; historia: string; decisao: "aceitar" | "descartar" | "desfazer" }
  | { tipo: "historia"; historia: string; decisao: "quebrar"; pedido?: string }
  | ({ tipo: "editar-historia"; historia: string } & Narrativa)
  | { tipo: "criterio"; historia: string; numero: number; situacao: SituacaoItem }
  | ({ tipo: "editar-criterio"; historia: string; numero: number } & Gherkin);

export interface Area {
  id: string;
  nome: string;
  sigla?: string | null;
}

export interface Pendencia {
  tipo: "analise" | "afirmacao" | "termo" | "historia" | "criterio";
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
  /** Mais de uma área no `projeto.md`: a área precisa ser escolhida. */
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
  revisar(projeto: string, analise: string, decisao: DecisaoRevisao): Promise<VistaAnalise>;
  prontidao(projeto: string, analise: string): Promise<Prontidao>;
  aprovar(projeto: string, analise: string, opcoes: OpcoesAprovacao): Promise<AprovacaoFeita>;
  /** Diálogo de salvar do projeto; `null` se o analista cancelar. */
  escolherDestino(nome: string, formato: FormatoExportacao): Promise<string | null>;
  /** Exporta e devolve o caminho gravado. */
  exportar(projeto: string, analise: string, pedido: PedidoExportacao): Promise<string>;
}

export const apiRevisaoTauri: ApiRevisao = {
  revisar: (projeto, analise, decisao) =>
    invoke("revisar_analise_projeto", { projeto, analise, decisao }),
  prontidao: (projeto, analise) => invoke("prontidao_aprovacao_projeto", { projeto, analise }),
  aprovar: (projeto, analise, opcoes) =>
    invoke("aprovar_analise_projeto", { projeto, analise, opcoes }),
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
  exportar: (projeto, analise, pedido) =>
    invoke("exportar_analise_projeto", { projeto, analise, pedido }),
};
