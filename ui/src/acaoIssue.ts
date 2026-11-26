import type { IssueDoProjeto, SituacaoNaBase } from "./issuesProjeto";
import { ROTULO_SITUACAO } from "./issuesProjeto";
import type { TomSelo } from "./componentes";

export interface Acao {
  rotulo: string;
  /** `analisar` cria a demanda e a análise; as outras só abrem. */
  tipo: "analisar" | "analise" | "demanda";
  /** Motivo de não poder agir, junto do botão. */
  bloqueio?: string;
}

/**
 * O que a issue oferece, pela situação na base: "Analisar impacto" sem análise,
 * "Abrir a análise" com análise em andamento ou em revisão e "Abrir a demanda" com a
 * análise aprovada. A issue encerrada no rastreador não se analisa (a base só liga
 * análise nova a demanda aberta): com demanda, abre-a; sem, explica.
 */
export function acaoDaIssue(i: IssueDoProjeto): Acao {
  if (i.situacao === "sem-analise") {
    if (i.estado === "aberta") return { rotulo: "Analisar impacto", tipo: "analisar" };
    if (i.demanda) return { rotulo: "Abrir a demanda", tipo: "demanda" };
    return {
      rotulo: "Analisar impacto",
      tipo: "analisar",
      bloqueio: "A issue está encerrada no rastreador; a análise de impacto parte de uma issue aberta.",
    };
  }
  if (i.situacao === "aprovada" && i.demanda) return { rotulo: "Abrir a demanda", tipo: "demanda" };
  if (i.analise) return { rotulo: "Abrir a análise", tipo: "analise" };
  return i.demanda
    ? { rotulo: "Abrir a demanda", tipo: "demanda" }
    : { rotulo: "Analisar impacto", tipo: "analisar" };
}

/** "A-0003 em revisão", "sem análise". */
export const situacaoEmTexto = (i: IssueDoProjeto) =>
  i.situacao === "sem-analise" || !i.analise
    ? ROTULO_SITUACAO[i.situacao]
    : `${i.analise} ${ROTULO_SITUACAO[i.situacao]}`;

/** Tom do selo da situação na base: em revisão espera o analista (atenção). */
export const TOM_SITUACAO: Record<SituacaoNaBase, TomSelo> = {
  "sem-analise": "atencao",
  "em-andamento": "andamento",
  "em-revisao": "atencao",
  aprovada: "concluido",
};
