import { invoke } from "@tauri-apps/api/core";

/**
 * Banco de dados numa análise (#440): as conexões de banco do projeto, o banco ligado
 * na análise ("Usar o banco <nome> nesta análise") e as consultas do agente, cada uma
 * com a aprovação do analista. Espelha `cba_core::consultas` e
 * `ui/src-tauri/src/consultas.rs`. Nenhum valor do resultado chega aqui: o estado
 * guarda só o SQL, as colunas e a quantidade de linhas.
 */

/** Conexão e ambiente de uma consulta (`cba_core::banco::OrigemBanco`). */
export interface OrigemBanco {
  conexao: string;
  nome: string;
  tipo: string;
  ambiente?: string | null;
}

export interface BancoLigado {
  conexao: string;
  nome: string;
  ligado_ms: number;
  por?: string | null;
}

export type SituacaoConsulta =
  | "pendente"
  | "aprovada"
  | "recusada"
  | "executada"
  | "falhou"
  | "expirada";

export interface ConsultaDados {
  numero: number;
  objetivo: string;
  origem: OrigemBanco;
  sql_proposto: string;
  sql_aprovado?: string | null;
  situacao: SituacaoConsulta;
  pedida_ms: number;
  decidida_ms?: number | null;
  decidida_por?: string | null;
  motivo?: string | null;
  editada?: boolean;
  executada_ms?: number | null;
  linhas?: number | null;
  cortada?: boolean;
  colunas?: string[];
  mascaradas?: string[];
  duracao_ms?: number | null;
  erro?: string | null;
}

/** Resultado de consulta citado numa afirmação (`cba_core::consultas::EvidenciaDados`). */
export interface EvidenciaDados {
  consulta: number;
  origem: OrigemBanco;
  sql: string;
  executada_ms: number;
  linhas: number;
  resumo: string;
}

export type DecisaoConsulta =
  | { decisao: "executar" }
  | { decisao: "editar_e_executar"; sql: string }
  | { decisao: "recusar"; motivo: string };

/** Uma conexão de banco do projeto, como o backend a descreve. */
export interface BancoDoProjeto {
  id: string;
  nome: string;
  rotuloTipo: string;
  ambiente: string | null;
  cadastrada: boolean;
}

export interface ApiConsultasBanco {
  bancosProjeto(projeto: string): Promise<BancoDoProjeto[]>;
  salvarBancosProjeto(projeto: string, bancos: string[]): Promise<BancoDoProjeto[]>;
  /** `null` desliga o banco da análise. */
  ligarBancoAnalise(projeto: string, analise: string, conexao: string | null): Promise<void>;
  decidirConsulta(
    projeto: string,
    analise: string,
    numero: number,
    decisao: DecisaoConsulta,
  ): Promise<void>;
}

export const apiConsultasBancoTauri: ApiConsultasBanco = {
  bancosProjeto: (projeto) => invoke("bancos_projeto_app", { projeto }),
  salvarBancosProjeto: (projeto, bancos) =>
    invoke("salvar_bancos_projeto_app", { projeto, bancos }),
  ligarBancoAnalise: (projeto, analise, conexao) =>
    invoke("ligar_banco_analise_app", { projeto, analise, conexao }),
  decidirConsulta: (projeto, analise, numero, decisao) =>
    invoke("decidir_consulta_app", { projeto, analise, numero, decisao }),
};

/** As consultas que aguardam o analista. */
export function pendentes(consultas: ConsultaDados[] | undefined): ConsultaDados[] {
  return (consultas ?? []).filter((c) => c.situacao === "pendente");
}

export const ROTULO_SITUACAO: Record<SituacaoConsulta, string> = {
  pendente: "aguardando sua aprovação",
  aprovada: "aprovada, executando",
  recusada: "recusada",
  executada: "executada",
  falhou: "falhou",
  expirada: "expirada",
};

/** "Banco de homologação (PostgreSQL, homologação)". */
export function descreverOrigem(o: OrigemBanco): string {
  return `${o.nome} (${o.tipo}${o.ambiente ? `, ${o.ambiente}` : ""})`;
}

/** Data e hora locais curtas (`26/11/2026 14:03`). */
export function quando(ms: number | null | undefined): string {
  if (!ms) return "";
  const d = new Date(ms);
  const dois = (n: number) => String(n).padStart(2, "0");
  return `${dois(d.getDate())}/${dois(d.getMonth() + 1)}/${d.getFullYear()} ${dois(d.getHours())}:${dois(d.getMinutes())}`;
}
