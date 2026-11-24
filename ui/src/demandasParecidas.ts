/** Texto sem acento, em minúsculas. */
export function dobrar(texto: string): string {
  return texto.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

const SEM_VALOR = new Set([
  "para",
  "como",
  "pela",
  "pelo",
  "pelas",
  "pelos",
  "sobre",
  "quando",
  "entre",
  "depois",
  "antes",
  "esta",
  "este",
  "essa",
  "esse",
  "mais",
  "menos",
  "deve",
  "ser",
  "fazer",
]);

/** Palavras do título que ajudam a achar o parecido (4 letras ou mais, fora as comuns). */
function palavras(titulo: string): Set<string> {
  return new Set(
    dobrar(titulo)
      .split(/[^\p{L}\p{N}]+/u)
      .filter((p) => p.length >= 4 && !SEM_VALOR.has(p)),
  );
}

export interface Candidato {
  /** Quem identifica o candidato na tela (a chave ou o número da demanda). */
  id: string;
  titulo: string;
}

/**
 * Itens de título parecido com o do alvo, para o analista não analisar duas vezes a
 * mesma coisa: parecido é dividir uma palavra longa (7 letras ou mais) ou duas
 * palavras. Os mais parecidos primeiro; sem o próprio alvo.
 */
export function parecidos<T extends Candidato>(alvo: Candidato, candidatos: T[], limite = 3): T[] {
  const doAlvo = palavras(alvo.titulo);
  return candidatos
    .filter((c) => c.id !== alvo.id)
    .map((c) => {
      const comuns = [...palavras(c.titulo)].filter((p) => doAlvo.has(p));
      const forte = comuns.some((p) => p.length >= 7);
      return { c, nota: comuns.length, ok: forte || comuns.length >= 2 };
    })
    .filter((x) => x.ok)
    .sort((a, b) => b.nota - a.nota)
    .slice(0, limite)
    .map((x) => x.c);
}
