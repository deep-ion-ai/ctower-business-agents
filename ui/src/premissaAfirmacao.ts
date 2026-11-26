/** Veredito da verificação de uma premissa do solicitante. */
export type Veredito = "nao-confere" | "confere-em-parte" | "confere";

/** Uma afirmação no formato "Premissa do solicitante: …. Verificação: …", separada. */
export interface PremissaVerificada {
  /** O que o solicitante afirmou, sem o rótulo. */
  premissa: string;
  /** Sem "Verificação:" no texto, `null`: só a premissa. */
  verificacao: {
    /** O veredito reconhecido; um veredito fora dos três fica `null` (sem selo). */
    veredito: Veredito | null;
    /** O que o código mostra, sem o veredito (Markdown, com os parágrafos). */
    texto: string;
  } | null;
}

/** Rótulo do selo de cada veredito. */
export const ROTULO_VEREDITO: Record<Veredito, string> = {
  "nao-confere": "não confere",
  "confere-em-parte": "confere em parte",
  confere: "confere",
};

const VERIFICACAO = /verificação:/i;

// A mesma ordem do Rust: "não confere" antes de "confere em parte" antes de "confere".
// O veredito termina numa fronteira de palavra ("conferem" não é veredito).
const VEREDITO = /^(n[ãa]o confere|confere em parte|confere)(?![\p{L}\p{N}])/iu;

/**
 * Separa a premissa e a verificação de uma afirmação de "Demanda como entendida"
 * (#404), pela mesma regra do Rust (`premissa_da_afirmacao` em
 * `crates/cba-tools/src/registros.rs`): o texto começa com "premissa"; a premissa vai
 * do primeiro ":" até "Verificação:"; o veredito abre a verificação ("não confere",
 * "confere em parte" ou "confere"). Texto fora do formato dá `null` e é mostrado
 * como sempre.
 *
 * Diferente do Rust, que só quer o título do registro, aqui a premissa e o texto da
 * verificação ficam inteiros (com as quebras de linha), e o que vem depois do
 * veredito é o que o código mostra.
 */
export function premissaDaAfirmacao(texto: string): PremissaVerificada | null {
  const limpo = texto.trim();
  if (!limpo.toLowerCase().startsWith("premissa")) return null;
  const doisPontos = limpo.indexOf(":");
  if (doisPontos < 0) return null;
  const inicio = doisPontos + 1;
  const achado = VERIFICACAO.exec(limpo);
  const temVerificacao = !!achado && achado.index >= inicio;
  const premissa = (temVerificacao ? limpo.slice(inicio, achado!.index) : limpo.slice(inicio)).trim();
  if (!premissa) return null;
  if (!temVerificacao) return { premissa, verificacao: null };

  const resto = limpo.slice(achado!.index + achado![0].length).trimStart();
  const v = VEREDITO.exec(resto);
  if (!v) return { premissa, verificacao: { veredito: null, texto: resto.trim() } };
  const palavras = v[1].toLowerCase();
  const veredito: Veredito = palavras.endsWith("em parte")
    ? "confere-em-parte"
    : palavras.startsWith("n")
      ? "nao-confere"
      : "confere";
  // Depois do veredito: a pontuação que o fecha (". ", ": ", " — ") sai do texto.
  const depois = resto.slice(v[0].length).replace(/^\s*[.:;,—–-]?\s*/u, "");
  return { premissa, verificacao: { veredito, texto: depois.trim() } };
}
