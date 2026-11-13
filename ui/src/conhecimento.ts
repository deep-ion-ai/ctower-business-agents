import { Channel, invoke } from "@tauri-apps/api/core";
import {
  apiAnaliseTauri,
  type AnaliseCriada,
  type AnaliseResumo,
  type ApiAmbiente,
  type Fluxo,
  type SessaoVista,
  type Situacao,
} from "./analise";

/**
 * Espelho dos comandos do início, do projeto e da base de conhecimento
 * (`ui/src-tauri/src/{inicio,projeto,base}.rs`, campos em camelCase).
 */
export type StatusRegra = "confirmada" | "a-revalidar" | "obsoleta";

export interface ProjetoInicio {
  id: string;
  nome: string;
  analises: number;
  regras: number;
  aRevalidar: number;
  termos: number;
  erro: string | null;
}

export interface AnaliseInicio extends AnaliseResumo {
  projeto: string;
  projetoNome: string;
  sessao: SessaoVista | null;
}

export interface Inicio {
  base: string | null;
  projetos: ProjetoInicio[];
  emAndamento: AnaliseInicio[];
  recentes: AnaliseInicio[];
}

export interface RegraVista {
  id: string;
  titulo: string;
  area: string;
  status: StatusRegra;
  /** Relativo à raiz da base. */
  caminho: string;
  /** A análise que originou a regra, relativa à raiz da base. */
  origem: string | null;
  evidencias: number;
}

export interface TermoVista {
  termo: string;
  sinonimos: string[];
  area: string | null;
  caminho: string;
  origem: string | null;
}

export interface AreaVista {
  id: string;
  nome: string;
  sigla: string | null;
  declarada: boolean;
  regras: number;
  aRevalidar: number;
  termos: number;
  memoria: string | null;
}

export interface ProblemaVista {
  caminho: string;
  erro: string;
}

export interface VisaoProjeto {
  id: string;
  nome: string;
  documento: string;
  /** As "a revalidar" primeiro. */
  regras: RegraVista[];
  termos: TermoVista[];
  areas: AreaVista[];
  problemas: ProblemaVista[];
}

export interface ResumoRevalidacao {
  evidencias: number;
  desatualizadas: number;
  regras: {
    id: string;
    titulo: string;
    antes: StatusRegra;
    depois: StatusRegra;
  }[];
  analises: string[];
  problemas: ProblemaVista[];
  mudou: boolean;
  duracaoMs: number;
}

export interface Contagem {
  chave: string;
  rotulo: string;
  quantidade: number;
}

export interface ModuloVista {
  /** `""` é a raiz do projeto. */
  caminho: string;
  arquivos: number;
  falhas: number;
  linguagens: Contagem[];
  simbolos: Contagem[];
  subpastas: { caminho: string; arquivos: number }[];
  arquivosDiretos: string[];
  dependencias: { de: string; para: string; referencias: number }[];
}

export interface MapaVista {
  /** Segundos desde 1970 (UTC). */
  geradoEm: number;
  modulo: ModuloVista;
}

/** Filtro da busca por grupo de tipo. */
export type FiltroTipo = "regra" | "termo" | "analise";

export interface Encontrado {
  caminho: string;
  projeto: string;
  tipo: string | null;
  identificador: string | null;
  titulo: string;
  area: string | null;
  status: string | null;
  /** Trecho com as palavras encontradas entre `**`. */
  trecho: string;
}

export interface Busca {
  resultados: Encontrado[];
  total: number;
  deslocamento: number;
  qualquerPalavra: boolean;
}

export interface LigacaoVista {
  caminho: string;
  titulo: string | null;
  /** `origem` ou `analise`. */
  campo: string;
  /** Este documento cita o outro (a regra cita a análise de origem). */
  sai: boolean;
  existe: boolean;
}

export interface DocumentoAberto {
  caminho: string;
  projeto: string | null;
  tipo: string | null;
  titulo: string;
  status: string | null;
  campos: { chave: string; valor: string }[];
  /** Corpo em Markdown, sem o frontmatter. */
  markdown: string;
  ligacoes: LigacaoVista[];
  /** A análise que o documento representa, para abrir na tela da análise. */
  analise: { projeto: string; id: string } | null;
  problema: string | null;
}

/** Operações das telas do início, do projeto e da base. A interface usa {@link apiConhecimentoTauri}; os testes, uma falsa. */
export interface ApiConhecimento extends ApiAmbiente {
  situacao(): Promise<Situacao>;
  inicio(): Promise<Inicio>;
  criarAnalise(
    projeto: string,
    fluxo: Fluxo,
    entrada: string,
    titulo?: string,
  ): Promise<AnaliseCriada>;
  /**
   * Cria a análise a partir de uma issue do rastreador do projeto (`PROJ-123`, `#12`):
   * título e descrição viram a entrada e o link vai em `demanda.origem`.
   */
  criarAnaliseDeDemanda(projeto: string, fluxo: Fluxo, referencia: string): Promise<AnaliseCriada>;
  visaoProjeto(projeto: string): Promise<VisaoProjeto>;
  revalidar(projeto: string): Promise<ResumoRevalidacao>;
  /** `null` se o projeto ainda não foi mapeado. */
  mapa(projeto: string, modulo?: string): Promise<MapaVista | null>;
  mapear(projeto: string): Promise<MapaVista>;
  buscar(
    texto: string,
    filtros?: { projeto?: string; tipos?: FiltroTipo[]; deslocamento?: number },
  ): Promise<Busca>;
  abrirDocumento(caminho: string): Promise<DocumentoAberto>;
  /** Avisa quando a base muda fora do app; devolve a função que para. */
  vigiarBase(aoMudar: () => void): Promise<() => void>;
}

export const apiConhecimentoTauri: ApiConhecimento = {
  situacao: () => invoke("situacao_app"),
  inicio: () => invoke("inicio_app"),
  criarAnalise: (projeto, fluxo, entrada, titulo) =>
    invoke("criar_analise_fluxo", {
      projeto,
      fluxo,
      entrada,
      titulo: titulo ?? null,
    }),
  criarAnaliseDeDemanda: (projeto, fluxo, referencia) =>
    invoke("criar_analise_de_demanda", { projeto, fluxo, referencia }),
  visaoProjeto: (projeto) => invoke("visao_projeto_app", { projeto }),
  revalidar: (projeto) => invoke("revalidar_projeto_app", { projeto }),
  mapa: (projeto, modulo) => invoke("mapa_projeto_app", { projeto, modulo: modulo ?? null }),
  mapear: (projeto) => invoke("mapear_projeto_app", { projeto }),
  buscar: (texto, filtros = {}) =>
    invoke("buscar_base", {
      texto,
      projeto: filtros.projeto ?? null,
      tipos: filtros.tipos ?? null,
      deslocamento: filtros.deslocamento ?? null,
    }),
  abrirDocumento: (caminho) => invoke("abrir_documento_base", { caminho }),
  async vigiarBase(aoMudar) {
    const canal = new Channel<unknown>();
    canal.onmessage = () => aoMudar();
    const id = await invoke<number>("vigiar_base", { canal });
    return () => {
      canal.onmessage = () => undefined;
      invoke("parar_vigia_base", { id }).catch(() => undefined);
    };
  },
  verificarAmbiente: apiAnaliseTauri.verificarAmbiente,
  planejarPreparo: apiAnaliseTauri.planejarPreparo,
  prepararAmbiente: apiAnaliseTauri.prepararAmbiente,
};

export const ROTULO_FLUXO: Record<Fluxo, string> = {
  perguntar: "Perguntar ao sistema",
  explicar: "Explicar funcionalidade",
  impacto: "Analisar impacto",
};

export const ROTULO_STATUS_REGRA: Record<StatusRegra, string> = {
  confirmada: "confirmada",
  "a-revalidar": "a revalidar",
  obsoleta: "obsoleta",
};

const ROTULO_TIPO: Record<string, string> = {
  regra: "Regra",
  termo: "Termo",
  pergunta: "Pergunta",
  "analise-impacto": "Análise de impacto",
  funcionalidade: "Funcionalidade",
  "caso-de-uso": "Caso de uso",
  sugestao: "Sugestão",
  "memoria-area": "Memória da área",
  projeto: "Projeto",
  "regras-do-projeto": "Regras do projeto",
};

export function rotuloTipo(tipo: string | null): string {
  return (tipo && ROTULO_TIPO[tipo]) ?? "Documento";
}

/**
 * Resolve o destino de um link de um documento da base: relativo ao documento
 * (`../analises/A-0001.md`) ou à raiz da base. Devolve o caminho relativo à base,
 * ou `null` se o link não for para um `.md` da base (externo, âncora, fora da base).
 */
export function resolverLink(documento: string, href: string): string | null {
  let alvo: string;
  try {
    alvo = decodeURI(href.split("#")[0].split("?")[0]).replace(/\\/g, "/");
  } catch {
    return null;
  }
  if (!alvo || /^[a-z][a-z0-9+.-]*:/i.test(alvo) || alvo.startsWith("//")) return null;
  if (!alvo.toLowerCase().endsWith(".md")) return null;
  const partes = alvo.startsWith("/") ? [] : documento.split("/").slice(0, -1);
  for (const p of alvo.split("/")) {
    if (p === "" || p === ".") continue;
    if (p === "..") {
      if (partes.length === 0) return null;
      partes.pop();
    } else {
      partes.push(p);
    }
  }
  return partes.length > 0 ? partes.join("/") : null;
}

/** Trecho da busca em partes, com as palavras encontradas (entre `**`) marcadas. */
export function partesDoTrecho(trecho: string): { texto: string; destaque: boolean }[] {
  return trecho
    .split("**")
    .map((texto, i) => ({ texto, destaque: i % 2 === 1 }))
    .filter((p) => p.texto !== "");
}
