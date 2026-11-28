import { invoke } from "@tauri-apps/api/core";
import type { Metodologia } from "./entregaveis";
import type { SituacaoFuncionalidade } from "./funcionalidades";
import { contar } from "./formatacao";

/**
 * Visão "Documentação" do mapa (#462): os documentos vivos por funcionalidade e
 * tipo, a cobertura por tipo e a geração da versão inicial em lote. Espelho de
 * `ui/src-tauri/src/documentacao_do_mapa.rs` e de
 * `cba_tools::documentacao_do_mapa`.
 */

/** Quem monta o tipo: o app, na hora, ou o agente, numa sessão por funcionalidade. */
export type Geracao = "app" | "agente";

/** Estado de um documento vivo (`cba_core::documentos_vivos::EstadoDocumento`). */
export type EstadoDocumento =
  | { estado: "em_dia"; versao: string }
  | { estado: "proposta_pendente"; versao: string; revisada: boolean }
  | { estado: "desatualizado"; versao: string; arquivos: string[] }
  | { estado: "sem_documento" };

/** Uma célula: o documento de um tipo numa funcionalidade. */
export interface DocumentoDoTipo {
  tipo: string;
  /** Caminho na base, relativo ao projeto. */
  caminho?: string;
  /** Versão publicada. */
  versao?: string;
  estado: EstadoDocumento;
  acao?: "Gerar versão inicial" | "Revisar proposta" | "Propor atualização";
}

export interface ColunaDeDocumentacao {
  tipo: string;
  nome: string;
  geracao: Geracao;
}

export interface LinhaDeDocumentacao {
  funcionalidade: string;
  nome: string;
  area: string;
  areaNome: string;
  situacao: SituacaoFuncionalidade;
  /** Na ordem das colunas. */
  documentos: DocumentoDoTipo[];
}

/** Cobertura de um tipo (`cba_tools::cobertura::CoberturaDoTipo`). */
export interface CoberturaDoTipo {
  tipo: string;
  nome: string;
  funcionalidades: number;
  comDocumento: number;
  emDia: number;
  desatualizados: number;
  propostasPendentes: number;
  semDocumento: number;
}

export interface TabelaDeDocumentacao {
  metodologia: Metodologia;
  colunas: ColunaDeDocumentacao[];
  linhas: LinhaDeDocumentacao[];
  cobertura: CoberturaDoTipo[];
}

export type ItemDoLote =
  | { resultado: "gerada"; funcionalidade: string; caminho: string }
  | { resultado: "sessao"; funcionalidade: string; pedido: string }
  | { resultado: "pulada"; funcionalidade: string; motivo: string };

export interface Lote {
  tipo: string;
  geracao: Geracao;
  itens: ItemDoLote[];
}

export type Marca = "mantido" | "novo" | "alterado" | "removido";

export interface Trecho {
  marca: Marca;
  texto: string;
  anterior?: string;
}

/** Proposta de versão (`cba_core::documentos_vivos::Proposta`, campos em snake_case). */
export interface Proposta {
  versao: string;
  base?: string;
  texto: string;
  trechos: Trecho[];
  mudancas: string;
  arquivos?: string[];
  revisada?: { por: string; em_ms: number };
  criada_ms: number;
  atualizada_ms: number;
}

export interface PropostaVista {
  caminho: string;
  tipo: string;
  nomeDoTipo: string;
  funcionalidade: string | null;
  publicada: string | null;
  proposta: Proposta;
}

export interface ApiDocumentacao {
  tabela(projeto: string): Promise<TabelaDeDocumentacao>;
  gerar(projeto: string, tipo: string, funcionalidades: string[]): Promise<Lote>;
  proposta(projeto: string, documento: string): Promise<PropostaVista>;
  marcarRevisada(projeto: string, documento: string): Promise<Proposta>;
  publicar(projeto: string, documento: string): Promise<unknown>;
  descartar(projeto: string, documento: string): Promise<void>;
  /** "Propor atualização" de um tipo montado pelo app; devolve o caminho do documento. */
  proporAtualizacao(projeto: string, tipo: string, funcionalidade: string): Promise<string>;
}

export const apiDocumentacaoTauri: ApiDocumentacao = {
  tabela: (projeto) => invoke("documentacao_do_mapa_app", { projeto }),
  gerar: (projeto, tipo, funcionalidades) =>
    invoke("gerar_versoes_iniciais_app", { projeto, tipo, funcionalidades }),
  proposta: (projeto, documento) => invoke("proposta_documento_vivo_app", { projeto, documento }),
  marcarRevisada: (projeto, documento) =>
    invoke("marcar_documento_vivo_revisado_app", { projeto, documento }),
  publicar: (projeto, documento) => invoke("publicar_documento_vivo_app", { projeto, documento }),
  descartar: (projeto, documento) =>
    invoke("descartar_proposta_documento_vivo_app", { projeto, documento }),
  proporAtualizacao: (projeto, tipo, funcionalidade) =>
    invoke("propor_atualizacao_documento_vivo_app", { projeto, tipo, funcionalidade }),
};

/** Rótulo do estado, como na legenda. */
export const ROTULO_ESTADO: Record<EstadoDocumento["estado"], string> = {
  em_dia: "em dia",
  proposta_pendente: "proposta pendente",
  desatualizado: "desatualizado",
  sem_documento: "sem documento",
};

/** O que a célula mostra como documento: a versão publicada ou o rascunho. */
export function rotuloDoDocumento(d: DocumentoDoTipo): string | null {
  const e = d.estado;
  if (e.estado === "sem_documento") return null;
  if (e.estado === "proposta_pendente")
    return d.versao ? `v${d.versao} → v${e.versao}` : `rascunho v${e.versao}`;
  return `v${e.versao}`;
}

/** Resumo da cobertura de um tipo: "4 de 6 funcionalidades". */
export function resumoDaCobertura(c: CoberturaDoTipo): string {
  return `${c.comDocumento} de ${contar(c.funcionalidades, "funcionalidade", "funcionalidades")}`;
}

/** Os alertas de um tipo: desatualizados e propostas pendentes. */
export function alertasDaCobertura(c: CoberturaDoTipo): string {
  return [
    c.desatualizados > 0 && contar(c.desatualizados, "desatualizado", "desatualizados"),
    c.propostasPendentes > 0 &&
      contar(c.propostasPendentes, "proposta pendente", "propostas pendentes"),
  ]
    .filter(Boolean)
    .join(" · ");
}

/** Resumo do lote do app, para o aviso. */
export function resumoDoLote(lote: Lote): string {
  const geradas = lote.itens.filter((i) => i.resultado === "gerada").length;
  const puladas = lote.itens.filter((i) => i.resultado === "pulada");
  const partes = [];
  if (geradas > 0)
    partes.push(
      `${contar(geradas, "rascunho gerado", "rascunhos gerados")} (v1.0), à espera da revisão final.`,
    );
  if (puladas.length > 0)
    partes.push(
      `Ficaram de fora: ${puladas.map((p) => `${p.funcionalidade} (${p.motivo})`).join("; ")}.`,
    );
  return partes.join(" ") || "Nada a gerar.";
}
