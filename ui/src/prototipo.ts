import { invoke } from "@tauri-apps/api/core";
import type { AnaliseCriada } from "./analise";

/** Fidelidade pedida para o protótipo (`cba_core::prototipo::Fidelidade`). */
export type Fidelidade = "wireframe" | "alta_fidelidade";

export const ROTULO_FIDELIDADE: Record<Fidelidade, string> = {
  wireframe: "Wireframe",
  alta_fidelidade: "Alta fidelidade",
};

/**
 * Entrada opcional do analista para o protótipo (`cba_core::prototipo::PedidoPrototipo`):
 * as telas (vazio deixa o agente listar as afetadas), a fidelidade e o que fica de fora.
 */
export interface PedidoPrototipo {
  telas: string[];
  fidelidade: Fidelidade;
  fora?: string;
}

/** Pedido de criação da análise de protótipo (#379). */
export interface NovoPrototipo {
  /** Análise de origem (`A-0001`): impacto, explicação, pergunta ou histórias. */
  origem: string;
  /** Demanda (`D-0001`); sem ela, a análise herda a da origem. */
  demanda?: string;
  pedido: PedidoPrototipo;
  entrada?: string;
  titulo?: string;
}

/** Fluxos cujas análises podem ser a origem do protótipo (#381). */
export const ORIGENS_PROTOTIPO = new Set(["perguntar", "explicar", "impacto", "historias"]);

/** Categoria de um token de estilo (`cba_index::CategoriaToken`). */
export type CategoriaToken = "cor" | "tipografia" | "espacamento" | "raio";

export const ROTULO_CATEGORIA_TOKEN: Record<CategoriaToken, string> = {
  cor: "Cores",
  tipografia: "Tipografia",
  espacamento: "Espaçamento",
  raio: "Raio",
};

/** Onde o token está no código do projeto. */
export interface OrigemToken {
  arquivo: string;
  linha: number;
}

/** Token achado no código (`cba_index::TokenEstilo`). */
export interface TokenEstilo {
  nome: string;
  original: string;
  categoria: CategoriaToken;
  valor?: string;
  origem: OrigemToken;
  nota?: string;
}

/** De onde vem o valor de um papel (`cba_core::tokens::SituacaoPapel`). */
export type SituacaoPapel = "encontrado" | "a-confirmar" | "informado";

/** Papel do protótipo (`--prototipo-<nome>`) com o valor da revisão. */
export interface PapelToken {
  nome: string;
  categoria: CategoriaToken;
  descricao: string;
  situacao: SituacaoPapel;
  valor?: string;
  token?: string;
  origem?: OrigemToken;
  nota?: string;
}

/** Tokens CSS do projeto (#384), como a tela mostra (`TokensVista`). */
export interface TokensVista {
  /** `rascunho` aguarda a revisão; só os `aprovado` vão para as telas. */
  estado: "rascunho" | "aprovado";
  tokens: TokenEstilo[];
  papeis: PapelToken[];
  css: string;
  aConfirmar: number;
  extraidoMs: number;
  atualizadoMs: number;
  aprovadoMs?: number | null;
  arquivo: string;
}

/** Comandos do protótipo de telas: a criação da análise e os tokens do projeto. */
export interface ApiPrototipo {
  criarAnaliseDePrototipo(projeto: string, novo: NovoPrototipo): Promise<AnaliseCriada>;
  /** `null` antes da primeira extração. */
  tokens(projeto: string): Promise<TokensVista | null>;
  /** Extrai do código (de novo): volta ao rascunho e mantém os valores informados. */
  extrairTokens(projeto: string): Promise<TokensVista>;
  /** Com valor, o analista informa o papel; sem valor, volta ao do código. */
  informarToken(projeto: string, papel: string, valor?: string): Promise<TokensVista>;
  aprovarTokens(projeto: string): Promise<TokensVista>;
}

export const apiPrototipoTauri: ApiPrototipo = {
  criarAnaliseDePrototipo: (projeto, novo) =>
    invoke("criar_analise_de_prototipo_app", {
      projeto,
      origem: novo.origem,
      demanda: novo.demanda ?? null,
      pedido: {
        telas: novo.pedido.telas,
        fidelidade: novo.pedido.fidelidade,
        fora: novo.pedido.fora ?? null,
      },
      entrada: novo.entrada ?? null,
      titulo: novo.titulo ?? null,
    }),
  tokens: (projeto) => invoke("tokens_css_projeto", { projeto }),
  extrairTokens: (projeto) => invoke("extrair_tokens_css_projeto", { projeto }),
  informarToken: (projeto, papel, valor) =>
    invoke("informar_token_css_projeto", { projeto, papel, valor: valor ?? null }),
  aprovarTokens: (projeto) => invoke("aprovar_tokens_css_projeto", { projeto }),
};

/** As telas do campo de texto, uma por linha (ou separadas por ponto e vírgula). */
export function telasDoTexto(texto: string): string[] {
  const telas: string[] = [];
  for (const t of texto.split(/[\n;]/)) {
    const nome = t.trim();
    if (nome && !telas.some((x) => x.toLowerCase() === nome.toLowerCase())) telas.push(nome);
  }
  return telas;
}
