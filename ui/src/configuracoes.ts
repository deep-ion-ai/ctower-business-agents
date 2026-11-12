import { invoke } from "@tauri-apps/api/core";
import {
  apiAnaliseTauri,
  type AmbienteClaude,
  type PlanoPreparo,
  type SituacaoProtecao,
} from "./analise";
import {
  apiPrimeiroUsoTauri,
  type ApiPrimeiroUso,
  type AvaliacaoBase,
  type ClientePreferido,
  type PreviaProjeto,
  type ProjetoCadastrado,
} from "./primeiroUso";
import { apiRepositoriosTauri, type ApiRepositorios, type Repositorios } from "./repositorios";

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

export interface ProjetoConfig {
  id: string;
  nome: string;
  pastas: PastaConfig[];
  exclusoes: string[];
  inclusoes: string[];
  /** Repositório de código e de issues; só o comando de credencial, nunca o token. */
  repositorios: Repositorios;
}

export interface Configuracoes {
  cliente: ClientePreferido | null;
  base: string | null;
  baseSugerida: string | null;
  projetos: ProjetoConfig[];
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
  projeto: string;
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
  /** Pasta do projeto na base, que fica. */
  documentos: string | null;
}

export type GrupoTexto = "template" | "roteiro" | "prompt" | "regrasDoProjeto";
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

export interface ApiConfiguracoes extends ApiPrimeiroUso, ApiRepositorios {
  configuracoes(): Promise<Configuracoes>;
  verificarAmbiente(projeto: string): Promise<AmbienteDetalhado>;
  avaliarTroca(destino: string): Promise<AvaliacaoTroca>;
  /** Só depois da confirmação do analista. */
  trocarBase(destino: string, modo: ModoTroca): Promise<TrocaBase>;
  previaExclusoes(ajuste: AjusteExclusoes): Promise<PreviaProjeto>;
  salvarExclusoes(ajuste: AjusteExclusoes): Promise<ProjetoCadastrado>;
  /** Proteção, servidor MCP e (no último projeto) hooks. Só depois da confirmação. */
  desfazerAmbiente(projeto: string): Promise<AmbienteDesfeito>;
  /** Desfaz o ambiente e apaga o cadastro; os documentos ficam. Só depois da confirmação. */
  descadastrar(projeto: string): Promise<Descadastro>;
  textos(): Promise<TextoEditavel[]>;
  /** Abre no editor do sistema operacional; devolve o caminho. */
  abrirTexto(arquivo: string): Promise<string>;
  situacaoProtecao(projeto: string): Promise<SituacaoProtecao>;
  mudarProtecao(projeto: string, ligada: boolean): Promise<SituacaoProtecao>;
}

export const apiConfiguracoesTauri: ApiConfiguracoes = {
  ...apiPrimeiroUsoTauri,
  ...apiRepositoriosTauri,
  configuracoes: () => invoke("configuracoes_app"),
  avaliarTroca: (destino) => invoke("avaliar_troca_base_app", { destino }),
  trocarBase: (destino, modo) => invoke("trocar_base_app", { destino, modo }),
  previaExclusoes: (ajuste) => invoke("previa_exclusoes_app", { ajuste }),
  salvarExclusoes: (ajuste) => invoke("salvar_exclusoes_app", { ajuste }),
  desfazerAmbiente: (projeto) => invoke("desfazer_ambiente_projeto", { projeto }),
  descadastrar: (projeto) => invoke("descadastrar_projeto_app", { projeto }),
  textos: () => invoke("textos_editaveis_app"),
  abrirTexto: (arquivo) => invoke("abrir_texto_editavel", { arquivo }),
  situacaoProtecao: (projeto) => invoke("situacao_protecao_projeto", { projeto }),
  mudarProtecao: (projeto, ligada) => invoke("mudar_protecao_projeto", { projeto, ligada }),
  verificarAmbiente: apiAnaliseTauri.verificarAmbiente,
};

/** Como a tela descreve a situação de um texto editável. */
export const ESTADO_DO_TEXTO: Record<EstadoTexto, string> = {
  padrao: "padrão do app",
  igualAoPadrao: "cópia do padrão na base",
  editado: "editado na base (sobrepõe o padrão)",
  ilegivel: "ilegível (o app usa o padrão)",
};
