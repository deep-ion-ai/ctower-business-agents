import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type { AnaliseCriada, AnaliseResumo, EstadoAnalise, Fluxo } from "./analise";
import type { StatusRegra } from "./conhecimento";
import type { RastreadorVista } from "./demandas";
import { contar } from "./formatacao";

/**
 * Mapa de funcionalidades do projeto (#208): um cadastro do projeto, fora das
 * análises. Espelho dos comandos de `ui/src-tauri/src/funcionalidades.rs`
 * (campos em camelCase).
 */

/** Natureza de um ponto de entrada (`cba_index::TipoEntrada`). */
export type TipoEntrada = "tela" | "endpoint" | "lote" | "rotina_banco" | "servico" | "integracao";

export const NOME_TIPO_ENTRADA: Record<TipoEntrada, string> = {
  tela: "tela",
  endpoint: "endpoint",
  lote: "lote",
  rotina_banco: "rotina de banco",
  servico: "serviço",
  integracao: "integração",
};

export type MotivoSemFuncionalidade = "codigo_morto" | "utilitario" | "outro";

export const NOME_MOTIVO: Record<MotivoSemFuncionalidade, string> = {
  codigo_morto: "código morto",
  utilitario: "utilitário",
  outro: "outro motivo",
};

export type SituacaoFuncionalidade = "pendente" | "aceita" | "descartada" | "juntada";

export interface PontoDaFuncionalidade {
  nome: string;
  chave: string | null;
  tipo: TipoEntrada | null;
  arquivo: string;
  linhaInicial: number;
  linhaFinal: number;
  trecho: string;
  /** Fora da lista do mapa estrutural: só a evidência o sustenta. */
  foraDaLista: boolean;
}

export interface Funcionalidade {
  id: string;
  nome: string;
  area: string;
  areaNome: string;
  descricao: string;
  atores: string[];
  pontos: PontoDaFuncionalidade[];
  dadosLidos: string[];
  dadosGravados: string[];
  relacionadas: string[];
  situacao: SituacaoFuncionalidade;
  juntadaEm: string | null;
  editada: boolean;
  /** Documento negocial aprovado, relativo à raiz da base. */
  documento: string | null;
  /** Análise de explicação ligada (a mais recente), se houver. */
  explicacao: AnaliseResumo | null;
  /** Nenhum ponto de entrada dela existe mais no código. */
  semPontoDeEntrada: boolean;
  /** Manual do usuário, quando o mapa o informa (senão só o painel sabe). */
  manual?: SituacaoManual;
}

export interface AreaMapa {
  area: string;
  nome: string;
  funcionalidades: Funcionalidade[];
}

export interface PontoSemTratamento {
  chave: string;
  tipo: TipoEntrada;
  nome: string;
  arquivo: string;
  linha: number;
}

export interface PontoJustificado {
  chave: string;
  tipo: TipoEntrada;
  nome: string;
  arquivo: string;
  motivo: MotivoSemFuncionalidade;
  justificativa: string;
}

export interface Completude {
  total: number;
  semTratamento: PontoSemTratamento[];
}

export interface MapaFuncionalidades {
  areas: AreaMapa[];
  /** Descartadas e juntadas. */
  fechadas: Funcionalidade[];
  total: number;
  pendentes: number;
  documentadas: number;
  justificados: PontoJustificado[];
  /** Sem ela, o mapa estrutural não pôde ser lido (`erroCompletude`). */
  completude: Completude | null;
  erroCompletude: string | null;
  /** `funcionalidades/mapa.md`, relativo à base, quando existe. */
  documento: string | null;
  versao: number;
}

/** Decisão do analista (`cba_tools::mapear::RevisaoDoMapa`). */
export type RevisaoDoMapa =
  | { tipo: "decidir"; funcionalidade: string; decisao: "aceitar" | "descartar" | "desfazer" }
  | { tipo: "editar"; funcionalidade: string; nome: string; area: string; descricao: string }
  | { tipo: "juntar"; origem: string; destino: string }
  | {
      tipo: "dividir";
      funcionalidade: string;
      /** Índices (a partir de 0) dos pontos de entrada que vão para a nova. */
      pontos: number[];
      nome: string;
      descricao: string;
    }
  | { tipo: "desfazer-justificativa"; ponto: string }
  | { tipo: "ligar-demanda"; funcionalidade: string; demanda: string; ligar: boolean };

/** Painel da funcionalidade (`painel_funcionalidade.rs`). */
export type Ligacao = "explica" | "funcionalidade" | "demanda" | "codigo";

export const NOME_LIGACAO: Record<Ligacao, string> = {
  explica: "explica esta",
  funcionalidade: "aberta aqui",
  demanda: "pela demanda",
  codigo: "pelo código",
};

export interface AnaliseNoPainel extends AnaliseResumo {
  ligacao: Ligacao;
  demanda: string | null;
  perguntasAbertas: number;
}

export interface PerguntaNoPainel {
  analise: string;
  numero: number;
  texto: string;
  destinatario: string | null;
  demanda: string | null;
}

export interface EtapaDemanda {
  analise: string;
  estado: EstadoAnalise;
}

export interface DemandaNoPainel {
  id: string;
  titulo: string;
  /** Pelo status efetivo: o conferido no rastreador, quando há (#292). */
  aberta: boolean;
  /** Estado conferido no rastreador, se a demanda veio de uma issue. */
  rastreador: RastreadorVista | null;
  /** Rastreador inacessível ou issue não encontrada na última conferência. */
  avisoRastreador: string | null;
  criado: string;
  /** Chave e endereço da issue, quando a demanda veio do rastreador (#270). */
  chave: string | null;
  origem: string | null;
  ligadaNoPainel: boolean;
  impacto: EtapaDemanda | null;
  historias: EtapaDemanda | null;
  cenarios: EtapaDemanda | null;
  perguntas: PerguntaNoPainel[];
}

export interface RegraNoPainel {
  id: string;
  titulo: string;
  /** Enunciado completo (a seção "Enunciado" do documento). */
  enunciado: string;
  status: StatusRegra;
  caminho: string;
}

export interface PainelFuncionalidade {
  funcionalidade: Funcionalidade;
  analises: AnaliseNoPainel[];
  demandas: DemandaNoPainel[];
  regras: RegraNoPainel[];
  perguntas: PerguntaNoPainel[];
  aRevisar: string[];
  relacionadas: { id: string; nome: string }[];
  atividade: { quandoMs: number; texto: string }[];
  demandasDoProjeto: { id: string; titulo: string }[];
  /** Manual do usuário da funcionalidade (#272). */
  manual?: SituacaoManual;
}

/** Situação do manual do usuário (`cba_tools::manual::SituacaoManual`). */
export type SituacaoManual =
  | { situacao: "nenhum" }
  | {
      situacao: "emAndamento";
      analise: string;
      estado: EstadoAnalise;
      passos: number;
      /** Passos sem print e sem dispensa. */
      semPrint: number;
    }
  | {
      situacao: "aprovado";
      analise: string;
      /** `manual.md` na base, relativo à raiz da base. */
      documento: string | null;
      /** Motivos para revisar (a funcionalidade mudou depois da aprovação). */
      aRevisar: string[];
    };

/** Vincular as issues do rastreador às funcionalidades (`vincular_issues.rs`, #270). */
export interface SugestaoDeIssue {
  funcionalidade: string;
  nome: string;
  pontuacao: number;
  evidencias: string[];
  /** Veio de uma proposta do agente (pontuação 0), não do texto. */
  ia: boolean;
}

export interface IssueDoRastreador {
  origem: "github" | "gitlab" | "jira";
  chave: string;
  titulo: string;
  descricao: string;
  url: string;
  criadaEm: string;
  estado: "aberta" | "fechada";
  rotulos: string[];
  sugestoes: SugestaoDeIssue[];
  /** Demanda da base que já representa a issue. */
  demanda: string | null;
  /** Funcionalidades a que essa demanda já está ligada. */
  ligadaA: string[];
  /** Por que a issue fica sem funcionalidade, se o analista justificou. */
  justificativa: JustificativaDeIssue | null;
}

export type MotivoDeIssue = "fora_do_escopo" | "infraestrutura" | "outro";

export interface JustificativaDeIssue {
  motivo: MotivoDeIssue;
  texto: string;
}

export interface VistaDeIssues {
  issues: IssueDoRastreador[];
  /** O período devolveu mais issues que o teto. */
  excedeuTeto: boolean;
  desde: string;
  funcionalidades: { id: string; nome: string; areaNome: string }[];
}

export type IssueParaConfirmar = Omit<IssueDoRastreador, "sugestoes" | "demanda" | "ligadaA" | "justificativa">;

export interface ItemConfirmado {
  issue: IssueParaConfirmar;
  funcionalidades: string[];
}

export interface ResultadoDoLote {
  /** A demanda da base de cada issue confirmada. */
  demandas: { origem: "github" | "gitlab" | "jira"; chave: string; demanda: string }[];
  criadas: string[];
  reaproveitadas: string[];
  vinculosNovos: number;
  vinculosExistentes: number;
  avisos: string[];
}

/** Documentos religados às funcionalidades (`cba_tools::vinculo::Religacao`, #287). */
export interface Religacao {
  analises: string[];
  regras: string[];
  termos: string[];
  negociais: string[];
  /** Regras que continuam sem funcionalidade (a origem não leva a uma). */
  regrasSemVinculo: number;
  avisos: string[];
}

/** Resumo da religação, para o aviso da tela. */
export function resumoReligacao(r: Religacao): string {
  const partes = [
    r.analises.length && contar(r.analises.length, "análise", "análises"),
    r.regras.length && contar(r.regras.length, "regra"),
    r.termos.length && contar(r.termos.length, "termo"),
    r.negociais.length && contar(r.negociais.length, "documento negocial", "documentos negociais"),
  ].filter(Boolean);
  const feito =
    partes.length > 0
      ? `Religados às funcionalidades: ${partes.join(", ")}.`
      : "Nenhum documento a religar.";
  const sem =
    r.regrasSemVinculo > 0
      ? ` ${contar(r.regrasSemVinculo, "regra continua", "regras continuam")} sem funcionalidade (a análise de origem não tem uma).`
      : "";
  return feito + sem;
}

export interface ApiMapa {
  /** Lê as issues do período (meses ou data `AAAA-MM-DD`) e sugere a funcionalidade. */
  issues: (projeto: string, meses: number | null, desde: string | null) => Promise<VistaDeIssues>;
  /** Confirma o lote: cria ou reaproveita as demandas e liga às funcionalidades. */
  vincular: (projeto: string, itens: ItemConfirmado[]) => Promise<ResultadoDoLote>;
  /** Justifica a issue sem funcionalidade; `null` tira a justificativa. */
  justificar: (
    projeto: string,
    issue: IssueParaConfirmar,
    justificativa: JustificativaDeIssue | null,
  ) => Promise<void>;
  /** Pedido que abre a sessão do Claude para propor o vínculo das issues. */
  pedidoVinculoIa: (projeto: string, issues: IssueParaConfirmar[]) => Promise<string>;
  mapa: (projeto: string) => Promise<MapaFuncionalidades>;
  revisar: (projeto: string, revisao: RevisaoDoMapa) => Promise<MapaFuncionalidades>;
  /** Pedido que abre a sessão de mapeamento no terminal do Claude. */
  pedido: (pasta: string | null, area: string | null, orientacao: string | null) => Promise<string>;
  explicar: (projeto: string, funcionalidade: string) => Promise<AnaliseCriada>;
  painel: (projeto: string, funcionalidade: string) => Promise<PainelFuncionalidade>;
  /** Análise de um fluxo aberta pelo painel, com a funcionalidade guardada nela. */
  criarAnalise: (
    projeto: string,
    funcionalidade: string,
    fluxo: Fluxo,
    texto: string | null,
    demanda: string | null,
  ) => Promise<AnaliseCriada>;
  /** Diálogo de salvar; `null` se o analista cancelar. */
  escolherDestino: (formato: "markdown" | "word") => Promise<string | null>;
  /** Exporta o documento do mapa e devolve o caminho gravado. */
  exportar: (projeto: string, destino: string, formato: "markdown" | "word") => Promise<string>;
  /** Liga às funcionalidades os documentos aprovados antes do vínculo (#287). */
  religar?: (projeto: string) => Promise<Religacao>;
}

export const apiMapaTauri: ApiMapa = {
  issues: (projeto, meses, desde) => invoke("issues_para_vincular_app", { projeto, meses, desde }),
  vincular: (projeto, itens) => invoke("confirmar_vinculos_app", { projeto, itens }),
  pedidoVinculoIa: (projeto, issues) => invoke("pedido_vinculo_ia_app", { projeto, issues }),
  justificar: (projeto, issue, justificativa) =>
    invoke("justificar_issue_app", { projeto, issue, justificativa }),
  mapa: (projeto) => invoke("mapa_funcionalidades_projeto", { projeto }),
  revisar: (projeto, revisao) => invoke("revisar_mapa_projeto", { projeto, revisao }),
  pedido: (pasta, area, orientacao) => invoke("pedido_de_mapeamento", { pasta, area, orientacao }),
  explicar: (projeto, funcionalidade) =>
    invoke("explicar_funcionalidade_app", { projeto, funcionalidade }),
  painel: (projeto, funcionalidade) =>
    invoke("painel_funcionalidade_app", { projeto, funcionalidade }),
  criarAnalise: (projeto, funcionalidade, fluxo, texto, demanda) =>
    invoke("criar_analise_da_funcionalidade_app", {
      projeto,
      funcionalidade,
      fluxo,
      texto,
      demanda,
    }),
  escolherDestino: (formato) =>
    save({
      title: "Exportar o mapa de funcionalidades",
      defaultPath: formato === "word" ? "mapa-funcionalidades.docx" : "mapa-funcionalidades.md",
      filters: [
        formato === "word"
          ? { name: "Documento do Word", extensions: ["docx"] }
          : { name: "Markdown", extensions: ["md"] },
      ],
    }),
  exportar: (projeto, destino, formato) =>
    invoke("exportar_mapa_projeto", { projeto, destino, formato }),
  religar: (projeto) => invoke("religar_funcionalidades_app", { projeto }),
};
