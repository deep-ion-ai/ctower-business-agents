import { Channel, invoke } from "@tauri-apps/api/core";

/**
 * Repositórios de código e de issues (rastreador) de um projeto. Espelha
 * `crates/cba-repos` (nomes de campo em snake_case, como no ctower-agents). O cadastro
 * guarda só o comando de credencial, nunca o token: o token fica no chaveiro do sistema.
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

export type ForgeKind = "github" | "gitlab";

/**
 * De onde vem o token do repositório de código: `global` = o token do host (no GitHub,
 * hoje, o `gh auth`); `proprio` = um token só deste repositório; `comando` = um comando
 * que imprime o token (`comando_credencial`).
 */
export type OrigemToken = "global" | "proprio" | "comando";

export interface RepositorioCodigo {
  forja: ForgeKind;
  /** `github.com` no GitHub. */
  host: string;
  /** `dono/repo` ou `grupo/sub/app`. */
  caminho: string;
  /** Como foi colada (HTTPS ou SSH). */
  url_clone: string;
  /** Pasta local deste repositório (absoluta); `null` = o projeto todo (cadastro antigo). */
  pasta: string | null;
  /** Vazio = branch padrão. */
  branch: string;
  token: OrigemToken;
  /** Usado quando `token === "comando"`. */
  comando_credencial: string;
}

export type TrackerConfig =
  | ({ kind: "github" } & GithubConfig)
  | ({ kind: "gitlab" } & GitlabConfig)
  | ({ kind: "jira" } & JiraConfig);

export interface Repositorios {
  codigo: RepositorioCodigo[];
  tracker: TrackerConfig | null;
}

export const SEM_REPOSITORIOS: Repositorios = { codigo: [], tracker: null };

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

export const REPOSITORIO_CODIGO_VAZIO: RepositorioCodigo = {
  forja: "github",
  host: "github.com",
  caminho: "",
  url_clone: "",
  pasta: null,
  branch: "",
  token: "global",
  comando_credencial: "",
};

/**
 * Deduz forja, host e caminho de uma URL de clone: `https://host/a/b(.git)`,
 * `ssh://[usuário@]host[:porta]/a/b(.git)` ou `git@host:a/b.git`. GitHub só em
 * `github.com` (ou `ssh.github.com`); qualquer outro host é GitLab. `null` se não der.
 */
export function repositorioDaUrl(url: string): RepositorioCodigo | null {
  const u = url.trim();
  let host: string;
  let caminho: string;
  const comEsquema = /^(https?|ssh|git):\/\/(?:[^@/]+@)?([^/:]+)(?::\d+)?\/(.+)$/i.exec(u);
  const scp = /^(?:[^@/\s]+@)?([^/:\s]+):(?!\/\/)(.+)$/.exec(u);
  if (comEsquema) {
    host = comEsquema[2]!;
    caminho = comEsquema[3]!;
  } else if (scp) {
    host = scp[1]!;
    caminho = scp[2]!;
  } else {
    return null;
  }
  caminho = caminho
    .replace(/[?#].*$/, "")
    .replace(/^\/+|\/+$/g, "")
    .replace(/\.git$/i, "")
    .replace(/\/+$/, "");
  host = host.toLowerCase();
  if (!host || !caminho.includes("/") || /\s/.test(caminho)) return null;
  const github = host === "github.com" || host === "ssh.github.com";
  return {
    ...REPOSITORIO_CODIGO_VAZIO,
    forja: github ? "github" : "gitlab",
    host: github ? "github.com" : host,
    caminho,
    url_clone: u,
  };
}

/** Mesmo repositório de código: host e caminho, sem diferenciar maiúsculas. */
export function mesmoRepositorio(
  a: Pick<RepositorioCodigo, "host" | "caminho">,
  b: Pick<RepositorioCodigo, "host" | "caminho">,
): boolean {
  return (
    a.host.trim().toLowerCase() === b.host.trim().toLowerCase() &&
    a.caminho.trim().toLowerCase() === b.caminho.trim().toLowerCase()
  );
}

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
    // Sem comando de credencial, vale o token global do host guardado no chaveiro.
    return null;
  };
  const github = (g: GithubConfig, o: string) =>
    REPO_GITHUB.test(g.repo.trim())
      ? null
      : `Repositório GitHub de ${o} inválido: use dono/repositório.`;
  for (const [i, c] of r.codigo.entries()) {
    const o = `Repositório de código ${i + 1}`;
    if (!c.host.trim()) return `${o}: informe o host.`;
    if (c.forja === "github" && !REPO_GITHUB.test(c.caminho.trim())) {
      return `${o}: caminho do GitHub inválido, use dono/repositório.`;
    }
    if (c.forja === "gitlab" && !c.caminho.includes("/")) {
      return `${o}: caminho do GitLab inválido, use grupo/projeto/app.`;
    }
    if (c.token === "comando" && !c.comando_credencial.trim()) {
      return `${o}: informe o comando que imprime o token (ex.: pass show gitlab | head -1).`;
    }
    const j = r.codigo.findIndex((d) => mesmoRepositorio(c, d));
    if (j < i) {
      return `${o}: repete o repositório de código ${j + 1} (${c.host}/${c.caminho}).`;
    }
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

/** Repositório git achado numa pasta (visão do backend, em camelCase). */
export interface RepoDetectado {
  caminho: string;
  branch: string | null;
  remoto: string | null;
  forja: ForgeKind | null;
  host: string | null;
  caminhoRepo: string | null;
}

/**
 * Onde vale um token no chaveiro: o host inteiro (`caminho` nulo ou vazio, o token
 * global) ou só um repositório (o token próprio, que tem precedência).
 */
export interface AlvoToken {
  host: string;
  caminho?: string | null;
}

/** O alvo do token de um repositório de código, conforme a origem escolhida. */
export function alvoDoToken(r: Pick<RepositorioCodigo, "host" | "caminho" | "token">): AlvoToken {
  return r.token === "proprio"
    ? { host: r.host.trim(), caminho: r.caminho.trim() }
    : { host: r.host.trim(), caminho: null };
}

/** Evento do clone, por repositório (`indice` na lista pedida). */
export type EventoClone =
  | { tipo: "inicio"; indice: number }
  | { tipo: "progresso"; indice: number; etapa: string; percentual: number | null }
  | { tipo: "concluido"; indice: number; pasta: string }
  | { tipo: "falhou"; indice: number; erro: string };

/** Resultado do clone de um repositório: com a `pasta` preenchida, ou o erro. */
export interface ResultadoClone {
  repositorio: RepositorioCodigo;
  erro: string | null;
}

export type SituacaoClone = "aguardando" | "clonando" | "concluido" | "falhou";

/** Progresso do clone de um repositório, montado a partir dos {@link EventoClone}. */
export interface ProgressoClone {
  situacao: SituacaoClone;
  etapa: string | null;
  percentual: number | null;
  pasta: string | null;
  erro: string | null;
}

export const PROGRESSO_CLONE_INICIAL: ProgressoClone = {
  situacao: "aguardando",
  etapa: null,
  percentual: null,
  pasta: null,
  erro: null,
};

/** Um progresso “aguardando” por repositório pedido. */
export function progressoInicialClone(quantos: number): ProgressoClone[] {
  return Array.from({ length: quantos }, () => ({ ...PROGRESSO_CLONE_INICIAL }));
}

/**
 * Aplica um evento do clone ao progresso por repositório, sem mudar o original. Um
 * índice além da lista a estende com repositórios “aguardando”.
 */
export function reduzirProgressoClone(
  estado: ProgressoClone[],
  e: EventoClone,
): ProgressoClone[] {
  if (!Number.isInteger(e.indice) || e.indice < 0) return estado;
  const novo = [...estado];
  while (novo.length <= e.indice) novo.push({ ...PROGRESSO_CLONE_INICIAL });
  const atual = novo[e.indice]!;
  switch (e.tipo) {
    case "inicio":
      novo[e.indice] = { ...PROGRESSO_CLONE_INICIAL, situacao: "clonando" };
      break;
    case "progresso":
      novo[e.indice] = {
        ...atual,
        situacao: "clonando",
        etapa: e.etapa,
        percentual: e.percentual,
      };
      break;
    case "concluido":
      novo[e.indice] = { ...atual, situacao: "concluido", percentual: 100, pasta: e.pasta };
      break;
    case "falhou":
      novo[e.indice] = { ...atual, situacao: "falhou", erro: e.erro };
      break;
  }
  return novo;
}

export interface ApiRepositorios {
  salvarRepositorios(projeto: string, repositorios: Repositorios): Promise<Repositorios>;
  prontidaoRepositorios(projeto: string): Promise<Prontidao>;
  detectarRepositorios(pasta: string): Promise<RepoDetectado[]>;
  /** Pasta base dos clones: a das preferências ou a sugerida; `null` se não houver. */
  pastaClones(): Promise<string | null>;
  /** Grava a pasta base dos clones (caminho completo); devolve a gravada. */
  definirPastaClones(pasta: string): Promise<string>;
  /** Grava o token no chaveiro do sistema (nunca no projeto). */
  salvarToken(alvo: AlvoToken, token: string): Promise<void>;
  apagarToken(alvo: AlvoToken): Promise<void>;
  /** Para cada alvo, se há token guardado (nunca o token). */
  tokensGuardados(alvos: AlvoToken[]): Promise<boolean[]>;
  /**
   * Testa o acesso de leitura ao repositório. Com `token` (digitado e ainda não
   * guardado), usa ele; senão, o guardado (próprio, global do host ou comando).
   * Recusa com a mensagem do erro.
   */
  testarAcesso(repositorio: RepositorioCodigo, token?: string | null): Promise<void>;
  /** Clona cada repositório em `pastaBase`, avisando o progresso; a falha de um não para os outros. */
  clonarRepositorios(
    repositorios: RepositorioCodigo[],
    pastaBase: string,
    aoEvento: (e: EventoClone) => void,
  ): Promise<ResultadoClone[]>;
}

export const apiRepositoriosTauri: ApiRepositorios = {
  salvarRepositorios: (projeto, repositorios) =>
    invoke("salvar_repositorios_app", { projeto, repositorios }),
  prontidaoRepositorios: (projeto) => invoke("prontidao_repositorios_app", { projeto }),
  detectarRepositorios: (pasta) => invoke("detectar_repositorios_app", { pasta }),
  pastaClones: () => invoke("pasta_clones_app"),
  definirPastaClones: (pasta) => invoke("definir_pasta_clones_app", { pasta }),
  salvarToken: (alvo, token) => invoke("salvar_token_app", { alvo, token }),
  apagarToken: (alvo) => invoke("apagar_token_app", { alvo }),
  tokensGuardados: (alvos) => invoke("tokens_guardados_app", { alvos }),
  testarAcesso: (repositorio, token) =>
    invoke("testar_acesso_app", { repositorio, token: token ?? null }),
  clonarRepositorios(repositorios, pastaBase, aoEvento) {
    const canal = new Channel<EventoClone>();
    canal.onmessage = aoEvento;
    // O canal não é desligado ao fim: um evento pode chegar depois da resposta.
    return invoke<ResultadoClone[]>("clonar_repositorios_app", { repositorios, pastaBase, canal });
  },
};
