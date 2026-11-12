import { invoke } from "@tauri-apps/api/core";

/**
 * Repositório de código (forja) e de issues (rastreador) de um projeto. Espelha
 * `crates/cba-repos` (nomes de campo em snake_case, como no ctower-agents). O cadastro
 * guarda só o comando de credencial, nunca o token.
 */
export interface GithubConfig {
  /** `dono/repositório`. */
  repo: string;
}

export interface GitlabConfig {
  host: string;
  path: string;
  clone_url: string;
  credential_cmd: string;
}

export type PapelTipoJira = "epic" | "issue" | "bug" | "chore" | "ignore";

export interface JiraConfig {
  url: string;
  project_key: string;
  jql: string;
  active_sprint_only: boolean;
  max_issues: number;
  credential_cmd: string;
  config_file: string;
  epic_link_field: string;
  epic_name_field: string;
  sprint_field: string;
  type_map: Record<string, PapelTipoJira>;
}

export type ForgeConfig = ({ kind: "github" } & GithubConfig) | ({ kind: "gitlab" } & GitlabConfig);

export type TrackerConfig =
  | ({ kind: "github" } & GithubConfig)
  | ({ kind: "gitlab" } & GitlabConfig)
  | ({ kind: "jira" } & JiraConfig);

export interface Repositorios {
  forge: ForgeConfig | null;
  tracker: TrackerConfig | null;
}

export const SEM_REPOSITORIOS: Repositorios = { forge: null, tracker: null };

export const PAPEIS_JIRA: PapelTipoJira[] = ["epic", "issue", "bug", "chore", "ignore"];

export const TIPOS_JIRA_PADRAO: Record<string, PapelTipoJira> = {
  Epic: "epic",
  Épico: "epic",
  Bug: "bug",
  Erro: "bug",
};

export const GITHUB_VAZIO: GithubConfig = { repo: "" };
export const GITLAB_VAZIO: GitlabConfig = {
  host: "",
  path: "",
  clone_url: "",
  credential_cmd: "",
};

/** Projeto novo: só a sprint ativa, com o mapa de tipos padrão. */
export const JIRA_VAZIO: JiraConfig = {
  url: "",
  project_key: "",
  jql: "",
  active_sprint_only: true,
  max_issues: 100,
  credential_cmd: "",
  config_file: "",
  epic_link_field: "",
  epic_name_field: "",
  sprint_field: "",
  type_map: TIPOS_JIRA_PADRAO,
};

export const MAX_ISSUES_TETO = 5000;

/** `Tipo = papel`, uma por linha. */
export function mapaDeTipos(texto: string): Record<string, PapelTipoJira> {
  const mapa: Record<string, PapelTipoJira> = {};
  for (const linha of texto.split("\n")) {
    const [nome, papel] = linha.split("=").map((x) => x.trim());
    if (nome && papel && (PAPEIS_JIRA as string[]).includes(papel)) {
      mapa[nome] = papel as PapelTipoJira;
    }
  }
  return mapa;
}

export function textoDoMapa(mapa: Record<string, PapelTipoJira>): string {
  return Object.entries(mapa)
    .map(([nome, papel]) => `${nome} = ${papel}`)
    .join("\n");
}

const REPO_GITHUB = /^[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+$/;

/** Mesmas regras do cadastro (`cba-repos`), para avisar antes de enviar. `null` = válido. */
export function validarRepositorios(r: Repositorios): string | null {
  const gitlab = (g: GitlabConfig, o: string) => {
    if (!g.host.trim() || !g.path.includes("/")) {
      return `Repositório do GitLab de ${o} inválido: informe o host e o caminho (grupo/projeto/app).`;
    }
    if (!g.credential_cmd.trim()) {
      return "Informe o comando de credencial do GitLab (ex.: pass show gitlab | head -1).";
    }
    return null;
  };
  const github = (g: GithubConfig, o: string) =>
    REPO_GITHUB.test(g.repo.trim())
      ? null
      : `Repositório GitHub de ${o} inválido: use dono/repositório.`;
  if (r.forge) {
    const e = r.forge.kind === "github" ? github(r.forge, "código") : gitlab(r.forge, "código");
    if (e) return e;
  }
  const t = r.tracker;
  if (!t) return null;
  if (t.kind === "github") return github(t, "issues");
  if (t.kind === "gitlab") return gitlab(t, "issues");
  if (!/^https?:\/\/.{3,}/i.test(t.url) || t.url.endsWith("/")) {
    return "URL do Jira inválida: use http(s)://, sem barra no final.";
  }
  if (!/^[A-Z][A-Z0-9_]*$/.test(t.project_key)) {
    return "Chave do projeto Jira inválida: maiúsculas, como PROJ.";
  }
  if (!t.credential_cmd.trim()) {
    return "Informe o comando de credencial do Jira (ex.: pass show jira | head -1).";
  }
  if (!t.active_sprint_only) {
    if (!t.jql.trim()) {
      return "Informe o JQL (ex.: labels = minha-equipe AND updated >= -90d) ou marque “somente a sprint ativa”.";
    }
    if (!Number.isInteger(t.max_issues) || t.max_issues < 1 || t.max_issues > MAX_ISSUES_TETO) {
      return `Limite de issues inválido: de 1 a ${MAX_ISSUES_TETO}.`;
    }
  }
  return null;
}

export type SituacaoPasso = "ok" | "falhou" | "pulado";

export interface PassoProntidao {
  id: string;
  rotulo: string;
  situacao: SituacaoPasso;
  detalhe: string | null;
  orientacao: string | null;
}

export interface IntegracaoProntidao {
  titulo: string;
  passos: PassoProntidao[];
}

export interface Prontidao {
  integracoes: IntegracaoProntidao[];
}

export interface ApiRepositorios {
  salvarRepositorios(projeto: string, repositorios: Repositorios): Promise<Repositorios>;
  prontidaoRepositorios(projeto: string): Promise<Prontidao>;
}

export const apiRepositoriosTauri: ApiRepositorios = {
  salvarRepositorios: (projeto, repositorios) =>
    invoke("salvar_repositorios_app", { projeto, repositorios }),
  prontidaoRepositorios: (projeto) => invoke("prontidao_repositorios_app", { projeto }),
};
