import { Channel, invoke } from "@tauri-apps/api/core";
import type { ProjetoResumo } from "./terminal";

/**
 * Espelho do estado da análise (`cba_core::analise::Analise`), no formato do arquivo
 * `A-NNNN.estado.json` (campos em snake_case), e dos comandos da tela da análise
 * (`ui/src-tauri/src/analise.rs` e `vigia.rs`, campos em camelCase).
 */
export type Fluxo =
  | "perguntar"
  | "explicar"
  | "impacto"
  | "reuniao"
  | "revisar"
  | "historias"
  | "cenarios";
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
  | { estado: "aprofundar"; pedido?: string }
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
  /** A resposta já foi enviada ao agente ("Enviar respostas"). */
  resposta_enviada?: boolean;
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
  /** Definição do agente, quando o analista a editou. */
  definicao_original?: string;
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
  projeto: string;
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
  /** Reunião analisada (fluxo `reuniao`), `R-0001`. */
  reuniao?: string | null;
  /** Análise de origem (fluxo `historias`), `A-0001`. */
  origem?: string | null;
  /** Histórias de usuário (fluxo `historias`, #115). */
  historias?: Historia[];
  /** Tabelas de decisão (fluxo `cenarios`, #117). */
  tabelas?: TabelaDecisao[];
  /** Cenários de teste (fluxo `cenarios`, #117). */
  cenarios?: Cenario[];
  /** Parecer de revisão (fluxo `revisar`, `cba_core::parecer::Parecer`). */
  parecer?: Parecer | null;
}

/** Característica da ISO/IEC/IEEE 29148 violada (`cba_core::qualidade`). */
export type Caracteristica =
  | "ambiguidade"
  | "verificabilidade"
  | "completude"
  | "consistencia"
  | "correcao"
  | "singularidade";
export type Severidade = "alta" | "media" | "baixa";
export type Veredito = "pronta" | "pronta_com_ressalvas" | "precisa_de_retrabalho";

export type Achado = {
  numero: number;
  caracteristica: Caracteristica;
  severidade: Severidade;
  /** Trecho do texto revisado; vazio só na completude. */
  trecho: string;
  problema: string;
  reescrita?: string | null;
  /** Regras da base (`RN-FAT-012`). */
  regras?: string[];
  evidencias: Evidencia[];
  registrada_ms: number;
} & ({ estado: "registrado" } | { estado: "descartado"; motivo?: string | null });

export interface AvaliacaoInvest {
  criterio: "independente" | "negociavel" | "valiosa" | "estimavel" | "pequena" | "testavel";
  resultado: "atende" | "atende_em_parte" | "nao_atende";
  justificativa: string;
}

export interface Parecer {
  achados: Achado[];
  invest?: AvaliacaoInvest[];
  veredito?: { veredito: Veredito; justificativa: string; registrada_ms: number } | null;
}

/** O que sustenta um critério (`cba_core::requisito::Fundamento`). */
export type Fundamento =
  | { tipo: "afirmacao"; ref: string; texto: string; evidencias?: unknown[] }
  | { tipo: "regra"; id: string; titulo: string; evidencias?: unknown[] };

export type SituacaoItem = "pendente" | "aceito" | "descartado";
export type SituacaoHistoria = "pendente" | "aceita" | "descartada" | "quebrada";
export type LetraInvest =
  "independente" | "negociavel" | "valiosa" | "estimavel" | "pequena" | "testavel";

export interface Gherkin {
  dado: string;
  quando: string;
  entao: string;
}

export interface Narrativa {
  titulo: string;
  como: string;
  quero: string;
  para: string;
}

/** Critério de aceite (`cba_core::historias::Criterio`), `CA-<numero>`. */
export interface Criterio extends Gherkin {
  numero: number;
  original?: Gherkin | null;
  fundamentos: Fundamento[];
  /** Pergunta de que depende (`A-0001/2`): o critério fica "a confirmar". */
  pergunta?: string | null;
  situacao: SituacaoItem;
  registrado_ms: number;
}

/** História de usuário (`cba_core::historias::Historia`), `HU-NNNN`. */
export interface Historia extends Narrativa {
  id: string;
  original?: Narrativa | null;
  criterios: Criterio[];
  invest?: { criterio: LetraInvest; motivo: string }[];
  quebra?: string | null;
  fatia_de?: string | null;
  situacao: SituacaoHistoria;
  /** O analista pediu a quebra em fatias (texto vazio: sem detalhe). */
  quebra_pedida?: string | null;
  registrada_ms: number;
}

/** Técnica de um cenário de teste (`cba_core::cenarios::Tecnica`). */
export type Tecnica =
  | "tabela_de_decisao"
  | "particao_de_equivalencia"
  | "valor_limite"
  | "negativo"
  | "excecao";

/** Evidência registrada (`cba_core::analise::Evidencia`), como o estado a guarda. */
export interface EvidenciaRegistrada {
  arquivo: string;
  linhas: string;
  trecho?: string;
}

/** Tabela de decisão (`cba_core::cenarios::TabelaDecisao`), `TD-<numero>`. */
export interface TabelaDecisao {
  numero: number;
  titulo: string;
  condicoes: { texto: string; evidencias?: EvidenciaRegistrada[] }[];
  acoes: string[];
  colunas: {
    numero: number;
    /** Um valor por condição: S, N ou - (indiferente). */
    valores: ("S" | "N" | "-")[];
    acoes?: number[];
    impossivel?: string | null;
  }[];
  fundamentos: Fundamento[];
  situacao: SituacaoItem;
  registrada_ms: number;
}

/** Limite lido do código (`cba_core::cenarios::Limite`). */
export interface LimiteCenario {
  grandeza: string;
  valor: string;
  evidencias: EvidenciaRegistrada[];
  /** Parâmetro de configuração de onde o valor vem: o cenário fica "a confirmar". */
  configuracao?: string | null;
}

/** Cenário de teste (`cba_core::cenarios::Cenario`), `CT-NNNN`. */
export interface Cenario extends Gherkin {
  id: string;
  tecnica: Tecnica;
  titulo: string;
  original?: (Gherkin & { titulo: string }) | null;
  valor?: string | null;
  limite?: LimiteCenario | null;
  /** Coluna da tabela de decisão que o cenário exercita. */
  coluna?: { tabela: number; coluna: number } | null;
  /** História que o cenário verifica (`HU-0001`). */
  historia?: string | null;
  fundamentos: Fundamento[];
  pergunta?: string | null;
  situacao: SituacaoItem;
  registrado_ms: number;
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
  projetos: ProjetoResumo[];
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

/** Arquivo da pasta do projeto alterado durante uma sessão (`cba_session::alteracoes::Detalhe`). */
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

/** Aviso de alteração de uma análise (`alteracoes_da_analise_projeto`). */
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
  projeto: string;
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

/** Prompt pronto (#32): `prompts_prontos { projeto, analise }`. */
export interface PromptPronto {
  /**
   * aprofundar, resumir, criterios-de-aceite, conferir-evidencias, retomar; na análise
   * de reunião, resumir-para-o-cliente e listar-duvidas (o app filtra pelo fluxo).
   */
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
  verificarAmbiente(projeto: string): Promise<AmbienteClaude>;
  /** O que o preparo mudaria, sem gravar. */
  planejarPreparo(projeto: string): Promise<PlanoPreparo>;
  /** Prepara; só depois do consentimento do analista. */
  prepararAmbiente(projeto: string): Promise<PlanoPreparo>;
  /** Proteção opcional contra edição (#29); a tela de configurações (#39) também usa. */
  situacaoProtecao?(projeto: string): Promise<SituacaoProtecao>;
  /** Liga ou desliga a proteção; só depois do consentimento do analista. */
  mudarProtecao?(projeto: string, ligada: boolean): Promise<SituacaoProtecao>;
}

/** Aviso de alteração na pasta do projeto (#29). */
export interface ApiAlteracoes {
  alteracoes(projeto: string, analise: string): Promise<AvisoAlteracao>;
  /** Volta os arquivos ao início da sessão; só depois da confirmação na tela. */
  desfazer(projeto: string, analise: string, pedidos: PedidoAlteracao[]): Promise<string[]>;
  /** O analista mantém as alterações: elas saem do aviso. */
  manter(projeto: string, analise: string, pedidos: PedidoAlteracao[]): Promise<string[]>;
}

export const apiAlteracoesTauri: ApiAlteracoes = {
  alteracoes: (projeto, analise) => invoke("alteracoes_da_analise_projeto", { projeto, analise }),
  desfazer: (projeto, analise, pedidos) =>
    invoke("desfazer_alteracoes_projeto", { projeto, analise, pedidos }),
  manter: (projeto, analise, pedidos) =>
    invoke("manter_alteracoes_projeto", { projeto, analise, pedidos }),
};

/** Operações da tela da análise. A interface usa {@link apiAnaliseTauri}; os testes, uma falsa. */
export interface ApiAnalise extends ApiAmbiente {
  /** A base e os projetos (o comando devolve também os campos do primeiro uso). */
  situacao(): Promise<Situacao>;
  listarAnalises(projeto: string): Promise<AnaliseResumo[]>;
  criarAnalise(projeto: string, entrada: string, titulo?: string): Promise<AnaliseCriada>;
  carregarAnalise(projeto: string, analise: string): Promise<VistaAnalise>;
  trechoEvidencia(projeto: string, arquivo: string, inicio: number, fim: number): Promise<Trecho>;
  /** Passa a receber os avisos do projeto; devolve a função que para. */
  vigiar(projeto: string, aoAviso: (a: AvisoAnalise) => void): Promise<() => void>;
  /** Prompts prontos da análise; lista vazia se o comando não existir (antes da #32). */
  promptsProntos(projeto: string, analise: string): Promise<PromptPronto[]>;
}

export const apiAnaliseTauri: ApiAnalise = {
  situacao: () => invoke("situacao_app"),
  verificarAmbiente: (projeto) => invoke("verificar_ambiente_projeto", { projeto }),
  planejarPreparo: (projeto) => invoke("planejar_preparo_projeto", { projeto }),
  prepararAmbiente: (projeto) => invoke("preparar_ambiente_projeto", { projeto }),
  situacaoProtecao: (projeto) => invoke("situacao_protecao_projeto", { projeto }),
  mudarProtecao: (projeto, ligada) => invoke("mudar_protecao_projeto", { projeto, ligada }),
  listarAnalises: (projeto) => invoke("listar_analises_projeto", { projeto }),
  criarAnalise: (projeto, entrada, titulo) =>
    invoke("criar_analise_pergunta", { projeto, entrada, titulo: titulo ?? null }),
  carregarAnalise: (projeto, analise) => invoke("carregar_analise_projeto", { projeto, analise }),
  trechoEvidencia: (projeto, arquivo, inicio, fim) =>
    invoke("trecho_evidencia_projeto", { projeto, arquivo, inicio, fim }),
  async vigiar(projeto, aoAviso) {
    const canal = new Channel<AvisoAnalise>();
    canal.onmessage = aoAviso;
    const id = await invoke<number>("vigiar_analises", { projeto, canal });
    return () => {
      canal.onmessage = () => undefined;
      invoke("parar_vigia", { id }).catch(() => undefined);
    };
  },
  async promptsProntos(projeto, analise) {
    try {
      const lista = await invoke<PromptPronto[]>("prompts_prontos", { projeto, analise });
      return Array.isArray(lista) ? lista : [];
    } catch {
      // O comando chega com a #32; sem ele (ou com falha), a tela fica sem os botões.
      return [];
    }
  },
};
