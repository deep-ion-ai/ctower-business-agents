import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { Fluxo } from "./analise";
import type { CicloDeVida, Destino, Geracao } from "./entregaveis";

/**
 * Espelho dos comandos da seção "Entregáveis" do painel da demanda
 * (`ui/src-tauri/src/entregaveis_demanda.rs`, #463): a análise validada de base, os
 * tipos habilitados no projeto, os entregáveis gerados (rascunhos, revisão final,
 * envios) e as ações.
 */

export type Situacao = "gerando" | "rascunho" | "revisado" | "enviado";
export type Finalidade = "demanda" | "versao_inicial";
export type Marca = "mantido" | "novo" | "alterado" | "removido";

export interface RegraDaBase {
  id: string;
  titulo: string;
}

export interface DecisaoRespondida {
  numero: number;
  texto: string;
  resposta: string;
}

export interface PerguntaAberta {
  numero: number;
  texto: string;
  decisao: boolean;
}

/** A análise validada (aprovada) de base. */
export interface BaseVista {
  analise: string;
  titulo: string;
  fluxo: Fluxo;
  regras: RegraDaBase[];
  decisoes: DecisaoRespondida[];
  perguntas: PerguntaAberta[];
  funcionalidades: string[];
}

export interface TipoNaDemanda {
  id: string;
  nome: string;
  descricao: string;
  ciclo: CicloDeVida;
  geracao: Geracao;
  destino: Destino;
  padrao: boolean;
  /** Por que não dá para gerar na demanda. */
  indisponivel?: string | null;
}

export interface EvidenciaVista {
  arquivo: string;
  inicio: number;
  fim: number;
  trecho?: string | null;
}

export interface RegraVista {
  id: string;
  titulo: string;
  evidencias: EvidenciaVista[];
}

export interface ItemVisto {
  numero: number;
  texto: string;
  regras: RegraVista[];
}

export interface TrechoVisto {
  marca: Marca;
  texto: string;
  anterior?: string | null;
}

export interface RevisaoVista {
  versao: string;
  data: string;
  demanda?: string | null;
  revisor: string;
  mudancas: string;
}

export interface PropostaVista {
  caminho: string;
  versao: string;
  base?: string | null;
  mudancas: string;
  trechos: TrechoVisto[];
  historico: RevisaoVista[];
}

export interface EnvioVisto {
  destino: Destino;
  onde: string;
  versao: number;
  por: string;
  emMs: number;
}

export interface EntregavelVisto {
  numero: number;
  tipo: string;
  nome: string;
  ciclo: CicloDeVida;
  analise: string;
  finalidade: Finalidade;
  funcionalidade?: string | null;
  destino: Destino;
  situacao: Situacao;
  versao: number;
  versoes: number[];
  texto: string;
  editado: boolean;
  itens: ItemVisto[];
  semCobertura: string[];
  avisoCobertura?: string | null;
  conferir: string[];
  revisadoPor?: string | null;
  envios: EnvioVisto[];
  proposta?: PropostaVista | null;
  prompt: string;
}

export interface DocumentoVivoVisto {
  tipo: string;
  caminho?: string | null;
  versao?: string | null;
  /** "em dia", "proposta pendente", "desatualizado" ou "sem documento". */
  estado: string;
}

export interface FuncionalidadeVista {
  id: string;
  nome: string;
  daAnalise: boolean;
  documentos: DocumentoVivoVisto[];
}

export interface VistaEntregaveis {
  demanda: string;
  aberta: boolean;
  analises: BaseVista[];
  tipos: TipoNaDemanda[];
  entregaveis: EntregavelVisto[];
  funcionalidades: FuncionalidadeVista[];
}

export interface PedidoDeGeracao {
  tipo: string;
  analise: string;
  funcionalidade?: string | null;
  versaoInicial: boolean;
}

export interface GeracaoAberta {
  numero: number;
  versao: number;
  prompt: string;
}

export interface ComentarioPreparado {
  texto: string;
  link?: string | null;
  chave?: string | null;
}

export interface ApiEntregaveisDemanda {
  vista(projeto: string, demanda: string): Promise<VistaEntregaveis>;
  gerar(projeto: string, demanda: string, pedido: PedidoDeGeracao): Promise<GeracaoAberta>;
  editar(projeto: string, demanda: string, numero: number, texto: string): Promise<void>;
  marcarRevisado(projeto: string, demanda: string, numero: number): Promise<void>;
  escolherDestino(nome: string): Promise<string | null>;
  exportar(projeto: string, demanda: string, numero: number, destino: string): Promise<string>;
  publicar(projeto: string, demanda: string, numero: number): Promise<string>;
  comentario(projeto: string, demanda: string, numero: number): Promise<ComentarioPreparado>;
  copiar(texto: string): Promise<void>;
}

export const apiEntregaveisDemandaTauri: ApiEntregaveisDemanda = {
  vista: async (projeto, demanda) => invoke("entregaveis_demanda_app", { projeto, demanda }),
  gerar: async (projeto, demanda, p) =>
    invoke("gerar_entregavel_app", {
      projeto,
      demanda,
      tipo: p.tipo,
      analise: p.analise,
      funcionalidade: p.funcionalidade ?? null,
      versaoInicial: p.versaoInicial,
    }),
  editar: async (projeto, demanda, numero, texto) =>
    invoke("editar_entregavel_app", { projeto, demanda, numero, texto }),
  marcarRevisado: async (projeto, demanda, numero) =>
    invoke("marcar_entregavel_revisado_app", { projeto, demanda, numero }),
  escolherDestino: async (nome) =>
    save({
      title: "Exportar o entregável",
      defaultPath: `${nome}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    }),
  exportar: async (projeto, demanda, numero, destino) =>
    invoke("exportar_entregavel_app", { projeto, demanda, numero, destino }),
  publicar: async (projeto, demanda, numero) =>
    invoke("publicar_entregavel_app", { projeto, demanda, numero }),
  comentario: async (projeto, demanda, numero) =>
    invoke("comentario_entregavel_app", { projeto, demanda, numero }),
  copiar: async (texto) => navigator.clipboard.writeText(texto),
};

export const ROTULO_SITUACAO: Record<Situacao, string> = {
  gerando: "aguardando o agente",
  rascunho: "rascunho",
  revisado: "revisado",
  enviado: "enviado",
};

/** O botão do destino depois da revisão; o tipo vivo sempre publica a versão. */
export function acaoDoDestino(e: Pick<EntregavelVisto, "ciclo" | "destino" | "proposta">): {
  texto: string;
  tipo: "exportar" | "publicar" | "comentario";
} {
  if (e.ciclo === "vivo") {
    return {
      texto: e.proposta ? `Publicar a versão ${e.proposta.versao}` : "Publicar a versão",
      tipo: "publicar",
    };
  }
  switch (e.destino) {
    case "repositorio_docs":
      return { texto: "Publicar", tipo: "publicar" };
    case "comentario_rastreador":
      return { texto: "Enviar ao rastreador", tipo: "comentario" };
    default:
      return { texto: "Exportar", tipo: "exportar" };
  }
}

/**
 * Pode enviar ou publicar: só depois da revisão final. No vivo, só com a
 * proposta pendente: publicada a versão, não há mais o que publicar.
 */
export const podeEnviar = (e: Pick<EntregavelVisto, "situacao" | "ciclo" | "proposta">) =>
  (e.situacao === "revisado" || e.situacao === "enviado") && (e.ciclo !== "vivo" || !!e.proposta);
