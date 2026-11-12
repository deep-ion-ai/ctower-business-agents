import { Channel, invoke } from "@tauri-apps/api/core";
import type { SistemaResumo } from "./terminal";

/**
 * Espelho do estado da análise (`cba_core::analise::Analise`), no formato do arquivo
 * `A-NNNN.estado.json` (campos em snake_case), e dos comandos da tela da análise
 * (`ui/src-tauri/src/analise.rs` e `vigia.rs`, campos em camelCase).
 */
export type Fluxo = "perguntar" | "explicar" | "impacto";
export type EstadoAnalise = "criada" | "em-andamento" | "concluida" | "aprovada";
export type EstadoSessao = "ativa" | "aguardando-analista" | "concluida" | "encerrada";
export type Cliente = "claude-code" | "claude-desktop" | "desconhecido";
export type Confianca = "confirmado" | "inferido" | "a_confirmar";

export interface Evidencia {
  arquivo: string;
  /** Intervalo `inicio-fim`, como `71-72`. */
  linhas: string;
  trecho: string;
  hash: string;
  commit?: string | null;
  estado: "valida" | "a-revalidar";
}

export type EstadoAfirmacao =
  | { estado: "registrada" }
  | { estado: "confirmada" }
  | { estado: "incorreta"; comentario: string }
  | { estado: "aprofundar" }
  | { estado: "a-revalidar" };

export type Afirmacao = {
  numero: number;
  secao: string;
  texto: string;
  /** Texto do agente, guardado quando o analista edita o texto (#37). */
  texto_original?: string | null;
  confianca: Confianca;
  evidencias: Evidencia[];
  sessao?: string | null;
  registrada_ms: number;
} & EstadoAfirmacao;

export type Pergunta = {
  numero: number;
  texto: string;
  afirmacao?: number | null;
  registrada_ms: number;
} & ({ estado: "aberta" } | { estado: "respondida"; resposta: string } | { estado: "descartada" });

export interface Sugestao {
  numero: number;
  id?: string | null;
  titulo: string;
  arquivo: string;
  trecho_atual: string;
  trecho_proposto: string;
  motivo: string;
  impacto: string;
  exemplo?: string | null;
  evidencia?: Evidencia | null;
  estado: "proposta" | "aceita" | "recusada";
  registrada_ms: number;
}

export interface TermoProposto {
  numero: number;
  termo: string;
  definicao: string;
  sinonimos?: string[];
  onde_vive?: string[];
  estado: "proposto" | "aceito" | "recusado";
}

export interface Sessao {
  id: string;
  cliente: Cliente;
  estado: EstadoSessao;
  inicio_ms: number;
  fim_ms?: number | null;
}

export interface Analise {
  id: string;
  sistema: string;
  fluxo: Fluxo;
  titulo: string;
  entrada: string;
  estado: EstadoAnalise;
  documento: string;
  versao: number;
  criada_ms: number;
  atualizada_ms: number;
  sessoes: Sessao[];
  afirmacoes: Afirmacao[];
  perguntas: Pergunta[];
  sugestoes: Sugestao[];
  termos: TermoProposto[];
}

export interface SessaoVista {
  id: string;
  estado: EstadoSessao;
  cliente: Cliente;
}

export interface VistaAnalise {
  analise: Analise;
  /** Caminho absoluto do documento `A-NNNN.md`. */
  documento: string;
  sessao: SessaoVista | null;
  /** Pedido para uma conversa nova continuar a análise (`retomar_analise`). */
  promptRetomada: string;
}

export interface AnaliseResumo {
  id: string;
  titulo: string;
  fluxo: Fluxo;
  estado: EstadoAnalise;
  atualizadaMs: number;
}

export interface AnaliseCriada {
  analise: AnaliseResumo;
  /** Pedido que faz o agente chamar `iniciar_analise` com o A-NNNN criado. */
  promptInicial: string;
}

export interface Trecho {
  arquivo: string;
  inicio: number;
  fim: number;
  /** Número da primeira linha de `linhas`. */
  primeira: number;
  linhas: string[];
  total: number;
}

export interface Situacao {
  base: string | null;
  sistemas: SistemaResumo[];
}

export interface AmbienteClaude {
  pronto: boolean;
  pendencias: string[];
}

export interface AlteracaoPreparo {
  arquivo: string;
  item: { tipo: "servidorMcp"; chave: string } | { tipo: "hook"; evento: string };
  acao: "adicionado" | "atualizado" | "removido" | "semMudanca";
}

export interface PlanoPreparo {
  diretorio: string;
  alteracoes: AlteracaoPreparo[];
  gravado: boolean;
}

/** Arquivo da pasta do sistema alterado durante uma sessão (`cba_session::alteracoes::Detalhe`). */
export interface ArquivoAlterado {
  sessao: string;
  /** Raiz do repositório (ou a pasta, fora do git). */
  pasta: string;
  /** Caminho relativo a `pasta`, com `/`. */
  caminho: string;
  tipo: "criado" | "modificado" | "removido";
  /** SHA-256 do conteúdo atual, conferido ao desfazer. */
  hashAtual: string | null;
  /** Há conteúdo de base para o diff e o desfazer (pasta no git). */
  desfazivel: boolean;
  /** Diff unificado (antes → agora). */
  diff: string | null;
  binario: boolean;
}

/** Aviso de alteração de uma análise (`alteracoes_da_analise_sistema`). */
export interface AvisoAlteracao {
  arquivos: ArquivoAlterado[];
  /** Limitações da conferência (git ausente, pasta fora do git). */
  avisos: string[];
}

/** Arquivo a desfazer ou manter. */
export interface PedidoAlteracao {
  sessao: string;
  pasta: string;
  caminho: string;
  hashAtual: string | null;
}

/** Situação da proteção opcional contra edição (`cba_session::protecao::SituacaoProtecao`). */
export interface SituacaoProtecao {
  sistema: string;
  ligada: boolean;
  completa: boolean;
  /** `~/.claude/settings.json`. */
  arquivo: string;
  regras: { regra: string; presente: boolean; doApp: boolean }[];
  negadasPeloHook: string[];
}

/** Aviso da vigia do diário de eventos. */
export type AvisoAnalise =
  { tipo: "mudanca"; analise: string; mudanca: string; versao: number } | { tipo: "reinicio" };

/** Prompt pronto (#32): `prompts_prontos { sistema, analise }`. */
export interface PromptPronto {
  /** aprofundar, resumir, criterios-de-aceite, conferir-evidencias, retomar. */
  id: string;
  /** Rótulo do botão. */
  rotulo: string;
  /** Dica do botão: o que o pedido faz. */
  descricao: string;
  /** O pedido, numa linha só, com as variáveis preenchidas. */
  texto: string;
}

/** Intervalo `71-72` (ou `71`) em números. */
export function intervaloDe(linhas: string): { inicio: number; fim: number } {
  const [a, b] = linhas.split("-");
  const inicio = Number(a);
  const fim = b === undefined ? inicio : Number(b);
  return { inicio, fim };
}

/** Mensagem de um erro vindo do Rust (texto) ou do JavaScript. */
export function mensagemDe(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

/** Verificação e preparo do ambiente do Claude, usados pela análise e pelo primeiro uso. */
export interface ApiAmbiente {
  verificarAmbiente(sistema: string): Promise<AmbienteClaude>;
  /** O que o preparo mudaria, sem gravar. */
  planejarPreparo(sistema: string): Promise<PlanoPreparo>;
  /** Prepara; só depois do consentimento do analista. */
  prepararAmbiente(sistema: string): Promise<PlanoPreparo>;
  /** Proteção opcional contra edição (#29); a tela de configurações (#39) também usa. */
  situacaoProtecao?(sistema: string): Promise<SituacaoProtecao>;
  /** Liga ou desliga a proteção; só depois do consentimento do analista. */
  mudarProtecao?(sistema: string, ligada: boolean): Promise<SituacaoProtecao>;
}

/** Aviso de alteração na pasta do sistema (#29). */
export interface ApiAlteracoes {
  alteracoes(sistema: string, analise: string): Promise<AvisoAlteracao>;
  /** Volta os arquivos ao início da sessão; só depois da confirmação na tela. */
  desfazer(sistema: string, analise: string, pedidos: PedidoAlteracao[]): Promise<string[]>;
  /** O analista mantém as alterações: elas saem do aviso. */
  manter(sistema: string, analise: string, pedidos: PedidoAlteracao[]): Promise<string[]>;
}

export const apiAlteracoesTauri: ApiAlteracoes = {
  alteracoes: (sistema, analise) => invoke("alteracoes_da_analise_sistema", { sistema, analise }),
  desfazer: (sistema, analise, pedidos) =>
    invoke("desfazer_alteracoes_sistema", { sistema, analise, pedidos }),
  manter: (sistema, analise, pedidos) =>
    invoke("manter_alteracoes_sistema", { sistema, analise, pedidos }),
};

/** Operações da tela da análise. A interface usa {@link apiAnaliseTauri}; os testes, uma falsa. */
export interface ApiAnalise extends ApiAmbiente {
  /** A base e os sistemas (o comando devolve também os campos do primeiro uso). */
  situacao(): Promise<Situacao>;
  listarAnalises(sistema: string): Promise<AnaliseResumo[]>;
  criarAnalise(sistema: string, entrada: string, titulo?: string): Promise<AnaliseCriada>;
  carregarAnalise(sistema: string, analise: string): Promise<VistaAnalise>;
  trechoEvidencia(sistema: string, arquivo: string, inicio: number, fim: number): Promise<Trecho>;
  /** Passa a receber os avisos do sistema; devolve a função que para. */
  vigiar(sistema: string, aoAviso: (a: AvisoAnalise) => void): Promise<() => void>;
  /** Prompts prontos da análise; lista vazia se o comando não existir (antes da #32). */
  promptsProntos(sistema: string, analise: string): Promise<PromptPronto[]>;
}

export const apiAnaliseTauri: ApiAnalise = {
  situacao: () => invoke("situacao_app"),
  verificarAmbiente: (sistema) => invoke("verificar_ambiente_sistema", { sistema }),
  planejarPreparo: (sistema) => invoke("planejar_preparo_sistema", { sistema }),
  prepararAmbiente: (sistema) => invoke("preparar_ambiente_sistema", { sistema }),
  situacaoProtecao: (sistema) => invoke("situacao_protecao_sistema", { sistema }),
  mudarProtecao: (sistema, ligada) => invoke("mudar_protecao_sistema", { sistema, ligada }),
  listarAnalises: (sistema) => invoke("listar_analises_sistema", { sistema }),
  criarAnalise: (sistema, entrada, titulo) =>
    invoke("criar_analise_pergunta", { sistema, entrada, titulo: titulo ?? null }),
  carregarAnalise: (sistema, analise) => invoke("carregar_analise_sistema", { sistema, analise }),
  trechoEvidencia: (sistema, arquivo, inicio, fim) =>
    invoke("trecho_evidencia_sistema", { sistema, arquivo, inicio, fim }),
  async vigiar(sistema, aoAviso) {
    const canal = new Channel<AvisoAnalise>();
    canal.onmessage = aoAviso;
    const id = await invoke<number>("vigiar_analises", { sistema, canal });
    return () => {
      canal.onmessage = () => undefined;
      invoke("parar_vigia", { id }).catch(() => undefined);
    };
  },
  async promptsProntos(sistema, analise) {
    try {
      const lista = await invoke<PromptPronto[]>("prompts_prontos", { sistema, analise });
      return Array.isArray(lista) ? lista : [];
    } catch {
      // O comando chega com a #32; sem ele (ou com falha), a tela fica sem os botões.
      return [];
    }
  },
};
