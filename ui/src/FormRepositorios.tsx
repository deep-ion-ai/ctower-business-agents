import { useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import { Icon } from "./shell/icons";
import {
  GITHUB_VAZIO,
  GITLAB_VAZIO,
  JIRA_VAZIO,
  MAX_ISSUES_TETO,
  REPOSITORIO_CODIGO_VAZIO,
  alvoDoToken,
  apiRepositoriosTauri,
  mapaDeTipos,
  mesmoRepositorio,
  repositorioDaUrl,
  textoDoMapa,
  type ApiRepositorios,
  type ForgeKind,
  type GitlabConfig,
  type JiraConfig,
  type OrigemToken,
  type RepositorioCodigo,
  type Repositorios,
  type TrackerConfig,
} from "./repositorios";

type KindTracker = "" | TrackerConfig["kind"];

function CampoTexto({
  rotulo,
  valor,
  aoMudar,
  exemplo,
  desabilitado,
  mono,
}: {
  rotulo: string;
  valor: string;
  aoMudar: (v: string) => void;
  exemplo?: string;
  desabilitado?: boolean;
  mono?: boolean;
}) {
  return (
    <div className="field">
      <label>
        {rotulo}
        <input
          type="text"
          className={mono ? "caminho" : undefined}
          value={valor}
          placeholder={exemplo}
          spellCheck={false}
          disabled={desabilitado}
          onChange={(e) => aoMudar(e.target.value)}
        />
      </label>
    </div>
  );
}

/** Parte dos campos: só os essenciais, só os de “Mais opções” ou todos (o padrão). */
export type ParteDosCampos = "essenciais" | "mais" | "tudo";

export function CamposGitlab({
  valor,
  aoMudar,
  fixo,
  parte = "tudo",
}: {
  valor: GitlabConfig;
  aoMudar: (v: GitlabConfig) => void;
  /** Host e caminho não mudam depois do cadastro. */
  fixo: boolean;
  parte?: ParteDosCampos;
}) {
  return (
    <>
      {parte !== "mais" && (
        <div className="field-pair">
          <CampoTexto
            rotulo="Host do GitLab"
            valor={valor.host}
            exemplo="gitlab.example.com"
            desabilitado={fixo}
            aoMudar={(host) => aoMudar({ ...valor, host })}
          />
          <CampoTexto
            rotulo="Caminho do projeto"
            valor={valor.path}
            exemplo="grupo/projeto/app"
            desabilitado={fixo}
            aoMudar={(path) => aoMudar({ ...valor, path })}
          />
        </div>
      )}
      {parte !== "essenciais" && (
        <>
          <CampoTexto
            rotulo="URL de clone (HTTPS ou SSH)"
            valor={valor.clone_url}
            exemplo="https://gitlab.example.com/grupo/projeto/app.git"
            aoMudar={(clone_url) => aoMudar({ ...valor, clone_url })}
          />
          <CampoTexto
            rotulo="Comando de credencial (opcional, imprime o token)"
            valor={valor.credential_cmd}
            exemplo="pass show gitlab | head -1"
            mono
            aoMudar={(credential_cmd) => aoMudar({ ...valor, credential_cmd })}
          />
          <p className="muted">
            Vazio: vale o token global do host guardado no chaveiro do sistema (Configurações ›
            Acessos).
          </p>
        </>
      )}
    </>
  );
}

export function CamposJira({
  valor,
  aoMudar,
  parte = "tudo",
}: {
  valor: JiraConfig;
  aoMudar: (v: JiraConfig) => void;
  parte?: ParteDosCampos;
}) {
  const campo = <K extends keyof JiraConfig>(k: K, v: JiraConfig[K]) =>
    aoMudar({ ...valor, [k]: v });
  const essenciais = parte !== "mais";
  const mais = parte !== "essenciais";
  return (
    <>
      {essenciais && (
        <div className="field-pair">
          <CampoTexto
            rotulo="URL do Jira"
            valor={valor.url}
            exemplo="https://jira.example.com"
            aoMudar={(v) => campo("url", v)}
          />
          <CampoTexto
            rotulo="Chave do projeto"
            valor={valor.project_key}
            exemplo="PROJ"
            aoMudar={(v) => campo("project_key", v)}
          />
        </div>
      )}
      {essenciais && (
        <div className="field">
          <label>
            <input
              type="checkbox"
              checked={valor.active_sprint_only}
              onChange={(e) => campo("active_sprint_only", e.target.checked)}
            />{" "}
            Usar somente a sprint ativa (<code>sprint in openSprints()</code>)
          </label>
        </div>
      )}
      {mais && !valor.active_sprint_only && (
        <div className="field-pair">
          <CampoTexto
            rotulo="JQL"
            valor={valor.jql}
            exemplo="labels = minha-equipe AND updated >= -90d"
            mono
            aoMudar={(v) => campo("jql", v)}
          />
          <div className="field">
            <label>
              Máximo de issues (as mais recentes por updated)
              <input
                type="number"
                min={1}
                max={MAX_ISSUES_TETO}
                value={valor.max_issues}
                onChange={(e) => campo("max_issues", Number(e.target.value))}
              />
            </label>
          </div>
        </div>
      )}
      {mais && (
        <>
          <CampoTexto
            rotulo="Comando de credencial (imprime o token)"
            valor={valor.credential_cmd}
            exemplo="pass show jira | head -1"
            mono
            aoMudar={(v) => campo("credential_cmd", v)}
          />
          <CampoTexto
            rotulo="Configuração do jira-cli"
            valor={valor.config_file}
            exemplo="~/.config/.jira/.config.yml"
            mono
            aoMudar={(v) => campo("config_file", v)}
          />
          <div className="field-pair">
            <CampoTexto
              rotulo="Campo do Epic Link (opcional)"
              valor={valor.epic_link_field}
              exemplo="customfield_10601"
              aoMudar={(v) => campo("epic_link_field", v)}
            />
            <CampoTexto
              rotulo="Campo do Epic Name (opcional)"
              valor={valor.epic_name_field}
              exemplo="customfield_10604"
              aoMudar={(v) => campo("epic_name_field", v)}
            />
            <CampoTexto
              rotulo="Campo da sprint (vazio = detectar)"
              valor={valor.sprint_field}
              exemplo="customfield_10600"
              aoMudar={(v) => campo("sprint_field", v)}
            />
          </div>
          <div className="field">
            <label>
              Tipos de issue (tipo = epic | issue | bug | chore | ignore)
              <textarea
                rows={4}
                value={textoDoMapa(valor.type_map)}
                onChange={(e) => campo("type_map", mapaDeTipos(e.target.value))}
              />
            </label>
          </div>
        </>
      )}
    </>
  );
}

export function CampoRepoGithub({
  valor,
  aoMudar,
  rotulo,
}: {
  valor: { repo: string };
  aoMudar: (v: { repo: string }) => void;
  rotulo: string;
}) {
  return (
    <CampoTexto
      rotulo={rotulo}
      valor={valor.repo}
      exemplo="deep-ion-ai/app"
      aoMudar={(repo) => aoMudar({ repo })}
    />
  );
}

const ORIGENS_TOKEN: { valor: OrigemToken; rotulo: string }[] = [
  { valor: "global", rotulo: "Token global do host" },
  { valor: "proprio", rotulo: "Token só deste repositório" },
  { valor: "comando", rotulo: "Comando que imprime o token" },
];

export type ApiToken = Pick<ApiRepositorios, "salvarToken" | "tokensGuardados" | "testarAcesso">;

/**
 * Token (PAT) de um repositório de código, quando a origem é o chaveiro (global do host
 * ou próprio do repositório): testar o acesso com o token digitado e guardá-lo no
 * chaveiro do sistema. O token digitado fica só neste campo: não entra no valor do
 * cadastro e some depois de guardado.
 */
function TokenDoRepositorio({ api, valor }: { api: ApiToken; valor: RepositorioCodigo }) {
  const [token, setToken] = useState("");
  const [guardado, setGuardado] = useState<boolean | null>(null);
  const [ocupado, setOcupado] = useState<"teste" | "guardar" | null>(null);
  const [aviso, setAviso] = useState<{ ok: boolean; texto: string } | null>(null);
  const [vez, setVez] = useState(0);
  const alvo = alvoDoToken(valor);
  const completo = alvo.host !== "" && (valor.token !== "proprio" || alvo.caminho !== "");
  const host = alvo.host;
  const caminho = alvo.caminho ?? "";

  useEffect(() => {
    setGuardado(null);
    if (!completo) return;
    let ativo = true;
    // Espera a digitação do host ou do caminho assentar antes de consultar o chaveiro.
    const t = setTimeout(() => {
      Promise.resolve()
        .then(() => api.tokensGuardados([{ host, caminho: caminho || null }]))
        .then((r) => ativo && setGuardado(r?.[0] === true))
        .catch(() => ativo && setGuardado(null));
    }, 200);
    return () => {
      ativo = false;
      clearTimeout(t);
    };
  }, [api, host, caminho, completo, vez]);

  const testar = () => {
    setOcupado("teste");
    setAviso(null);
    api
      .testarAcesso(valor, token.trim() || null)
      .then(() => setAviso({ ok: true, texto: "Acesso OK" }))
      .catch((e: unknown) => setAviso({ ok: false, texto: mensagemDe(e) }))
      .finally(() => setOcupado(null));
  };

  const guardar = () => {
    setOcupado("guardar");
    setAviso(null);
    api
      .salvarToken(alvo, token.trim())
      .then(() => {
        setToken("");
        setAviso({ ok: true, texto: "Token guardado no chaveiro." });
        setVez((v) => v + 1);
      })
      .catch((e: unknown) => setAviso({ ok: false, texto: mensagemDe(e) }))
      .finally(() => setOcupado(null));
  };

  return (
    <div className="token-repositorio">
      <div className="field">
        <label>
          Token (PAT)
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={token}
            placeholder={guardado ? "guardado; digite para trocar" : undefined}
            onChange={(e) => setToken(e.target.value)}
          />
        </label>
        <p className="muted">
          {valor.token === "proprio"
            ? "Só deste repositório; tem precedência sobre o token global do host."
            : `Vale para todos os repositórios de ${alvo.host || "este host"} ` +
              "(Configurações › Acessos)."}{" "}
          Fica no chaveiro do sistema, nunca no projeto.
          {guardado === true && <span className="ok-text"> Token guardado.</span>}
          {guardado === false && <span> Nenhum token guardado.</span>}
        </p>
      </div>
      <div className="acoes">
        <button
          className="btn btn-sm"
          type="button"
          disabled={!completo || ocupado !== null}
          onClick={testar}
        >
          Testar acesso
        </button>{" "}
        <button
          className="btn btn-sm"
          type="button"
          disabled={!completo || !token.trim() || ocupado !== null}
          onClick={guardar}
        >
          Guardar no chaveiro
        </button>
      </div>
      <p aria-live="polite" role="status">
        {ocupado === "teste" && <span className="muted">Testando o acesso…</span>}
        {aviso && <span className={aviso.ok ? "ok-text" : "error-text"}>{aviso.texto}</span>}
      </p>
    </div>
  );
}

/**
 * Um repositório de código da lista. Colar a URL de clone preenche forja, host e
 * caminho; `fixo` (já gravado) trava os três, porque o cadastro não os muda depois.
 */
export function ItemRepositorioCodigo({
  numero,
  valor,
  aoMudar,
  aoRemover,
  fixo,
  api,
  nome = `Repositório de código ${numero}`,
}: {
  api: ApiToken;
  numero: number;
  valor: RepositorioCodigo;
  aoMudar: (v: RepositorioCodigo) => void;
  /** Sem ele, o item não oferece “Remover”. */
  aoRemover?: () => void;
  fixo: boolean;
  nome?: string;
}) {
  const mudarUrl = (url_clone: string) => {
    const deduzido = fixo ? null : repositorioDaUrl(url_clone);
    aoMudar(
      deduzido
        ? {
            ...valor,
            url_clone,
            forja: deduzido.forja,
            host: deduzido.host,
            caminho: deduzido.caminho,
          }
        : { ...valor, url_clone },
    );
  };
  const mudarForja = (forja: ForgeKind) =>
    aoMudar({
      ...valor,
      forja,
      host: forja === "github" ? "github.com" : valor.host === "github.com" ? "" : valor.host,
    });
  return (
    <fieldset className="repositorio-codigo" aria-label={nome}>
      <legend>
        <Icon name={valor.forja} /> {nome}
      </legend>
      <CampoTexto
        rotulo="URL de clone (HTTPS ou SSH)"
        valor={valor.url_clone}
        exemplo="https://github.com/dono/repositorio.git"
        mono
        aoMudar={mudarUrl}
      />
      <div className="field-pair">
        <div className="field">
          <label>
            Forja
            <select
              value={valor.forja}
              disabled={fixo}
              onChange={(e) => mudarForja(e.target.value as ForgeKind)}
            >
              <option value="github">GitHub</option>
              <option value="gitlab">GitLab</option>
            </select>
          </label>
        </div>
        <CampoTexto
          rotulo="Host"
          valor={valor.host}
          exemplo={valor.forja === "github" ? "github.com" : "gitlab.example.com"}
          desabilitado={fixo}
          aoMudar={(host) => aoMudar({ ...valor, host })}
        />
        <CampoTexto
          rotulo="Caminho"
          valor={valor.caminho}
          exemplo={valor.forja === "github" ? "dono/repositorio" : "grupo/projeto/app"}
          desabilitado={fixo}
          aoMudar={(caminho) => aoMudar({ ...valor, caminho })}
        />
      </div>
      <div className="field-pair">
        <CampoTexto
          rotulo="Branch (opcional)"
          valor={valor.branch}
          exemplo="vazio = branch padrão"
          aoMudar={(branch) => aoMudar({ ...valor, branch })}
        />
        <div className="field">
          <label>
            Token
            <select
              value={valor.token}
              onChange={(e) => aoMudar({ ...valor, token: e.target.value as OrigemToken })}
            >
              {ORIGENS_TOKEN.map((o) => (
                <option key={o.valor} value={o.valor}>
                  {o.rotulo}
                </option>
              ))}
            </select>
          </label>
        </div>
      </div>
      {(valor.token === "global" || valor.token === "proprio") && (
        <TokenDoRepositorio api={api} valor={valor} />
      )}
      {valor.token === "comando" && (
        <CampoTexto
          rotulo="Comando de credencial (imprime o token)"
          valor={valor.comando_credencial}
          exemplo="pass show gitlab | head -1"
          mono
          aoMudar={(comando_credencial) => aoMudar({ ...valor, comando_credencial })}
        />
      )}
      {!fixo && aoRemover && (
        <div className="acoes">
          <button className="btn btn-sm" type="button" onClick={aoRemover}>
            Remover
          </button>
        </div>
      )}
    </fieldset>
  );
}

/**
 * Seção “Repositórios” do cadastro do projeto: os repositórios de código (GitHub ou
 * GitLab, quantos forem) e onde ficam as issues (GitHub, GitLab ou Jira). O que já está
 * gravado (`travado`) não muda: o repositório de código não sai da lista nem troca de
 * forja, host ou caminho, e o tipo do rastreador fica travado (no GitLab, também host e
 * caminho). O cadastro guarda só o comando de credencial, nunca o token: o token (PAT)
 * vai para o chaveiro do sistema pela `api`.
 */
export function FormRepositorios({
  valor,
  aoMudar,
  travado,
  api = apiRepositoriosTauri,
}: {
  valor: Repositorios;
  aoMudar: (v: Repositorios) => void;
  travado?: Repositorios;
  api?: ApiToken;
}) {
  const tracker = valor.tracker;
  const trackerFixo = travado?.tracker != null;
  // Fixo = já gravado. Só a primeira ocorrência trava: uma repetição digitada num item
  // novo continua editável e removível (a validação aponta a duplicata).
  const fixos = valor.codigo.map(
    (c, i) =>
      (travado?.codigo.some((t) => mesmoRepositorio(t, c)) ?? false) &&
      valor.codigo.findIndex((d) => mesmoRepositorio(d, c)) === i,
  );

  const mudarCodigo = (codigo: RepositorioCodigo[]) => aoMudar({ ...valor, codigo });
  const trocarTracker = (kind: KindTracker) => {
    aoMudar({
      ...valor,
      tracker:
        kind === "github"
          ? { kind, ...GITHUB_VAZIO }
          : kind === "gitlab"
            ? { kind, ...GITLAB_VAZIO }
            : kind === "jira"
              ? { kind, ...JIRA_VAZIO }
              : null,
    });
  };

  return (
    <fieldset className="repositorios-projeto" aria-label="Repositórios">
      <legend>Repositórios</legend>
      <p className="muted">
        Opcional. O token (PAT) fica no chaveiro do sistema, nunca no projeto; o cadastro guarda no
        máximo o comando que imprime o token (por exemplo <code>pass show gitlab | head -1</code>).
      </p>

      <section className="repositorios-codigo" aria-label="Repositórios de código">
        <h4>
          <Icon name="project" /> Repositórios de código
        </h4>
        {valor.codigo.length === 0 && <p className="muted">Nenhum repositório de código.</p>}
        {valor.codigo.map((c, i) => (
          <ItemRepositorioCodigo
            key={i}
            numero={i + 1}
            valor={c}
            fixo={fixos[i]!}
            api={api}
            aoMudar={(novo) => mudarCodigo(valor.codigo.map((d, j) => (j === i ? novo : d)))}
            aoRemover={() => mudarCodigo(valor.codigo.filter((_, j) => j !== i))}
          />
        ))}
        <button
          className="btn btn-sm"
          type="button"
          onClick={() => mudarCodigo([...valor.codigo, { ...REPOSITORIO_CODIGO_VAZIO }])}
        >
          Adicionar repositório
        </button>
      </section>

      <div className="field">
        <label>
          <Icon name={tracker?.kind ?? "project"} /> Repositório de demandas (issues)
          <select
            value={tracker?.kind ?? ""}
            disabled={trackerFixo}
            onChange={(e) => trocarTracker(e.target.value as KindTracker)}
          >
            <option value="">Não configurar</option>
            <option value="github">GitHub Issues</option>
            <option value="gitlab">GitLab Issues</option>
            <option value="jira">Jira</option>
          </select>
        </label>
      </div>
      {tracker?.kind === "github" && (
        <CampoRepoGithub
          rotulo="Repositório das issues (dono/repositório)"
          valor={tracker}
          aoMudar={(g) => aoMudar({ ...valor, tracker: { kind: "github", ...g } })}
        />
      )}
      {tracker?.kind === "gitlab" && (
        <CamposGitlab
          valor={tracker}
          fixo={trackerFixo}
          aoMudar={(g) => aoMudar({ ...valor, tracker: { kind: "gitlab", ...g } })}
        />
      )}
      {tracker?.kind === "jira" && (
        <CamposJira
          valor={tracker}
          aoMudar={(j) => aoMudar({ ...valor, tracker: { kind: "jira", ...j } })}
        />
      )}
    </fieldset>
  );
}
