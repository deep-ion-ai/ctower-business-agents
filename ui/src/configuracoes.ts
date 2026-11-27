import {
  apiAmbienteTesteTauri,
  type AmbienteTeste,
  type ApiAmbienteTeste,
} from "./ambienteTeste";
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
import { apiAcessosTauri, type ApiAcessos } from "./acessos";

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
  /** Ambiente de teste (URL, usuários e perfis); nunca a senha. */
  ambienteTeste?: AmbienteTeste | null;
  /** Dados pessoais do texto da demanda mascarados ao exportar e publicar (#454); ausente = ligado. */
  mascararDemanda?: boolean;
}

export interface Configuracoes {
  cliente: ClientePreferido | null;
  base: string | null;
  baseSugerida: string | null;
  projetos: ProjetoConfig[];
  /** Arquivos da configuração do Claude Code que o app altera. */
  configClaude: { estado: string; settings: string } | null;
}

/** A pasta da base como as configurações a mostram (`InfoBase`). */
export interface InfoBase {
  caminho: string;
  existe: boolean;
  /** Arquivos da base, sem a pasta `.git`. */
  arquivos: number;
  bytes: number;
  git: boolean;
  /** Última atualização do índice, em segundos desde 1970; `null` se ainda não há índice. */
  indiceAtualizadoEm: number | null;
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

/**
 * O que é analisado num projeto cadastrado (#192): as pastas (as que ficam guardam o
 * prefixo; as novas ganham um), as exclusões e as reinclusões.
 */
export interface AjusteLeitura {
  projeto: string;
  /** Caminhos absolutos; a lista vazia é recusada (o projeto precisa de uma pasta). */
  pastas: string[];
  exclusoes: string[];
  inclusoes: string[];
}

export interface AmbienteDesfeito {
  protecaoTirada: string[];
  preparo: PlanoPreparo;
  /** O que ficou e por quê (o que não voltou a ser como antes). */
  avisos: string[];
  /** A configuração do Claude voltou a ser a de antes do preparo. */
  igualAoAntes: boolean;
}

export interface Descadastro {
  ambiente: AmbienteDesfeito;
  /** Pasta do projeto na base, que fica. */
  documentos: string | null;
}

export type GrupoTexto = "template" | "roteiro" | "prompt" | "exportacao" | "regrasDoProjeto";
export type EstadoTexto = "padrao" | "igualAoPadrao" | "editado" | "ilegivel";

export interface TextoEditavel {
  grupo: GrupoTexto;
  id: string;
  rotulo: string;
  /** Relativo à raiz da base. */
  arquivo: string;
  estado: EstadoTexto;
}

/** Variável ou marcador que um texto aceita (`VariavelTexto`). */
export interface VariavelTexto {
  nome: string;
  descricao: string;
}

/** Aviso da validação de um texto (`AvisoTexto`); `linha` conta a partir de 1. */
export interface AvisoTexto {
  linha: number | null;
  variavel: string;
  /** O texto não usa a variável (em vez de usar uma que não existe). */
  ausente: boolean;
  mensagem: string;
}

/** O texto de um editor: o atual, o padrão e o que ele aceita (`LeituraTexto`). */
export interface LeituraTexto {
  grupo: GrupoTexto;
  id: string;
  rotulo: string;
  arquivo: string;
  /** O da base, ou o padrão se não há cópia (ou ela é ilegível). */
  texto: string;
  padrao: string;
  estado: EstadoTexto;
  variaveis: VariavelTexto[];
  /** Hash do conteúdo do arquivo, ou `ausente`; vai de volta em `salvarTexto`. */
  versao: string;
}

/** Prévia da exportação em HTML (`PreviaExportacao`). */
export interface PreviaExportacao {
  /** O arquivo completo, para um `iframe` isolado. */
  html: string;
  avisos: string[];
}

/** Resultado de gravar um texto (`SalvoTexto`). */
export interface SalvoTexto {
  estado: EstadoTexto;
  versao: string;
  avisos: AvisoTexto[];
}

/** Prefixo do erro de `salvarTexto` quando o arquivo mudou fora do app. */
export const CONFLITO = "conflito:";

type Situacao = "ok" | "ausente" | "desatualizado" | "conflito" | "divergente";

/** Detalhe da verificação do ambiente (`cba_session::preparo::Verificacao`). */
export interface VerificacaoAmbiente {
  diretorio: string;
  chave: string;
  servidorMcp: Situacao;
  /** O Claude Code confia na pasta: a primeira conversa abre sem o diálogo (#218). */
  pastaConfiavel: boolean;
  hooks: { evento: string; situacao: Situacao }[];
  hooksDesativados: boolean;
  pronto: boolean;
  /** Workflows das análises por raiz (#370); "divergente" se editado à mão. */
  workflows?: { nome: string; arquivo: string; situacao: Situacao }[];
  permissoesWorkflow?: { regra: string; arquivo: string; presente: boolean }[];
  /** Os workflows estão instalados e liberados: a condução por workflow pode ser oferecida. */
  workflowsProntos?: boolean;
}

export interface AmbienteDetalhado extends AmbienteClaude {
  verificacao?: VerificacaoAmbiente;
}

export interface ApiConfiguracoes
  extends ApiPrimeiroUso,
    ApiRepositorios,
    ApiAcessos,
    ApiAmbienteTeste {
  configuracoes(): Promise<Configuracoes>;
  /** Documentos, tamanho, git e índice da base; `null` sem base escolhida. */
  infoBase(): Promise<InfoBase | null>;
  /** Abre a pasta da base no gerenciador de arquivos do sistema. */
  abrirBase(): Promise<string>;
  verificarAmbiente(projeto: string): Promise<AmbienteDetalhado>;
  avaliarTroca(destino: string): Promise<AvaliacaoTroca>;
  /** Só depois da confirmação do analista. */
  trocarBase(destino: string, modo: ModoTroca): Promise<TrocaBase>;
  /** O inventário com as pastas e as exclusões ajustadas, sem gravar. */
  previaLeitura(ajuste: AjusteLeitura): Promise<PreviaProjeto>;
  /** Grava pastas e exclusões e refaz o inventário; se falhar, nada muda. */
  salvarLeitura(ajuste: AjusteLeitura): Promise<ProjetoCadastrado>;
  /** Liga ou desliga a máscara dos dados pessoais da demanda (#454); devolve o valor gravado. */
  salvarMascararDemanda?(projeto: string, ligado: boolean): Promise<boolean>;
  /** Proteção, servidor MCP e (no último projeto) hooks. Só depois da confirmação. */
  desfazerAmbiente(projeto: string): Promise<AmbienteDesfeito>;
  /** Desfaz o ambiente e apaga o cadastro; os documentos ficam. Só depois da confirmação. */
  descadastrar(projeto: string): Promise<Descadastro>;
  textos(): Promise<TextoEditavel[]>;
  /** Abre no editor do sistema operacional; devolve o caminho. */
  abrirTexto(arquivo: string): Promise<string>;
  lerTexto(arquivo: string): Promise<LeituraTexto>;
  /** Os avisos que o agente receberia, sem gravar. */
  validarTexto(arquivo: string, texto: string): Promise<AvisoTexto[]>;
  /** Recusa com erro iniciado por {@link CONFLITO} se o arquivo mudou desde a leitura. */
  salvarTexto(arquivo: string, texto: string, versaoLida: string): Promise<SalvoTexto>;
  /** Remove a cópia da base (nas regras do projeto, regrava o padrão). */
  restaurarTexto(arquivo: string): Promise<LeituraTexto>;
  /**
   * Prévia da exportação em HTML (#166): um documento de exemplo com o template e os
   * tokens da base, com `texto` no lugar de `arquivo`. Só os arquivos do grupo
   * `exportacao`.
   */
  previaExportacao(arquivo: string, texto: string): Promise<PreviaExportacao>;
  situacaoProtecao(projeto: string): Promise<SituacaoProtecao>;
  mudarProtecao(projeto: string, ligada: boolean): Promise<SituacaoProtecao>;
}

export const apiConfiguracoesTauri: ApiConfiguracoes = {
  ...apiPrimeiroUsoTauri,
  ...apiRepositoriosTauri,
  ...apiAcessosTauri,
  ...apiAmbienteTesteTauri,
  configuracoes: () => invoke("configuracoes_app"),
  infoBase: () => invoke("info_base_app"),
  abrirBase: () => invoke("abrir_base_app"),
  avaliarTroca: (destino) => invoke("avaliar_troca_base_app", { destino }),
  trocarBase: (destino, modo) => invoke("trocar_base_app", { destino, modo }),
  previaLeitura: (ajuste) => invoke("previa_leitura_app", { ajuste }),
  salvarLeitura: (ajuste) => invoke("salvar_leitura_app", { ajuste }),
  salvarMascararDemanda: (projeto, ligado) =>
    invoke("salvar_mascarar_demanda_app", { projeto, ligado }),
  desfazerAmbiente: (projeto) => invoke("desfazer_ambiente_projeto", { projeto }),
  descadastrar: (projeto) => invoke("descadastrar_projeto_app", { projeto }),
  textos: () => invoke("textos_editaveis_app"),
  abrirTexto: (arquivo) => invoke("abrir_texto_editavel", { arquivo }),
  lerTexto: (arquivo) => invoke("ler_texto_editavel", { arquivo }),
  validarTexto: (arquivo, texto) => invoke("validar_texto_editavel", { arquivo, texto }),
  salvarTexto: (arquivo, texto, versaoLida) =>
    invoke("salvar_texto_editavel", { arquivo, texto, versaoLida }),
  restaurarTexto: (arquivo) => invoke("restaurar_texto_padrao", { arquivo }),
  previaExportacao: (arquivo, texto) => invoke("previa_exportacao_texto", { arquivo, texto }),
  situacaoProtecao: (projeto) => invoke("situacao_protecao_projeto", { projeto }),
  mudarProtecao: (projeto, ligada) => invoke("mudar_protecao_projeto", { projeto, ligada }),
  verificarAmbiente: apiAnaliseTauri.verificarAmbiente,
};

/** Como a tela descreve a situação de um texto editável. */
export const ESTADO_DO_TEXTO: Record<EstadoTexto, string> = {
  padrao: "padrão do app",
  igualAoPadrao: "cópia sem mudanças",
  editado: "editado por você",
  ilegivel: "ilegível (o app usa o padrão)",
};
