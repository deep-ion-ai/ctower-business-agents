import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { apiAnaliseTauri, type ApiAmbiente } from "./analise";
import { estadoClientes, type EstadoClientes } from "./clientes";
import {
  apiRepositoriosTauri,
  type ApiRepositorios,
  type ForgeKind,
  type Repositorios,
} from "./repositorios";
import type { ProjetoResumo } from "./terminal";

/**
 * Espelho dos comandos do primeiro uso (`ui/src-tauri/src/primeiro_uso.rs`, campos em
 * camelCase). Os componentes do assistente usam só {@link ApiPrimeiroUso}, para a tela
 * de configurações (#39) poder reaproveitá-los.
 */
export type ClientePreferido = "terminalEmbutido" | "claudeDesktop";

export interface Situacao {
  base: string | null;
  /** `Documentos/ctower-business`, quando o sistema operacional informa a pasta. */
  baseSugerida: string | null;
  projetos: ProjetoResumo[];
  cliente: ClientePreferido | null;
  primeiroUsoConcluido: boolean;
}

export type EstadoPasta = "nova" | "vazia" | "base" | "comArquivos";

export interface AvaliacaoBase {
  caminho: string;
  /** `null` quando o caminho existe e não é pasta. */
  estado: EstadoPasta | null;
  /** Por que a pasta não pode ser usada. */
  recusa: string | null;
  git: boolean;
}

export interface PedidoProjeto {
  nome: string;
  /** Caminhos completos; a primeira pasta é a raiz. */
  pastas: string[];
  exclusoes: string[];
  inclusoes: string[];
  /** Repositório de código e de issues (opcional). */
  repositorios?: Repositorios;
}

export interface PastaVista {
  caminho: string;
  prefixo: string | null;
  branch: string | null;
  git: boolean;
  /** Remoto do repositório git da pasta e o que se deduz dele (issue #178). */
  remoto?: string | null;
  forja?: ForgeKind | null;
  host?: string | null;
  caminhoRepo?: string | null;
}

export interface Contagem {
  chave: string;
  rotulo: string;
  quantidade: number;
}

export interface IgnoradoVista {
  caminho: string;
  pasta: boolean;
  /** Arquivos que o item cobre (numa pasta, os que há dentro dela). */
  arquivos: number;
  /** segredo, controle_versao, dependencia, saida_build, binario, link_simbolico, ilegivel, usuario. */
  motivo: string;
  rotulo: string;
  regra: string | null;
  /** Pode ser reincluído pelo analista. */
  ajustavel: boolean;
}

/** Itens fora, agrupados pela pasta em que estão e pelo tipo (a extensão). */
export interface GrupoIgnorado {
  /** Pasta que contém os itens (vazio = a raiz do projeto). */
  pasta: string;
  /** Extensão (`png`), `sem extensão` ou `pasta` (pastas inteiras que ficam fora). */
  tipo: string;
  motivo: string;
  rotulo: string;
  /** Itens do grupo (uma pasta ignorada é um item). */
  itens: number;
  /** Arquivos que o grupo cobre. */
  arquivos: number;
  /** Até três caminhos do grupo. */
  exemplos: string[];
  /** Padrões que reincluem o grupo todo; vazio quando não dá para reincluir. */
  padroes: string[];
  ajustavel: boolean;
}

/** O efeito de uma reinclusão: arquivos que entram e arquivos que continuam fora. */
export interface InclusaoVista {
  padrao: string;
  entram: number;
  continuamFora: number;
}

/** As contagens são sempre em arquivos: uma pasta que fica fora conta os arquivos dela. */
export interface ResumoInventario {
  analisados: number;
  bytesAnalisados: number;
  ignorados: number;
  /** Itens ignorados (uma pasta conta 1), para o "e mais N itens" de {@link itensIgnorados}. */
  totalItensIgnorados: number;
  /** Todos os itens ignorados, por pasta e tipo. */
  gruposIgnorados: GrupoIgnorado[];
  porLinguagem: Contagem[];
  porMotivo: Contagem[];
  /** Os primeiros 500 itens ignorados (e todos os possíveis segredos). */
  itensIgnorados: IgnoradoVista[];
  duracaoMs: number;
}

export interface PreviaProjeto {
  id: string;
  nome: string;
  pastas: PastaVista[];
  exclusoes: string[];
  inclusoes: string[];
  /** O que cada reinclusão faz entrar (ou não) na análise. */
  inclusoesVistas: InclusaoVista[];
  jaCadastrado: boolean;
  inventario: ResumoInventario;
}

export interface ProjetoCadastrado {
  id: string;
  nome: string;
  pastas: PastaVista[];
  inventario: ResumoInventario;
}

/** O que o cadastro do projeto (assistente, #180) usa dos repositórios: clone, tokens e testes. */
export type ApiRepositoriosDoCadastro = Pick<
  ApiRepositorios,
  | "pastaClones"
  | "salvarToken"
  | "tokensGuardados"
  | "testarAcesso"
  | "clonarRepositorios"
  | "cancelarClone"
  | "detectarRepositorios"
  | "testarJira"
  | "testarDemandas"
>;

/** Operações do primeiro uso. A interface usa {@link apiPrimeiroUsoTauri}; os testes, uma falsa. */
export interface ApiPrimeiroUso extends ApiAmbiente, ApiRepositoriosDoCadastro {
  situacao(): Promise<Situacao>;
  estadoClientes(): Promise<EstadoClientes>;
  escolherCliente(cliente: ClientePreferido): Promise<void>;
  avaliarBase(pasta: string): Promise<AvaliacaoBase>;
  configurarBase(pasta: string): Promise<string>;
  previaProjeto(pedido: PedidoProjeto): Promise<PreviaProjeto>;
  /** Já existe um projeto cadastrado com este nome? */
  nomeEmUso(nome: string): Promise<boolean>;
  cadastrarProjeto(pedido: PedidoProjeto): Promise<ProjetoCadastrado>;
  concluir(): Promise<void>;
  /** Abre um link oficial no navegador. */
  abrirLink(url: string): Promise<void>;
  /** Seletor de pasta do sistema operacional; `null` se o analista cancelar. */
  escolherPasta(titulo: string, inicial?: string): Promise<string | null>;
}

export const apiPrimeiroUsoTauri: ApiPrimeiroUso = {
  situacao: () => invoke("situacao_app"),
  estadoClientes,
  escolherCliente: (cliente) => invoke("escolher_cliente_app", { cliente }),
  avaliarBase: (pasta) => invoke("avaliar_base_app", { pasta }),
  configurarBase: (pasta) => invoke("configurar_base_app", { pasta }),
  previaProjeto: (pedido) => invoke("previa_projeto_app", { pedido }),
  nomeEmUso: (nome) => invoke("nome_projeto_em_uso_app", { nome }),
  cadastrarProjeto: (pedido) => invoke("cadastrar_projeto_app", { pedido }),
  concluir: () => invoke("concluir_primeiro_uso_app"),
  abrirLink: (url) => openUrl(url),
  async escolherPasta(titulo, inicial) {
    const escolhida = await open({
      directory: true,
      multiple: false,
      title: titulo,
      defaultPath: inicial,
    });
    return typeof escolhida === "string" ? escolhida : null;
  },
  verificarAmbiente: apiAnaliseTauri.verificarAmbiente,
  planejarPreparo: apiAnaliseTauri.planejarPreparo,
  prepararAmbiente: apiAnaliseTauri.prepararAmbiente,
  pastaClones: apiRepositoriosTauri.pastaClones,
  salvarToken: apiRepositoriosTauri.salvarToken,
  tokensGuardados: apiRepositoriosTauri.tokensGuardados,
  testarAcesso: apiRepositoriosTauri.testarAcesso,
  clonarRepositorios: apiRepositoriosTauri.clonarRepositorios,
  cancelarClone: apiRepositoriosTauri.cancelarClone,
  detectarRepositorios: apiRepositoriosTauri.detectarRepositorios,
  testarJira: apiRepositoriosTauri.testarJira,
  testarDemandas: apiRepositoriosTauri.testarDemandas,
};

/**
 * Padrão que reinclui exatamente um caminho do inventário: ancorado na raiz (`/`) e
 * com os curingas do glob escapados (`[*]`).
 */
export function padraoDoCaminho(caminho: string): string {
  return "/" + caminho.replace(/[*?[\]{}]/g, (c) => `[${c}]`);
}

/** Tamanho legível (`1,2 MB`). */
export function tamanhoLegivel(bytes: number): string {
  const unidades = ["B", "KB", "MB", "GB"];
  let valor = bytes;
  let i = 0;
  while (valor >= 1024 && i < unidades.length - 1) {
    valor /= 1024;
    i++;
  }
  const texto =
    i === 0 ? String(valor) : valor.toLocaleString("pt-BR", { maximumFractionDigits: 1 });
  return `${texto} ${unidades[i]}`;
}
