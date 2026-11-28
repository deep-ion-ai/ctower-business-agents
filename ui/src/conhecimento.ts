import { Channel, invoke } from "@tauri-apps/api/core";
import { apiCoberturaTauri, type ApiCobertura } from "./cobertura";
import { apiConducaoTauri, type ApiConducao } from "./conducao";
import { apiMudancasTauri, type ApiMudancas } from "./mudancas";
import {
  apiAnaliseTauri,
  type AnaliseCriada,
  type AnaliseResumo,
  type ApiAmbiente,
  type Fluxo,
  type SessaoVista,
  type Situacao,
} from "./analise";
import type { DemandaVista, NovaDemanda, PainelDemanda } from "./demandas";
import { apiResumoTauri, type ResumoProjeto } from "./resumoProjeto";
import { apiIssuesTauri, type ApiIssues } from "./issuesProjeto";
import { apiPrototipoTauri, type ApiPrototipo } from "./prototipo";

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
  /** Histórias com critério de aceite a revalidar (#169). */
  historiasAfetadas?: number;
  /** Cenários de teste a revalidar (#169). */
  cenariosAfetados?: number;
  /** Tabelas de decisão a revalidar (#169). */
  tabelasAfetadas?: number;
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
export type FiltroTipo = "regra" | "termo" | "analise" | "externo";

/** De onde veio um documento externo (a cópia do repositório de docs, #277). */
export interface OrigemExterna {
  repositorio: string;
  /** Caminho no repositório. */
  caminho: string;
  commit: string;
  /** `publicada` (documento aprovado publicado pelo app) ou `externa`. */
  confianca: string;
}

/** Rótulo curto da origem: `GitHub dono/docs · abc1234`. */
export function rotuloOrigem(o: OrigemExterna): string {
  return `${o.repositorio} · ${o.commit.slice(0, 7)}`;
}

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
  /** Documentação externa: de onde veio (ausente nos documentos da base). */
  externo?: OrigemExterna | null;
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

/** Início do erro de `salvarDocumento` quando o arquivo mudou fora do app. */
export const CONFLITO_DOCUMENTO = "conflito:";

export interface DocumentoAberto {
  caminho: string;
  projeto: string | null;
  tipo: string | null;
  titulo: string;
  status: string | null;
  campos: { chave: string; valor: string }[];
  /** Corpo em Markdown, sem o frontmatter. */
  markdown: string;
  /** Marca da versão do arquivo, para {@link ApiConhecimento.salvarDocumento}. */
  versao: string;
  ligacoes: LigacaoVista[];
  /** A análise que o documento representa, para abrir na tela da análise. */
  analise: { projeto: string; id: string } | null;
  problema: string | null;
  /** Documentação externa (`externos/`): não se edita no app. */
  somenteLeitura?: boolean;
  externo?: OrigemExterna | null;
}

/** Pedido de uma análise de histórias: a origem, a demanda, ou as duas. */
export interface PedidoHistorias {
  origem?: string;
  demanda?: string;
  /** O que o analista quer nas histórias (opcional). */
  entrada?: string;
  titulo?: string;
}

/** Operações das telas do início, do projeto e da base. A interface usa {@link apiConhecimentoTauri}; os testes, uma falsa. */
export interface ApiConhecimento extends ApiAmbiente {
  situacao(): Promise<Situacao>;
  inicio(): Promise<Inicio>;
  /** Com `demanda` (`D-0001`), a análise fica ligada a ela (`demanda.id`). */
  criarAnalise(
    projeto: string,
    fluxo: Fluxo,
    entrada: string,
    titulo?: string,
    demanda?: string,
  ): Promise<AnaliseCriada>;
  /**
   * Cria a análise a partir de uma issue do rastreador do projeto (`PROJ-123`, `#12`):
   * título e descrição viram a entrada e o link vai em `demanda.origem`. Com
   * `pedido.entrada` (a pergunta, #359), ela é a entrada e a issue só liga o resultado.
   */
  criarAnaliseDeDemanda(
    projeto: string,
    fluxo: Fluxo,
    referencia: string,
    pedido?: { entrada?: string; titulo?: string },
  ): Promise<AnaliseCriada>;
  /** Análises do projeto, da mais recente para a mais antiga (a origem das histórias). */
  listarAnalises(projeto: string): Promise<AnaliseResumo[]>;
  /**
   * Cria a análise de histórias (#115) a partir da análise de origem (`A-0001`) ou de
   * uma demanda (`D-0001`); sem demanda, ela herda a da origem.
   */
  criarAnaliseDeHistorias(projeto: string, pedido: PedidoHistorias): Promise<AnaliseCriada>;
  /**
   * Cria a análise de cenários de teste (#117) a partir da análise de origem (do código
   * ou de histórias), de uma demanda ou só do pedido com as regras.
   */
  criarAnaliseDeCenarios(projeto: string, pedido: PedidoHistorias): Promise<AnaliseCriada>;
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
  /**
   * Grava o corpo Markdown do documento (o frontmatter fica como está) e atualiza o
   * índice. Recusa com erro iniciado por {@link CONFLITO_DOCUMENTO} se o arquivo mudou
   * desde a abertura (`versao`). Devolve o documento relido.
   */
  salvarDocumento(caminho: string, markdown: string, versao: string): Promise<DocumentoAberto>;
  /** Avisa quando a base muda fora do app; devolve a função que para. */
  vigiarBase(aoMudar: () => void): Promise<() => void>;
  /** Demandas do projeto, as abertas primeiro. */
  listarDemandas(projeto: string): Promise<DemandaVista[]>;
  /** Confere de novo o status de todas as demandas com o rastreador, sem cache (#292). */
  atualizarStatusDemandas(projeto: string): Promise<DemandaVista[]>;
  criarDemanda(projeto: string, nova: NovaDemanda): Promise<DemandaVista>;
  encerrarDemanda(projeto: string, demanda: string): Promise<DemandaVista>;
  reabrirDemanda(projeto: string, demanda: string): Promise<DemandaVista>;
  painelDemanda(projeto: string, demanda: string): Promise<PainelDemanda>;
  /**
   * Resumo do projeto (#318): contagens e a lista de atividades do analista, para as
   * Atividades e o selo do menu (#321).
   */
  resumoProjeto(projeto: string): Promise<ResumoProjeto>;
  /** Cobertura da base por área e pasta (#125); sem ela, a tela não mostra a seção. */
  cobertura?: ApiCobertura["cobertura"];
  /** O que mudou no código (#124), na tela do projeto. */
  mudancas?: ApiMudancas;
  /** Issues do rastreador (#322, #323); sem ela, a área Demandas mostra só as demandas da base. */
  issues?: ApiIssues;
  /** Condução pelo workflow em Nova análise (#371); sem ela, só a condução simples. */
  conducao?: ApiConducao;
  /** Protótipo de telas e tokens CSS do projeto (#379); sem ela, os comandos do app. */
  prototipo?: ApiPrototipo;
  /**
   * Fluxos que o projeto não oferece (#465): Histórias e Cenários sem os tipos deles
   * habilitados nos entregáveis. Sem ela, a tela mostra todos.
   */
  fluxosOcultos?(projeto: string): Promise<Fluxo[]>;
}

export const apiConhecimentoTauri: ApiConhecimento = {
  issues: apiIssuesTauri,
  conducao: apiConducaoTauri,
  prototipo: apiPrototipoTauri,
  cobertura: apiCoberturaTauri.cobertura,
  mudancas: apiMudancasTauri,
  situacao: () => invoke("situacao_app"),
  inicio: () => invoke("inicio_app"),
  fluxosOcultos: (projeto) => invoke("fluxos_ocultos_app", { projeto }),
  criarAnalise: (projeto, fluxo, entrada, titulo, demanda) =>
    invoke("criar_analise_fluxo", {
      projeto,
      fluxo,
      entrada,
      titulo: titulo ?? null,
      demanda: demanda ?? null,
    }),
  criarAnaliseDeDemanda: (projeto, fluxo, referencia, pedido) =>
    invoke("criar_analise_de_demanda", {
      projeto,
      fluxo,
      referencia,
      entrada: pedido?.entrada ?? null,
      titulo: pedido?.titulo ?? null,
    }),
  listarAnalises: apiAnaliseTauri.listarAnalises,
  criarAnaliseDeHistorias: (projeto, pedido) =>
    invoke("criar_analise_de_historias_app", {
      projeto,
      origem: pedido.origem ?? null,
      demanda: pedido.demanda ?? null,
      entrada: pedido.entrada ?? null,
      titulo: pedido.titulo ?? null,
    }),
  criarAnaliseDeCenarios: (projeto, pedido) =>
    invoke("criar_analise_de_cenarios_app", {
      projeto,
      origem: pedido.origem ?? null,
      demanda: pedido.demanda ?? null,
      entrada: pedido.entrada ?? null,
      titulo: pedido.titulo ?? null,
    }),
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
  salvarDocumento: (caminho, markdown, versao) =>
    invoke("salvar_documento_base", { caminho, markdown, versao }),
  async vigiarBase(aoMudar) {
    const canal = new Channel<unknown>();
    canal.onmessage = () => aoMudar();
    const id = await invoke<number>("vigiar_base", { canal });
    return () => {
      canal.onmessage = () => undefined;
      invoke("parar_vigia_base", { id }).catch(() => undefined);
    };
  },
  listarDemandas: (projeto) => invoke("listar_demandas_app", { projeto }),
  atualizarStatusDemandas: (projeto) => invoke("atualizar_status_demandas_app", { projeto }),
  criarDemanda: (projeto, nova) =>
    invoke("criar_demanda_app", {
      projeto,
      titulo: nova.titulo,
      descricao: nova.descricao ?? null,
      chave: nova.chave ?? null,
      tipo: nova.tipo ?? null,
    }),
  encerrarDemanda: (projeto, demanda) => invoke("encerrar_demanda_app", { projeto, demanda }),
  reabrirDemanda: (projeto, demanda) => invoke("reabrir_demanda_app", { projeto, demanda }),
  painelDemanda: (projeto, demanda) => invoke("painel_demanda_app", { projeto, demanda }),
  verificarAmbiente: apiAnaliseTauri.verificarAmbiente,
  resumoProjeto: apiResumoTauri.resumoProjeto,
  planejarPreparo: apiAnaliseTauri.planejarPreparo,
  prepararAmbiente: apiAnaliseTauri.prepararAmbiente,
};

export const ROTULO_FLUXO: Record<Fluxo, string> = {
  perguntar: "Perguntar ao sistema",
  explicar: "Explicar funcionalidade",
  impacto: "Analisar impacto",
  reuniao: "Analisar reunião",
  revisar: "Revisar requisito",
  historias: "Histórias e critérios",
  cenarios: "Cenários de teste",
  manual: "Manual do usuário",
  prototipo: "Protótipo de telas",
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
  demanda: "Demanda",
  pessoa: "Pessoa",
  ata: "Ata de reunião",
  "analise-reuniao": "Análise de reunião",
  "parecer-revisao": "Parecer de revisão",
  historias: "Histórias e critérios de aceite",
  "cenarios-teste": "Cenários de teste",
  premissa: "Premissa",
  decisao: "Decisão",
  risco: "Risco",
  restricao: "Restrição",
  projeto: "Projeto",
  "regras-do-projeto": "Regras do projeto",
  "doc-externo": "Documento externo",
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
