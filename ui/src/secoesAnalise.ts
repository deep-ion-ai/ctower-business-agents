import type { SecaoDocumento } from "./analise";

/**
 * Títulos conhecidos, para quando a vista não traz as seções do documento (respostas
 * antigas, testes): os de `cba_docs::MODELOS`.
 */
const TITULOS_CONHECIDOS: Record<string, string> = {
  resposta: "Resposta",
  regras_encontradas: "Regras encontradas",
  exemplo: "Exemplo",
  contexto_no_codigo: "O que a base e o código dizem",
  fluxo_processo: "Fluxo do processo",
  modelo_dados: "Modelo de dados conceitual",
};

/** Ordem do template "pergunta ao sistema", quando a vista não traz as seções. */
const ORDEM_CONHECIDA = ["resposta", "regras_encontradas", "exemplo", "contexto_no_codigo"];

/**
 * Título da seção como no documento (`atores_e_telas` → "Atores e telas"): o das
 * seções do fluxo ou, sem ele, o marcador legível. A tela nunca mostra o marcador
 * cru (#222).
 */
export function tituloDaSecao(marcador: string, secoes?: SecaoDocumento[]): string {
  const doDocumento = secoes?.find((s) => s.marcador === marcador)?.titulo;
  if (doDocumento) return doDocumento;
  const conhecido = TITULOS_CONHECIDOS[marcador];
  if (conhecido) return conhecido;
  const texto = marcador.replace(/[_-]+/g, " ").trim();
  return texto ? texto.charAt(0).toUpperCase() + texto.slice(1) : marcador;
}

/**
 * Ordena os marcadores na ordem do template do documento; os de fora dele vêm
 * depois, na ordem em que apareceram.
 */
export function ordenarSecoes<T>(
  grupos: [string, T][],
  secoes?: SecaoDocumento[],
): [string, T][] {
  const ordem = secoes?.length ? secoes.map((s) => s.marcador) : ORDEM_CONHECIDA;
  const posicao = (m: string) => {
    const i = ordem.indexOf(m);
    return i < 0 ? ordem.length : i;
  };
  // `sort` é estável: os de fora do template mantêm a ordem de chegada.
  return [...grupos].sort(([a], [b]) => posicao(a) - posicao(b));
}
