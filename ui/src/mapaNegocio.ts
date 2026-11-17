import type { Contagem } from "./conhecimento";

/**
 * O mapa conta os símbolos do código por tipo técnico (`campo`, `metodo`, `construtor`).
 * Para o analista de negócio, o que importa é o que o código guarda e o que ele faz:
 * os tipos se agrupam em poucos termos (#227).
 */
const GRUPOS: { rotulo: string; chaves: string[] }[] = [
  { rotulo: "tabelas", chaves: ["tabela"] },
  { rotulo: "colunas", chaves: ["coluna"] },
  { rotulo: "consultas e visões", chaves: ["visao"] },
  { rotulo: "numeração e índices", chaves: ["sequencia", "indice", "sinonimo"] },
  { rotulo: "gatilhos automáticos", chaves: ["trigger"] },
  {
    rotulo: "operações (rotinas e funções)",
    chaves: [
      "metodo",
      "construtor",
      "funcao",
      "procedure",
      "rotina",
      "secao",
      "paragrafo",
    ],
  },
  { rotulo: "dados (atributos e valores fixos)", chaves: ["campo", "propriedade", "constante", "variavel"] },
  {
    rotulo: "conceitos do código (classes e módulos)",
    chaves: [
      "classe",
      "interface",
      "enum",
      "registro",
      "tipo",
      "namespace",
      "modulo",
      "pacote",
      "programa",
      "copybook",
    ],
  },
  { rotulo: "telas", chaves: ["tela"] },
  { rotulo: "componentes de tela", chaves: ["componente"] },
];

const OUTROS = "outros itens";

/** Agrupa as contagens de símbolos em termos de negócio, as maiores primeiro. */
export function simbolosEmNegocio(simbolos: Contagem[]): { rotulo: string; quantidade: number }[] {
  const soma = new Map<string, number>();
  for (const s of simbolos) {
    const grupo = GRUPOS.find((g) => g.chaves.includes(s.chave))?.rotulo ?? OUTROS;
    soma.set(grupo, (soma.get(grupo) ?? 0) + s.quantidade);
  }
  return [...soma.entries()]
    .map(([rotulo, quantidade]) => ({ rotulo, quantidade }))
    .sort((a, b) => b.quantidade - a.quantidade || a.rotulo.localeCompare(b.rotulo, "pt-BR"));
}
