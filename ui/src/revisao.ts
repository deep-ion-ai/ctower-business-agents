import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { Gherkin, Narrativa, OrigemPergunta, SituacaoItem, VistaAnalise } from "./analise";

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
  | ({ tipo: "editar-criterio"; historia: string; numero: number } & Gherkin)
  /** Cenários de teste (#117): aceitar, descartar ou desfazer; editar título e passos. */
  | { tipo: "cenario"; cenario: string; situacao: SituacaoItem }
  | ({ tipo: "editar-cenario"; cenario: string; titulo: string } & Gherkin)
  | { tipo: "tabela"; numero: number; situacao: SituacaoItem }
  /** "Virar pergunta" (#119, #120): pergunta do analista a partir de um item da análise, com o texto editado e as evidências do item. */
  | { tipo: "virar-pergunta"; texto: string; origem: OrigemPergunta };

export interface Area {
  id: string;
  nome: string;
  sigla?: string | null;
}

export interface Pendencia {
  tipo: "analise" | "afirmacao" | "termo" | "historia" | "criterio" | "tabela" | "cenario";
  numero: number | null;
  motivo: string;
}

export interface RegraPrevista {
  afirmacao: number;
  /** Título derivado do texto (sem markdown), que o analista pode trocar. */
  titulo: string;
  texto: string;
  /** Seção da afirmação (o marcador, como `regras_encontradas`). */
  secao?: string;
  /** Título da seção no documento ("Regras encontradas"); vazio se desconhecida. */
  secaoTitulo?: string;
  /** Vem marcada para virar regra: as respostas e os exemplos não vêm. */
  sugerida?: boolean;
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
  /** Afirmações que viram regra; sem a lista, as de fora da resposta e dos exemplos. */
  regras?: number[] | null;
}

export type Acao = "criada" | "atualizada" | "sem-mudanca";

export interface AprovacaoFeita {
  jaAprovada: boolean;
  regras: { id: string; arquivo: string; area: string; afirmacoes: number[]; acao: Acao }[];
  termos: { termo: string; arquivo: string; acao: Acao }[];
  avisos: string[];
}

/** Formato da exportação; `csv` (uma linha por cenário) só nos cenários de teste (#117). */
export type FormatoExportacao = "markdown" | "word" | "csv";

export interface OpcoesExportacao {
  /** Notas de rodapé e seção de evidências. */
  evidencias: boolean;
  /** Perguntas em aberto para o solicitante. */
  perguntas: boolean;
  /** Apêndice com os trechos de código citados. */
  trechos: boolean;
  /** Word: apêndice com o código Mermaid dos diagramas desenhados (#118). */
  codigo_diagramas: boolean;
}

/** Figura de um diagrama renderizada pela tela, para o Word (#118). */
export interface FiguraPedida {
  /** Código do bloco Mermaid, como {@link ApiRevisao.diagramasExportacao} devolveu. */
  codigo: string;
  /** Bytes do PNG. */
  png: number[];
  /** Tamanho de exibição, em pixels CSS. */
  largura: number;
  altura: number;
}

export interface PedidoExportacao {
  destino: string;
  formato: FormatoExportacao;
  opcoes: OpcoesExportacao;
  /** Word: as figuras dos diagramas; bloco sem figura sai como aviso e código. */
  figuras?: FiguraPedida[];
}

export const EXTENSAO: Record<FormatoExportacao, string> = {
  markdown: "md",
  word: "docx",
  csv: "csv",
};

const FILTRO: Record<FormatoExportacao, { name: string; extensions: string[] }> = {
  markdown: { name: "Markdown", extensions: ["md"] },
  word: { name: "Documento do Word", extensions: ["docx"] },
  csv: { name: "CSV (planilha, ferramentas de teste)", extensions: ["csv"] },
};

/** Operações da revisão. A interface usa {@link apiRevisaoTauri}; os testes, uma falsa. */
export interface ApiRevisao {
  revisar(projeto: string, analise: string, decisao: DecisaoRevisao): Promise<VistaAnalise>;
  prontidao(projeto: string, analise: string): Promise<Prontidao>;
  aprovar(projeto: string, analise: string, opcoes: OpcoesAprovacao): Promise<AprovacaoFeita>;
  /** Diálogo de salvar do projeto; `null` se o analista cancelar. */
  escolherDestino(nome: string, formato: FormatoExportacao): Promise<string | null>;
  /** Exporta e devolve o caminho gravado. */
  exportar(projeto: string, analise: string, pedido: PedidoExportacao): Promise<string>;
  /**
   * Código dos blocos Mermaid do documento que a exportação vai ler (#118), para a tela
   * desenhar as figuras do Word. Sem ela, o Word sai com aviso e código no lugar.
   */
  diagramasExportacao?(projeto: string, analise: string): Promise<string[]>;
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
      filters: [FILTRO[formato]],
    }),
  exportar: (projeto, analise, pedido) =>
    invoke("exportar_analise_projeto", { projeto, analise, pedido }),
  diagramasExportacao: (projeto, analise) =>
    invoke("diagramas_exportacao_projeto", { projeto, analise }),
};
