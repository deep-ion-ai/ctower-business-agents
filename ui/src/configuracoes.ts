import { invoke } from "@tauri-apps/api/core";
import { apiAnaliseTauri, type AmbienteClaude, type PlanoPreparo, type SituacaoProtecao } from "./analise";
import {
  apiPrimeiroUsoTauri,
  type ApiPrimeiroUso,
  type AvaliacaoBase,
  type ClientePreferido,
  type PreviaSistema,
  type SistemaCadastrado,
} from "./primeiroUso";

/**
 * Espelho dos comandos das configurações (`ui/src-tauri/src/configuracoes.rs`, campos
 * em camelCase). A tela reaproveita os componentes do primeiro uso, então a API
 * estende {@link ApiPrimeiroUso}.
 */
export interface PastaConfig {
  caminho: string;
  prefixo: string | null;
  /** A pasta ainda existe no disco. */
  existe: boolean;
}

export interface SistemaConfig {
  id: string;
  nome: string;
  pastas: PastaConfig[];
  exclusoes: string[];
  inclusoes: string[];
}

export interface Configuracoes {
  cliente: ClientePreferido | null;
  base: string | null;
  baseSugerida: string | null;
  sistemas: SistemaConfig[];
  /** Arquivos da configuração do Claude Code que o app altera. */
  configClaude: { estado: string; settings: string } | null;
}

export type ModoTroca = "mover" | "reapontar";

export interface AvaliacaoTroca {
  destino: AvaliacaoBase;
  atual: string | null;
  arquivos: number;
  bytes: number;
  recusaMover: string | null;
  recusaReapontar: string | null;
  /** Sessões do Claude ainda abertas, que podem gravar na base durante a troca. */
  sessoesAbertas: number;
}

export interface TrocaBase {
  modo: ModoTroca;
  base: string;
  anterior: string | null;
  copiados: number;
  origemApagada: boolean;
  /** Documentos no índice reconstruído (`null` se a reconstrução falhou). */
  indexados: number | null;
  avisos: string[];
}

export interface AjusteExclusoes {
  sistema: string;
  exclusoes: string[];
  inclusoes: string[];
}

export interface AmbienteDesfeito {
  protecaoTirada: string[];
  preparo: PlanoPreparo;
  avisos: string[];
}

export interface Descadastro {
  ambiente: AmbienteDesfeito;
  /** Pasta do sistema na base, que fica. */
  documentos: string | null;
}

export type GrupoTexto = "template" | "roteiro" | "prompt" | "regrasDoSistema";
export type EstadoTexto = "padrao" | "igualAoPadrao" | "editado" | "ilegivel";

export interface TextoEditavel {
  grupo: GrupoTexto;
  id: string;
  rotulo: string;
  /** Relativo à raiz da base. */
  arquivo: string;
  estado: EstadoTexto;
}

type Situacao = "ok" | "ausente" | "desatualizado" | "conflito";

/** Detalhe da verificação do ambiente (`cba_session::preparo::Verificacao`). */
export interface VerificacaoAmbiente {
  diretorio: string;
  chave: string;
  servidorMcp: Situacao;
  hooks: { evento: string; situacao: Situacao }[];
  hooksDesativados: boolean;
  pronto: boolean;
}

export interface AmbienteDetalhado extends AmbienteClaude {
  verificacao?: VerificacaoAmbiente;
}

export interface ApiConfiguracoes extends ApiPrimeiroUso {
  configuracoes(): Promise<Configuracoes>;
  verificarAmbiente(sistema: string): Promise<AmbienteDetalhado>;
  avaliarTroca(destino: string): Promise<AvaliacaoTroca>;
  /** Só depois da confirmação do analista. */
  trocarBase(destino: string, modo: ModoTroca): Promise<TrocaBase>;
  previaExclusoes(ajuste: AjusteExclusoes): Promise<PreviaSistema>;
  salvarExclusoes(ajuste: AjusteExclusoes): Promise<SistemaCadastrado>;
  /** Proteção, servidor MCP e (no último sistema) hooks. Só depois da confirmação. */
  desfazerAmbiente(sistema: string): Promise<AmbienteDesfeito>;
  /** Desfaz o ambiente e apaga o cadastro; os documentos ficam. Só depois da confirmação. */
  descadastrar(sistema: string): Promise<Descadastro>;
  textos(): Promise<TextoEditavel[]>;
  /** Abre no editor do sistema operacional; devolve o caminho. */
  abrirTexto(arquivo: string): Promise<string>;
  situacaoProtecao(sistema: string): Promise<SituacaoProtecao>;
  mudarProtecao(sistema: string, ligada: boolean): Promise<SituacaoProtecao>;
}

export const apiConfiguracoesTauri: ApiConfiguracoes = {
  ...apiPrimeiroUsoTauri,
  configuracoes: () => invoke("configuracoes_app"),
  avaliarTroca: (destino) => invoke("avaliar_troca_base_app", { destino }),
  trocarBase: (destino, modo) => invoke("trocar_base_app", { destino, modo }),
  previaExclusoes: (ajuste) => invoke("previa_exclusoes_app", { ajuste }),
  salvarExclusoes: (ajuste) => invoke("salvar_exclusoes_app", { ajuste }),
  desfazerAmbiente: (sistema) => invoke("desfazer_ambiente_sistema", { sistema }),
  descadastrar: (sistema) => invoke("descadastrar_sistema_app", { sistema }),
  textos: () => invoke("textos_editaveis_app"),
  abrirTexto: (arquivo) => invoke("abrir_texto_editavel", { arquivo }),
  situacaoProtecao: (sistema) => invoke("situacao_protecao_sistema", { sistema }),
  mudarProtecao: (sistema, ligada) => invoke("mudar_protecao_sistema", { sistema, ligada }),
  verificarAmbiente: apiAnaliseTauri.verificarAmbiente,
};

/** Como a tela descreve a situação de um texto editável. */
export const ESTADO_DO_TEXTO: Record<EstadoTexto, string> = {
  padrao: "padrão do app",
  igualAoPadrao: "cópia do padrão na base",
  editado: "editado na base (sobrepõe o padrão)",
  ilegivel: "ilegível (o app usa o padrão)",
};
