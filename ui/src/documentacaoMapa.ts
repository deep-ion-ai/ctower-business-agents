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
  /** A versão inicial só sai com regras ligadas à funcionalidade (cada etapa cita uma). */
  exigeRegras: boolean;
}

export interface LinhaDeDocumentacao {
  funcionalidade: string;
  nome: string;
  area: string;
  areaNome: string;
  situacao: SituacaoFuncionalidade;
  /** Regras da base ligadas à funcionalidade. */
  regras: number;
  /** A explicação foi aprovada (o documento negocial está na base). */
  explicada: boolean;
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

/** Documento vivo habilitado que o app ainda não gera: fica fora das colunas. */
export interface TipoEmBreve {
  tipo: string;
  nome: string;
}

export interface TabelaDeDocumentacao {
  metodologia: Metodologia;
  colunas: ColunaDeDocumentacao[];
  emBreve: TipoEmBreve[];
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

// ---------------------------------------------------------------------------
// Estado da célula, pré-requisitos e elegibilidade do lote (#499)

/** O que a célula diz ao analista: o estado do documento ou, sem documento, se dá para gerar. */
export type EstadoDaCelula = "em_dia" | "a_revisar" | "desatualizado" | "pronta" | "aguardando";

export const ESTADOS_DA_CELULA: EstadoDaCelula[] = [
  "em_dia",
  "a_revisar",
  "desatualizado",
  "pronta",
  "aguardando",
];

/** Rótulos dos filtros e da cobertura, no plural de "funcionalidades". */
export const ROTULO_FILTRO: Record<EstadoDaCelula, string> = {
  em_dia: "Em dia",
  a_revisar: "A revisar",
  desatualizado: "Desatualizadas",
  pronta: "Prontas para gerar",
  aguardando: "Aguardando pré-requisito",
};

/** Como a cobertura conta cada estado: "14 em dia", "31 prontas para gerar". */
export function rotuloDaContagem(estado: EstadoDaCelula, n: number): string {
  switch (estado) {
    case "em_dia":
      return `${n} em dia`;
    case "a_revisar":
      return `${n} a revisar`;
    case "desatualizado":
      return contar(n, "desatualizado", "desatualizados");
    case "pronta":
      return `${n} ${n === 1 ? "pronta" : "prontas"} para gerar`;
    case "aguardando":
      return `${n} aguardando pré-requisito`;
  }
}

/**
 * O que falta para gerar a versão inicial, quando falta: o tipo exige regras e a
 * funcionalidade não tem. Sem a explicação aprovada, é ela que falta.
 */
export function pendencia(linha: LinhaDeDocumentacao, coluna: ColunaDeDocumentacao): string | null {
  if (!coluna.exigeRegras || linha.regras > 0) return null;
  return linha.explicada ? "sem regras na base" : "falta explicação";
}

export function estadoDaCelula(
  d: DocumentoDoTipo,
  linha: LinhaDeDocumentacao,
  coluna: ColunaDeDocumentacao,
): EstadoDaCelula {
  switch (d.estado.estado) {
    case "em_dia":
      return "em_dia";
    case "proposta_pendente":
      return "a_revisar";
    case "desatualizado":
      return "desatualizado";
    case "sem_documento":
      return pendencia(linha, coluna) ? "aguardando" : "pronta";
  }
}

/** Um pré-requisito da versão inicial, com o que diz e se impede a geração. */
export interface Prerequisito {
  nome: string;
  ok: boolean;
  detalhe: string;
  /** Sem ele, a versão inicial não sai. */
  impede: boolean;
}

/** Os pré-requisitos do tipo na funcionalidade, na ordem do fluxo: mapear, explicar, regras. */
export function prerequisitos(
  linha: LinhaDeDocumentacao,
  coluna: ColunaDeDocumentacao,
): Prerequisito[] {
  const aceita = linha.situacao === "aceita";
  const lista: Prerequisito[] = [
    {
      nome: "Mapeamento aceito",
      ok: aceita,
      detalhe: aceita
        ? "Pontos de entrada no código confirmados."
        : "Ainda a revisar no mapa; a geração não depende disso, mas a revisão ajuda.",
      impede: false,
    },
    {
      nome: "Explicação aprovada",
      ok: linha.explicada,
      detalhe: linha.explicada
        ? "O documento negocial está na base."
        : "A funcionalidade ainda não foi explicada, ou a explicação não foi aprovada.",
      impede: false,
    },
  ];
  if (coluna.exigeRegras)
    lista.push({
      nome: `Regras de negócio na base · ${linha.regras}`,
      ok: linha.regras > 0,
      detalhe: `Cada passo de ${coluna.nome} cita uma regra (RN-…). As regras saem da explicação aprovada.`,
      impede: true,
    });
  return lista;
}

/** Quantas funcionalidades selecionadas podem gerar o tipo, e por que as outras ficam de fora. */
export interface Elegibilidade {
  elegiveis: string[];
  fora: { motivo: string; funcionalidades: string[] }[];
}

export function elegibilidade(
  linhas: LinhaDeDocumentacao[],
  coluna: ColunaDeDocumentacao,
  indice: number,
): Elegibilidade {
  const elegiveis: string[] = [];
  const fora = new Map<string, string[]>();
  for (const l of linhas) {
    const d = l.documentos[indice];
    const motivo =
      d.estado.estado !== "sem_documento" ? "já tem documento" : pendencia(l, coluna);
    if (motivo) fora.set(motivo, [...(fora.get(motivo) ?? []), l.funcionalidade]);
    else elegiveis.push(l.funcionalidade);
  }
  return {
    elegiveis,
    fora: [...fora].map(([motivo, funcionalidades]) => ({ motivo, funcionalidades })),
  };
}

/** "2 falta explicação · 1 já tem documento". */
export function resumoDoFora(e: Elegibilidade): string {
  return e.fora.map((f) => `${f.funcionalidades.length} ${f.motivo}`).join(" · ");
}

/** Nome curto do tipo, para botões: "caso de uso", "catálogo de regras". */
export function nomeCurto(coluna: Pick<ColunaDeDocumentacao, "tipo" | "nome">): string {
  const curtos: Record<string, string> = {
    "caso-de-uso": "caso de uso",
    "catalogo-regras": "catálogo de regras",
    "especificacao-suplementar": "suplementar",
    manual: "manual",
    "documento-negocial": "documento negocial",
  };
  return curtos[coluna.tipo] ?? coluna.nome.toLowerCase();
}

/** Os rascunhos à espera da revisão final, na ordem das colunas e das linhas. */
export interface RascunhoNaFila {
  caminho: string;
  funcionalidade: string;
  nome: string;
  tipo: string;
  nomeDoTipo: string;
  rotulo: string;
  revisada: boolean;
}

export function filaDeRevisao(t: TabelaDeDocumentacao): RascunhoNaFila[] {
  return t.colunas.flatMap((c, i) =>
    t.linhas.flatMap((l) => {
      const d = l.documentos[i];
      if (d.estado.estado !== "proposta_pendente" || !d.caminho) return [];
      return [
        {
          caminho: d.caminho,
          funcionalidade: l.funcionalidade,
          nome: l.nome,
          tipo: c.tipo,
          nomeDoTipo: c.nome,
          rotulo: rotuloDoDocumento(d) ?? "",
          revisada: d.estado.revisada,
        },
      ];
    }),
  );
}

/** Título da aba da revisão: "Revisão · F-CRE-009 · Caso de uso". */
export function tituloDaRevisao(funcionalidade: string, coluna: Pick<ColunaDeDocumentacao, "tipo" | "nome">): string {
  const curto = nomeCurto(coluna);
  return `Revisão · ${funcionalidade} · ${curto.charAt(0).toUpperCase()}${curto.slice(1)}`;
}

/** Evento da janela: um documento vivo mudou (gerado, revisado, publicado, descartado). */
export const EVENTO_DOCUMENTACAO = "cba:documentacao-mudou";

export function avisarMudancaNaDocumentacao(): void {
  window.dispatchEvent(new Event(EVENTO_DOCUMENTACAO));
}
