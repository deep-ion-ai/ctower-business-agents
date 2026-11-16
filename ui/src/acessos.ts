import { invoke } from "@tauri-apps/api/core";
import type { Configuracoes } from "./configuracoes";
import type { AlvoToken, ForgeKind, RepositorioCodigo } from "./repositorios";

/**
 * Acessos ao GitHub e ao GitLab (#193): os tokens (PAT) guardados no chaveiro do
 * sistema, globais por host ou próprios de um repositório, o comando que imprime o
 * token no lugar do token guardado e o resultado do último teste. Espelha
 * `ui/src-tauri/src/acessos.rs`. Nada aqui traz o token: só se ele existe.
 */

/** Host acrescentado em “+ Adicionar host” (os dos projetos a tela já conhece). */
export interface HostCadastrado {
  host: string;
  forja: ForgeKind;
}

/** Resultado do último “Testar” de um token. */
export interface TesteToken {
  ok: boolean;
  /** Milissegundos desde 1970 (UTC). */
  quando: number;
  mensagem?: string | null;
  usuario?: string | null;
  /** Validade, `AAAA-MM-DD`, quando a forja informa. */
  expira?: string | null;
}

/** Situação do token de um alvo: nunca o token. */
export interface EstadoToken {
  guardado: boolean;
  /** Comando que imprime o token global do host (só no alvo de host). */
  comando: string | null;
  teste: TesteToken | null;
}

export interface ApiAcessos {
  hostsAcessos(): Promise<HostCadastrado[]>;
  adicionarHostAcesso(host: string, forja: ForgeKind): Promise<HostCadastrado>;
  /** Apaga o token e o comando do host, o último teste e o host acrescentado. */
  removerHostAcesso(host: string): Promise<void>;
  /** Usa o comando no lugar do token guardado do host (que é apagado). */
  definirComandoHost(host: string, comando: string): Promise<void>;
  estadoTokens(alvos: AlvoToken[]): Promise<EstadoToken[]>;
  /**
   * Testa o token global do host na API da forja; sem a CLI (`gh`, `glab`), pelo git
   * no repositório `exemplo`. Guarda e devolve o resultado.
   */
  testarTokenHost(
    host: string,
    forja: ForgeKind,
    exemplo: RepositorioCodigo | null,
  ): Promise<TesteToken>;
  /** Testa o acesso de leitura do repositório com o token dele; guarda o resultado. */
  testarTokenRepositorio(repositorio: RepositorioCodigo): Promise<TesteToken>;
}

export const apiAcessosTauri: ApiAcessos = {
  hostsAcessos: () => invoke("hosts_acessos_app"),
  adicionarHostAcesso: (host, forja) => invoke("adicionar_host_acesso_app", { host, forja }),
  removerHostAcesso: (host) => invoke("remover_host_acesso_app", { host }),
  definirComandoHost: (host, comando) => invoke("definir_comando_host_app", { host, comando }),
  estadoTokens: (alvos) => invoke("estado_tokens_app", { alvos }),
  testarTokenHost: (host, forja, exemplo) =>
    invoke("testar_token_host_app", { host, forja, exemplo }),
  testarTokenRepositorio: (repositorio) => invoke("testar_token_repositorio_app", { repositorio }),
};

// ---------------------------------------------------------------------------
// Quem usa cada token, a partir do cadastro dos projetos.

/** Um uso do token global de um host: um repositório de código ou as issues. */
export interface UsoToken {
  projetoId: string;
  projetoNome: string;
  /** Último trecho do caminho do repositório, ou “issues”. */
  rotulo: string;
}

export interface HostAcesso {
  host: string;
  forja: ForgeKind;
  usos: UsoToken[];
  /** Um repositório do host que usa o token global, para testar pelo git. */
  exemplo: RepositorioCodigo | null;
  /** Acrescentado à mão (não vem dos projetos). */
  extra: boolean;
}

export interface RepositorioTokenProprio {
  projetoId: string;
  projetoNome: string;
  repositorio: RepositorioCodigo;
}

const normal = (h: string) => h.trim().toLowerCase();

/** O GitHub só em `github.com`; qualquer outro host é GitLab. */
export function forjaDoHost(host: string): ForgeKind {
  return normal(host) === "github.com" ? "github" : "gitlab";
}

/**
 * Os hosts com token global: `github.com`, os dos repositórios de código e do GitLab
 * Issues dos projetos e os acrescentados à mão, cada um com quem o usa.
 */
export function hostsDosProjetos(
  cfg: Pick<Configuracoes, "projetos">,
  extras: HostCadastrado[] = [],
): HostAcesso[] {
  const hosts: HostAcesso[] = [];
  const obter = (host: string, forja: ForgeKind, extra = false) => {
    const h = normal(host);
    let item = hosts.find((x) => x.host === h);
    if (!item) {
      item = { host: h, forja, usos: [], exemplo: null, extra };
      hosts.push(item);
    }
    return item;
  };
  obter("github.com", "github");
  for (const p of cfg.projetos) {
    const r = p.repositorios;
    if (!r) continue;
    for (const c of r.codigo) {
      if (!c.host.trim()) continue;
      const item = obter(c.host, c.forja);
      if (c.token !== "global") continue;
      item.usos.push({
        projetoId: p.id,
        projetoNome: p.nome,
        rotulo: c.caminho.split("/").filter(Boolean).pop() ?? c.caminho,
      });
      item.exemplo ??= c;
    }
    const t = r.tracker;
    if (t?.kind === "gitlab" && t.host.trim()) {
      const item = obter(t.host, "gitlab");
      if (!t.credential_cmd.trim()) {
        item.usos.push({
          projetoId: p.id,
          projetoNome: p.nome,
          rotulo: "issues",
        });
      }
    } else if (t?.kind === "github") {
      obter("github.com", "github").usos.push({
        projetoId: p.id,
        projetoNome: p.nome,
        rotulo: "issues",
      });
    }
  }
  for (const e of extras) {
    const item = obter(e.host, e.forja, true);
    if (item.usos.length === 0) item.forja = e.forja;
  }
  return hosts;
}

/** Os repositórios de código com token próprio ou comando: têm precedência sobre o do host. */
export function repositoriosComTokenProprio(
  cfg: Pick<Configuracoes, "projetos">,
): RepositorioTokenProprio[] {
  return cfg.projetos.flatMap((p) =>
    (p.repositorios?.codigo ?? [])
      .filter((c) => c.token === "proprio" || c.token === "comando")
      .map((repositorio) => ({
        projetoId: p.id,
        projetoNome: p.nome,
        repositorio,
      })),
  );
}

/** “Faturamento (faturamento-api), Portal (portal, issues)”. */
export function descreverUsos(usos: UsoToken[]): string {
  const porProjeto = new Map<string, { nome: string; rotulos: string[] }>();
  for (const u of usos) {
    const p = porProjeto.get(u.projetoId) ?? {
      nome: u.projetoNome,
      rotulos: [],
    };
    if (!p.rotulos.includes(u.rotulo)) p.rotulos.push(u.rotulo);
    porProjeto.set(u.projetoId, p);
  }
  return [...porProjeto.values()].map((p) => `${p.nome} (${p.rotulos.join(", ")})`).join(", ");
}

// ---------------------------------------------------------------------------
// Situação do token para a tela.

export type TomSituacao = "ok" | "atencao" | "erro" | "neutro";

export interface SituacaoToken {
  tom: TomSituacao;
  texto: string;
  /** Detalhe (motivo da falha, dono, validade). */
  detalhe: string | null;
}

/** Dias antes do vencimento a partir dos quais a validade vira aviso. */
export const DIAS_AVISO_VALIDADE = 14;

const DIA = 24 * 60 * 60 * 1000;

function diaLocal(ms: number): number {
  const d = new Date(ms);
  return Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()) / DIA;
}

function diaDaData(iso: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return null;
  return Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])) / DIA;
}

/** `10/11` (mesmo ano de `agora`) ou `10/11/2027`. */
export function dataCurta(iso: string, agora: number): string {
  const [a, m, d] = iso.split("-");
  return a === String(new Date(agora).getFullYear()) ? `${d}/${m}` : `${d}/${m}/${a}`;
}

/** “hoje”, “ontem” ou “há N dias”. */
export function haQuanto(ms: number, agora: number): string {
  const dias = Math.max(0, diaLocal(agora) - diaLocal(ms));
  if (dias === 0) return "hoje";
  if (dias === 1) return "ontem";
  return `há ${dias} dias`;
}

/**
 * Como mostrar o token de um alvo: sem token, não testado, falhou, expirado, perto de
 * vencer ou ok. `agora` em milissegundos.
 */
export function situacaoDoToken(estado: EstadoToken | undefined, agora: number): SituacaoToken {
  if (!estado) return { tom: "neutro", texto: "…", detalhe: null };
  if (!estado.guardado && !estado.comando) {
    return { tom: "neutro", texto: "sem token", detalhe: null };
  }
  const t = estado.teste;
  if (!t) return { tom: "neutro", texto: "não testado", detalhe: null };
  const expira = t.expira ? diaDaData(t.expira) : null;
  const restam = expira === null ? null : expira - diaLocal(agora);
  if (t.expira && restam !== null && restam < 0) {
    return {
      tom: "erro",
      texto: `✕ expirou em ${dataCurta(t.expira, agora)}`,
      detalhe: t.mensagem ?? null,
    };
  }
  if (!t.ok) {
    return {
      tom: "erro",
      texto: `✕ sem acesso · ${haQuanto(t.quando, agora)}`,
      detalhe: t.mensagem ?? null,
    };
  }
  const dono = t.usuario ? `token de ${t.usuario}` : null;
  if (t.expira && restam !== null && restam <= DIAS_AVISO_VALIDADE) {
    return {
      tom: "atencao",
      texto: `! vence em ${dataCurta(t.expira, agora)}`,
      detalhe: [dono, `acesso ok ${haQuanto(t.quando, agora)}`].filter(Boolean).join(" · "),
    };
  }
  const validade = t.expira ? `válido até ${dataCurta(t.expira, agora)}` : null;
  return {
    tom: "ok",
    texto: `✓ acesso ok · ${haQuanto(t.quando, agora)}`,
    detalhe: [dono, validade].filter(Boolean).join(" · ") || null,
  };
}
