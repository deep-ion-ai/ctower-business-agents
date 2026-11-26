import { invoke } from "@tauri-apps/api/core";
import type {
  ForgeKind,
  IntegracaoProntidao,
  JiraConfig,
  RepositorioCodigo,
  TrackerConfig,
} from "./repositorios";

/**
 * Conexões externas (#437): GitHub, GitLab, Jira e bancos cadastrados uma vez nas
 * Configurações e referenciados pelos projetos pelo `id`. Espelha
 * `ui/src-tauri/src/conexoes.rs` e `cba_repos::providers`.
 *
 * A tela não conhece os tipos: os descritores ({@link Descritor}) vêm do backend com o
 * rótulo, o ícone, as capacidades, as fontes de credencial aceitas e os campos do
 * formulário. Nada aqui traz uma credencial: só se há uma guardada.
 */

export type Capacidade = "rastreador" | "forja" | "banco";
export type TipoFonte = "chaveiro" | "comando" | "cli" | "arquivo" | "nenhuma";

/** De onde vem a credencial; nunca o valor. */
export type FonteCredencial =
  | { fonte: "chaveiro"; conta: string }
  | { fonte: "comando"; cmd: string }
  | { fonte: "cli" }
  | { fonte: "arquivo"; caminho: string }
  | { fonte: "nenhuma" };

export interface Conexao {
  /** Vazio numa conexão nova: o app gera a partir do nome. */
  id: string;
  nome: string;
  tipo: string;
  endereco: string;
  credencial: FonteCredencial;
  opcoes?: Record<string, string>;
}

export type TipoCampo =
  | { tipo: "texto" }
  | { tipo: "numero"; min: number; max: number }
  | { tipo: "escolha"; opcoes: { valor: string; rotulo: string }[] }
  | { tipo: "booleano" };

export type CampoOpcao = TipoCampo & {
  chave: string;
  rotulo: string;
  ajuda: string;
  padrao: string | null;
  obrigatorio: boolean;
};

export interface PassoDeclarado {
  id: string;
  rotulo: string;
  orientacao: string;
}

/** O descritor de um tipo de conexão, como o backend o descreve para a tela. */
export interface Descritor {
  tipo: string;
  rotulo: string;
  icone: string;
  capacidades: Capacidade[];
  endereco_padrao: string;
  formato_endereco: string;
  fontes_aceitas: { fonte: TipoFonte; rotulo: string }[];
  fonte_padrao: TipoFonte;
  campos_opcoes: CampoOpcao[];
  passos_prontidao: PassoDeclarado[];
}

export interface UsoConexao {
  projeto: string;
  nomeProjeto: string;
  /** `issues`, `código g/p` ou `documentação`. */
  onde: string;
}

export interface UltimoTeste {
  pronta: boolean;
  quandoMs: number;
  resumo: string;
}

export interface ConexaoVista {
  conexao: Conexao;
  rotuloTipo: string;
  icone: string;
  /** Com a fonte `chaveiro`: se há credencial guardada. `null` nas outras fontes. */
  credencialGuardada: boolean | null;
  usos: UsoConexao[];
  ultimoTeste: UltimoTeste | null;
}

export interface ApiConexoes {
  descritoresConexao(): Promise<Descritor[]>;
  listarConexoes(): Promise<ConexaoVista[]>;
  /** Cria (sem `id`) ou edita; devolve a conexão gravada, com o `id`. */
  salvarConexao(conexao: Conexao): Promise<Conexao>;
  /** Recusa a conexão em uso, dizendo quais projetos a usam. */
  removerConexao(id: string): Promise<void>;
  /** Guarda (ou, vazio, apaga) a credencial no chaveiro; o valor não volta. */
  guardarCredencialConexao(id: string, valor: string): Promise<void>;
  /** Os passos de prontidão do descritor, com a orientação de quem falhou. */
  testarConexao(id: string): Promise<IntegracaoProntidao>;
}

export const apiConexoesTauri: ApiConexoes = {
  descritoresConexao: () => invoke("descritores_conexao_app"),
  listarConexoes: () => invoke("listar_conexoes_app"),
  salvarConexao: (conexao) => invoke("salvar_conexao_app", { conexao }),
  removerConexao: (id) => invoke("remover_conexao_app", { id }),
  guardarCredencialConexao: (id, valor) =>
    invoke("guardar_credencial_conexao_app", { id, valor }),
  testarConexao: (id) => invoke("testar_conexao_app", { id }),
};

/** Os descritores que servem para um lugar (`forja`, `rastreador`). */
export function descritoresCom(lista: Descritor[], capacidade: Capacidade): Descritor[] {
  return lista.filter((d) => d.capacidades.includes(capacidade));
}

/** As conexões de um tipo ou, sem `tipo`, de qualquer tipo com a capacidade. */
export function conexoesPara(
  lista: ConexaoVista[],
  descritores: Descritor[],
  capacidade: Capacidade,
  tipo?: string,
): ConexaoVista[] {
  const tipos = new Set(descritoresCom(descritores, capacidade).map((d) => d.tipo));
  return lista.filter(
    (c) => tipos.has(c.conexao.tipo) && (tipo === undefined || c.conexao.tipo === tipo),
  );
}

/** O host de um endereço (`https://Jira.example.com:8443/x` → `jira.example.com`). */
export function hostDoEndereco(endereco: string): string {
  const semEsquema = endereco.trim().replace(/^[a-z][a-z0-9+.-]*:\/\//i, "");
  const autoridade = semEsquema.split("/")[0] ?? "";
  const semUsuario = autoridade.includes("@")
    ? autoridade.slice(autoridade.lastIndexOf("@") + 1)
    : autoridade;
  return (semUsuario.split(":")[0] ?? "").toLowerCase();
}

/** Uma conexão nova do tipo, com o endereço e a fonte padrão do descritor. */
export function conexaoNova(d: Descritor): Conexao {
  return {
    id: "",
    nome: "",
    tipo: d.tipo,
    endereco: d.endereco_padrao,
    credencial: fonteVazia(d.fonte_padrao),
    opcoes: {},
  };
}

/** A fonte do tipo, com os campos vazios (a conta do chaveiro o app preenche). */
export function fonteVazia(fonte: TipoFonte): FonteCredencial {
  switch (fonte) {
    case "chaveiro":
      return { fonte, conta: "" };
    case "comando":
      return { fonte, cmd: "" };
    case "arquivo":
      return { fonte, caminho: "" };
    default:
      return { fonte };
  }
}

/** A fonte da credencial em palavras, sem o valor. */
export function descreverFonte(c: FonteCredencial, descritores: Descritor[], tipo: string): string {
  const rotulo =
    descritores
      .find((d) => d.tipo === tipo)
      ?.fontes_aceitas.find((f) => f.fonte === c.fonte)?.rotulo ?? c.fonte;
  switch (c.fonte) {
    case "comando":
      return `${rotulo}: ${c.cmd}`;
    case "arquivo":
      return `${rotulo}: ${c.caminho}`;
    default:
      return rotulo;
  }
}

/** “Faturamento Exemplo (issues), Cobrança Exemplo (código g/cob)”. */
export function descreverUsos(usos: UsoConexao[]): string {
  return usos.map((u) => `${u.nomeProjeto} (${u.onde})`).join(", ");
}

/**
 * O repositório de código apontando para a conexão: a forja e o host vêm dela, e o
 * comando de credencial sai. O token próprio do repositório continua valendo como
 * sobrescrita. Sem conexão (`null`), o cadastro volta a usar o token do host.
 */
export function repositorioComConexao(
  r: RepositorioCodigo,
  c: Conexao | null,
): RepositorioCodigo {
  if (!c) return { ...r, conexao: "" };
  return {
    ...r,
    conexao: c.id,
    forja: c.tipo as ForgeKind,
    host: hostDoEndereco(c.endereco),
    token: r.token === "proprio" ? "proprio" : "global",
    comando_credencial: "",
  };
}

/**
 * O rastreador apontando para a conexão: o endereço (host ou URL) e o arquivo do
 * jira-cli vêm dela, e o comando de credencial sai. Os campos do projeto (chave, JQL,
 * caminho, campos) ficam.
 */
export function rastreadorComConexao(t: TrackerConfig, c: Conexao | null): TrackerConfig {
  if (!c) return { ...t, conexao: "" };
  const novo: TrackerConfig = { ...t, conexao: c.id };
  if ("url" in novo) novo.url = c.endereco;
  if ("host" in novo) novo.host = hostDoEndereco(c.endereco);
  if ("credential_cmd" in novo) novo.credential_cmd = "";
  if ("config_file" in novo) novo.config_file = c.opcoes?.config_file ?? "";
  return novo;
}

/** O cadastro do Jira apontando para a conexão (o mesmo de {@link rastreadorComConexao}). */
export function jiraComConexao(j: JiraConfig, c: Conexao | null): JiraConfig {
  if (!c) return { ...j, conexao: "" };
  return {
    ...j,
    conexao: c.id,
    url: c.endereco,
    credential_cmd: "",
    config_file: c.opcoes?.config_file ?? "",
  };
}
