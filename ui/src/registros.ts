import { invoke } from "@tauri-apps/api/core";

/**
 * Espelho dos comandos das premissas, decisões, riscos e restrições da demanda
 * (`ui/src-tauri/src/registros.rs`, campos em camelCase; #122).
 */
export type Categoria = "premissa" | "decisao" | "risco" | "restricao";
export type StatusPremissa = "nao-validada" | "validada" | "invalidada";
export type Nivel = "baixo" | "medio" | "alto";
export type TipoRestricao = "prazo" | "legal" | "tecnica";

export const CATEGORIAS: Categoria[] = ["premissa", "decisao", "risco", "restricao"];

export const TITULO_CATEGORIA: Record<Categoria, string> = {
  premissa: "Premissas",
  decisao: "Decisões",
  risco: "Riscos",
  restricao: "Restrições",
};

export const NOME_CATEGORIA: Record<Categoria, string> = {
  premissa: "premissa",
  decisao: "decisão",
  risco: "risco",
  restricao: "restrição",
};

export const ROTULO_STATUS_PREMISSA: Record<StatusPremissa, string> = {
  "nao-validada": "não validada",
  validada: "validada",
  invalidada: "invalidada",
};

export const ROTULO_NIVEL: Record<Nivel, string> = {
  baixo: "baixo",
  medio: "médio",
  alto: "alto",
};

export const ROTULO_RESTRICAO: Record<TipoRestricao, string> = {
  prazo: "prazo",
  legal: "legal",
  tecnica: "técnica",
};

/** Campos que o analista edita (`cba_kb::DadosRegistro`). */
export interface DadosRegistro {
  titulo: string;
  detalhe?: string | null;
  quem?: string | null;
  /** Slug da pessoa no catálogo. */
  pessoa?: string | null;
  status?: StatusPremissa | null;
  comoValidada?: string | null;
  /** `AAAA-MM-DD`. */
  decididoEm?: string | null;
  alternativas?: string[];
  probabilidade?: Nivel | null;
  impacto?: Nivel | null;
  resposta?: string | null;
  restricao?: TipoRestricao | null;
}

export interface Validacao {
  como: string;
  em: string;
  pergunta?: string | null;
}

export interface RegistroVista {
  /** `PR-0001`. */
  id: string;
  categoria: Categoria;
  titulo: string;
  detalhe: string | null;
  quem: string | null;
  pessoa: string | null;
  /** Ata ou análise de origem, relativa à raiz da base. */
  origem: string | null;
  /** `R-0001/D-1`, `A-0001/3`. */
  referencia: string | null;
  criado: string;
  status: StatusPremissa | null;
  validacao: Validacao | null;
  decididoEm: string | null;
  alternativas: string[];
  probabilidade: Nivel | null;
  impacto: Nivel | null;
  resposta: string | null;
  restricao: TipoRestricao | null;
  historico: { em: string; evento: string }[];
  /** O registro, relativo à raiz da base. */
  documento: string;
}

export type FonteProposta =
  | { tipo: "analise"; id: string; titulo: string }
  | { tipo: "ata"; id: string; data: string };

export interface Proposta {
  referencia: string;
  categoria: Categoria;
  fonte: FonteProposta;
  /** Valores sugeridos pelo agente. */
  dados: DadosRegistro;
}

export interface PessoaVista {
  slug: string;
  nome: string;
  papel: string | null;
}

export interface RegistrosDaDemanda {
  registros: RegistroVista[];
  propostas: Proposta[];
  pessoas: PessoaVista[];
  /** Pasta dos registros, relativa à raiz da base. */
  pasta: string;
}

export interface ValidacaoDePremissa {
  status: StatusPremissa;
  como?: string | null;
  pergunta?: string | null;
}

/** Operações da aba. A interface usa {@link apiRegistrosTauri}; os testes, uma falsa. */
export interface ApiRegistros {
  registros(projeto: string, demanda: string): Promise<RegistrosDaDemanda>;
  criar(
    projeto: string,
    demanda: string,
    categoria: Categoria,
    campos: DadosRegistro,
  ): Promise<RegistroVista>;
  editar(projeto: string, registro: string, campos: DadosRegistro): Promise<RegistroVista>;
  validarPremissa(
    projeto: string,
    registro: string,
    validacao: ValidacaoDePremissa,
  ): Promise<RegistroVista>;
  aceitarProposta(
    projeto: string,
    demanda: string,
    referencia: string,
    ajustes?: DadosRegistro,
  ): Promise<RegistroVista>;
  recusarProposta(projeto: string, demanda: string, referencia: string): Promise<void>;
}

export const apiRegistrosTauri: ApiRegistros = {
  registros: async (projeto, demanda) => invoke("registros_demanda_app", { projeto, demanda }),
  criar: async (projeto, demanda, categoria, campos) =>
    invoke("criar_registro_app", { projeto, demanda, categoria, campos }),
  editar: async (projeto, registro, campos) =>
    invoke("editar_registro_app", { projeto, registro, campos }),
  validarPremissa: async (projeto, registro, validacao) =>
    invoke("validar_premissa_app", { projeto, registro, validacao }),
  aceitarProposta: async (projeto, demanda, referencia, ajustes) =>
    invoke("aceitar_proposta_app", { projeto, demanda, referencia, ajustes: ajustes ?? null }),
  recusarProposta: async (projeto, demanda, referencia) =>
    invoke("recusar_proposta_app", { projeto, demanda, referencia }),
};

/** Origem legível de um registro ou proposta: `A-0001/3` → "análise A-0001". */
export function rotuloReferencia(referencia: string | null): string | null {
  if (!referencia) return null;
  const [doc, item] = referencia.split("/");
  if (doc.startsWith("A-")) return `análise ${doc}`;
  if (doc.startsWith("R-")) return item ? `ata ${doc}, item ${item}` : `ata ${doc}`;
  return referencia;
}

const espacos = (texto: string) => texto.split(/\s+/).filter(Boolean).join(" ");

/**
 * O texto inteiro de um registro ou proposta, para ler sem corte (#409). O agente
 * resume o enunciado numa linha (cortada com "…" quando passa do tamanho) e guarda o
 * texto original no detalhe: com o enunciado cortado, vale o detalhe; com um
 * enunciado inteiro, o detalhe vem num parágrafo depois dele.
 */
export function textoCompleto(titulo: string, detalhe?: string | null): string {
  const d = detalhe?.trim();
  if (!d || espacos(d) === espacos(titulo)) return titulo;
  if (titulo.trimEnd().endsWith("…")) {
    const prefixo = espacos(titulo.trimEnd().slice(0, -1));
    if (espacos(d).startsWith(prefixo)) return d;
  }
  return `${titulo}\n\n${d}`;
}

/** Quem originou as propostas, para o aviso: "A-0011", "ata R-0002". */
export function rotuloFonte(fonte: FonteProposta): string {
  return fonte.tipo === "analise" ? fonte.id : `ata ${fonte.id}`;
}

/** "a", "a e b", "a, b e c". */
export function listaPorExtenso(itens: string[]): string {
  if (itens.length <= 1) return itens.join("");
  return `${itens.slice(0, -1).join(", ")} e ${itens[itens.length - 1]}`;
}
