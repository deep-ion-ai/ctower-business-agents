import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { apiAnaliseTauri, type ApiAmbiente } from "./analise";
import { estadoClientes, type EstadoClientes } from "./clientes";
import type { SistemaResumo } from "./terminal";

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
  sistemas: SistemaResumo[];
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

export interface PedidoSistema {
  nome: string;
  /** Caminhos completos; a primeira pasta é a raiz. */
  pastas: string[];
  exclusoes: string[];
  inclusoes: string[];
}

export interface PastaVista {
  caminho: string;
  prefixo: string | null;
  branch: string | null;
  git: boolean;
}

export interface Contagem {
  chave: string;
  rotulo: string;
  quantidade: number;
}

export interface IgnoradoVista {
  caminho: string;
  pasta: boolean;
  /** segredo, controle_versao, dependencia, saida_build, binario, link_simbolico, ilegivel, usuario. */
  motivo: string;
  rotulo: string;
  regra: string | null;
  /** Pode ser reincluído pelo analista. */
  ajustavel: boolean;
}

export interface ResumoInventario {
  analisados: number;
  bytesAnalisados: number;
  ignorados: number;
  porLinguagem: Contagem[];
  porMotivo: Contagem[];
  /** Os primeiros 500 itens ignorados. */
  itensIgnorados: IgnoradoVista[];
  duracaoMs: number;
}

export interface PreviaSistema {
  id: string;
  nome: string;
  pastas: PastaVista[];
  exclusoes: string[];
  inclusoes: string[];
  jaCadastrado: boolean;
  inventario: ResumoInventario;
}

export interface SistemaCadastrado {
  id: string;
  nome: string;
  pastas: PastaVista[];
  inventario: ResumoInventario;
}

/** Operações do primeiro uso. A interface usa {@link apiPrimeiroUsoTauri}; os testes, uma falsa. */
export interface ApiPrimeiroUso extends ApiAmbiente {
  situacao(): Promise<Situacao>;
  estadoClientes(): Promise<EstadoClientes>;
  escolherCliente(cliente: ClientePreferido): Promise<void>;
  avaliarBase(pasta: string): Promise<AvaliacaoBase>;
  configurarBase(pasta: string): Promise<string>;
  previaSistema(pedido: PedidoSistema): Promise<PreviaSistema>;
  cadastrarSistema(pedido: PedidoSistema): Promise<SistemaCadastrado>;
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
  previaSistema: (pedido) => invoke("previa_sistema_app", { pedido }),
  cadastrarSistema: (pedido) => invoke("cadastrar_sistema_app", { pedido }),
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
