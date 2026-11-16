import type { ApiPrimeiroUso, ApiRepositoriosDoCadastro } from "./primeiroUso";
import {
  GITLAB_VAZIO,
  JIRA_VAZIO,
  REPOSITORIO_CODIGO_VAZIO,
  mesmoRepositorio,
  repositorioDaUrl,
  type ForgeKind,
  type GitlabConfig,
  type JiraConfig,
  type ProgressoClone,
  type RepoDetectado,
  type RepositorioCodigo,
  type TrackerConfig,
} from "./repositorios";

/**
 * Estado e regras do assistente de cadastro de projeto (#180), sem React: o que cada
 * passo guarda, o que se deduz dele (pastas, repositórios, rastreador) e as sugestões.
 */
export type ApiAssistenteProjeto = Pick<
  ApiPrimeiroUso,
  "previaProjeto" | "cadastrarProjeto" | "escolherPasta"
> &
  ApiRepositoriosDoCadastro;

export type OrigemCodigo = "clonar" | "pasta";

export type TipoDemanda = "nenhum" | "github" | "gitlab" | "jira";

export const ROTULO_FORJA: Record<ForgeKind, string> = { github: "GitHub", gitlab: "GitLab" };

/** Um repositório a clonar, como o analista o descreveu no passo 1. */
export interface RepoClone {
  /** Identidade estável na lista (a `key` do React e o alvo dos eventos do clone). */
  chave: number;
  /** `pasta` preenchida = já clonado. */
  repo: RepositorioCodigo;
  /** PAT digitado para o token próprio, guardado no chaveiro antes do clone. */
  token: string;
  progresso: ProgressoClone | null;
}

export interface EstadoCodigo {
  origem: OrigemCodigo;
  /** Nome digitado; vale só se {@link EstadoCodigo.nomeTocado}, senão vale a sugestão. */
  nome: string;
  nomeTocado: boolean;
  /** Pasta existente escolhida. */
  pasta: string | null;
  /** Repositórios git achados na pasta; `null` enquanto procura (ou sem pasta). */
  detectados: RepoDetectado[] | null;
  clones: RepoClone[];
  pastaBase: string;
  /** A pasta dos clones das preferências (para não sugerir o nome por ela). */
  pastaPadrao: string | null;
  /** Lista de clones aberta para edição; senão, o resumo (quando todos foram clonados). */
  editandoClones: boolean;
}

let proximaChave = 1;

export function novoRepoClone(url = ""): RepoClone {
  const lido = url.trim() ? repositorioDaUrl(url) : null;
  return {
    chave: proximaChave++,
    repo: lido ?? { ...REPOSITORIO_CODIGO_VAZIO, url_clone: url.trim(), caminho: "" },
    token: "",
    progresso: null,
  };
}

export function codigoInicial(): EstadoCodigo {
  return {
    origem: "pasta",
    nome: "",
    nomeTocado: false,
    pasta: null,
    detectados: null,
    clones: [novoRepoClone()],
    pastaBase: "",
    pastaPadrao: null,
    editandoClones: true,
  };
}

/** Último trecho de um caminho (`/a/b/` → `b`), com `/` ou `\`. */
export function nomeDaPasta(caminho: string): string {
  const partes = caminho.trim().split(/[\\/]+/).filter(Boolean);
  return partes[partes.length - 1] ?? "";
}

/** Nome legível a partir do nome de uma pasta: `faturamento-exemplo` → `Faturamento Exemplo`. */
export function nomeSugerido(pasta: string): string {
  return pasta
    .split(/[-_.\s]+/)
    .filter(Boolean)
    .map((p) => p.charAt(0).toUpperCase() + p.slice(1))
    .join(" ");
}

/** Subpasta onde o repositório é clonado: o último trecho do caminho (`dono/app` → `app`). */
export function pastaDoClone(r: Pick<RepositorioCodigo, "caminho">): string {
  return nomeDaPasta(r.caminho);
}

/** Repositórios com URL preenchida (os cartões vazios não contam). */
export function clonesPreenchidos(e: EstadoCodigo): RepoClone[] {
  return e.clones.filter((c) => c.repo.url_clone.trim() !== "");
}

export function todosClonados(e: EstadoCodigo): boolean {
  const lista = clonesPreenchidos(e);
  return lista.length > 0 && lista.every((c) => c.repo.pasta !== null);
}

/** Repositórios git achados numa pasta existente, já no formato do cadastro (sem repetidos). */
export function repositoriosDetectados(lista: RepoDetectado[]): RepositorioCodigo[] {
  const codigo: RepositorioCodigo[] = [];
  for (const d of lista) {
    if (!d.forja || !d.host || !d.caminhoRepo) continue;
    const r: RepositorioCodigo = {
      ...REPOSITORIO_CODIGO_VAZIO,
      forja: d.forja,
      host: d.host,
      caminho: d.caminhoRepo,
      url_clone: d.remoto ?? "",
      pasta: d.caminho,
      token: "global",
    };
    if (!codigo.some((c) => mesmoRepositorio(c, r))) codigo.push(r);
  }
  return codigo;
}

/** As pastas do projeto: a escolhida ou, no clone, as pastas clonadas (uma por repositório). */
export function pastasDoProjeto(e: EstadoCodigo): string[] {
  if (e.origem === "pasta") return e.pasta ? [e.pasta] : [];
  return clonesPreenchidos(e).flatMap((c) => (c.repo.pasta ? [c.repo.pasta] : []));
}

/** Os repositórios de código do projeto. */
export function codigoDoProjeto(e: EstadoCodigo): RepositorioCodigo[] {
  if (e.origem === "pasta") return repositoriosDetectados(e.detectados ?? []);
  return clonesPreenchidos(e).map((c) => c.repo);
}

/**
 * Nome sugerido: o da pasta escolhida; no clone, o da pasta dos clones quando o analista
 * a mudou para o projeto (a padrão das preferências serve a todos os projetos) ou, senão,
 * o do primeiro repositório.
 */
export function sugestaoDeNome(e: EstadoCodigo): string {
  if (e.origem === "pasta") return e.pasta ? nomeSugerido(nomeDaPasta(e.pasta)) : "";
  const base = e.pastaBase.trim();
  if (base && base !== (e.pastaPadrao ?? "").trim()) return nomeSugerido(nomeDaPasta(base));
  const primeiro = clonesPreenchidos(e).find((c) => c.repo.caminho);
  return primeiro ? nomeSugerido(pastaDoClone(primeiro.repo)) : "";
}

export function nomeDoProjeto(e: EstadoCodigo): string {
  return (e.nomeTocado ? e.nome : sugestaoDeNome(e)).trim();
}

/** Hosts distintos dos repositórios (em minúsculas, na ordem em que aparecem). */
export function hostsDistintos(repos: Pick<RepositorioCodigo, "host">[]): string[] {
  const hosts: string[] = [];
  for (const r of repos) {
    const h = r.host.trim().toLowerCase();
    if (h && !hosts.includes(h)) hosts.push(h);
  }
  return hosts;
}

/** `GitHub · dono/repo`. */
export function rotuloRepositorio(r: Pick<RepositorioCodigo, "forja" | "caminho">): string {
  return `${ROTULO_FORJA[r.forja]} · ${r.caminho}`;
}

/** Valor da opção de um repositório do passo 1 na lista de repositórios das issues. */
export function chaveRepositorio(r: Pick<RepositorioCodigo, "host" | "caminho">): string {
  return `${r.host.trim().toLowerCase()}/${r.caminho.trim()}`;
}

export const OUTRO_REPOSITORIO = "outro";

export interface EstadoDemandas {
  tipo: TipoDemanda;
  /** {@link chaveRepositorio} de um repositório do passo 1, ou {@link OUTRO_REPOSITORIO}. */
  repoIssues: string;
  outroGithub: string;
  outroGitlab: GitlabConfig;
  jira: JiraConfig;
  /** Token do Jira digitado; vai para o chaveiro ao cadastrar. */
  tokenJira: string;
  /** Há token do host do Jira no chaveiro (`null` = não se sabe). */
  jiraGuardado: boolean | null;
}

export const DEMANDAS_INICIAIS: EstadoDemandas = {
  tipo: "nenhum",
  repoIssues: OUTRO_REPOSITORIO,
  outroGithub: "",
  outroGitlab: GITLAB_VAZIO,
  jira: JIRA_VAZIO,
  tokenJira: "",
  jiraGuardado: null,
};

/** O repositório escolhido para as issues (`null` = outro, informado à mão). */
export function repositorioDasIssues(
  d: EstadoDemandas,
  codigo: RepositorioCodigo[],
): RepositorioCodigo | null {
  if (d.repoIssues === OUTRO_REPOSITORIO) return null;
  return codigo.find((r) => chaveRepositorio(r) === d.repoIssues) ?? null;
}

/**
 * O rastreador de issues do cadastro. GitHub e GitLab de um repositório do passo 1 usam
 * o mesmo token dele (o comando de credencial vazio = o token do host no chaveiro).
 */
export function trackerDoProjeto(
  d: EstadoDemandas,
  codigo: RepositorioCodigo[],
): TrackerConfig | null {
  if (d.tipo === "nenhum") return null;
  if (d.tipo === "jira") return { kind: "jira", ...d.jira, url: d.jira.url.trim() };
  const r = repositorioDasIssues(d, codigo);
  if (d.tipo === "github") {
    return { kind: "github", repo: r && r.forja === "github" ? r.caminho : d.outroGithub.trim() };
  }
  if (r && r.forja === "gitlab") {
    return { kind: "gitlab", host: r.host, path: r.caminho, clone_url: r.url_clone, credential_cmd: "" };
  }
  const g = d.outroGitlab;
  const host = g.host.trim();
  const path = g.path.trim();
  return {
    kind: "gitlab",
    host,
    path,
    clone_url: g.clone_url.trim() || (host && path ? `https://${host}/${path}.git` : ""),
    credential_cmd: g.credential_cmd.trim(),
  };
}

/** Linhas não vazias (padrões de exclusão, URLs coladas). */
export function linhas(texto: string): string[] {
  return texto
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean);
}

/** `1.284` (pt-BR). */
export function numero(n: number): string {
  return n.toLocaleString("pt-BR");
}

/** `1 repositório`, `2 repositórios`. */
export function plural(n: number, um: string, varios: string): string {
  return `${numero(n)} ${n === 1 ? um : varios}`;
}
