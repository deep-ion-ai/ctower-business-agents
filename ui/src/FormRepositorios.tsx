import { Icon } from "./shell/icons";
import {
  GITHUB_VAZIO,
  GITLAB_VAZIO,
  JIRA_VAZIO,
  MAX_ISSUES_TETO,
  mapaDeTipos,
  textoDoMapa,
  type ForgeConfig,
  type GitlabConfig,
  type JiraConfig,
  type Repositorios,
  type TrackerConfig,
} from "./repositorios";

type KindForge = "" | ForgeConfig["kind"];
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

function CamposGitlab({
  valor,
  aoMudar,
  fixo,
}: {
  valor: GitlabConfig;
  aoMudar: (v: GitlabConfig) => void;
  /** Host e caminho não mudam depois do cadastro. */
  fixo: boolean;
}) {
  return (
    <>
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
      <CampoTexto
        rotulo="URL de clone (HTTPS ou SSH)"
        valor={valor.clone_url}
        exemplo="https://gitlab.example.com/grupo/projeto/app.git"
        aoMudar={(clone_url) => aoMudar({ ...valor, clone_url })}
      />
      <CampoTexto
        rotulo="Comando de credencial (imprime o token)"
        valor={valor.credential_cmd}
        exemplo="pass show gitlab | head -1"
        mono
        aoMudar={(credential_cmd) => aoMudar({ ...valor, credential_cmd })}
      />
    </>
  );
}

function CamposJira({ valor, aoMudar }: { valor: JiraConfig; aoMudar: (v: JiraConfig) => void }) {
  const campo = <K extends keyof JiraConfig>(k: K, v: JiraConfig[K]) =>
    aoMudar({ ...valor, [k]: v });
  return (
    <>
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
      {!valor.active_sprint_only && (
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
  );
}

function CampoRepoGithub({
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

/**
 * Seção “Repositórios” do cadastro do projeto: onde fica o código (GitHub ou GitLab) e
 * onde ficam as issues (GitHub, GitLab ou Jira). O tipo de cada um não muda depois do
 * cadastro; por isso `travado` (o que já está gravado) desabilita o seletor e, no
 * GitLab, host e caminho. O cadastro guarda só o comando de credencial, nunca o token.
 */
export function FormRepositorios({
  valor,
  aoMudar,
  travado,
}: {
  valor: Repositorios;
  aoMudar: (v: Repositorios) => void;
  travado?: Repositorios;
}) {
  const forge = valor.forge;
  const tracker = valor.tracker;
  const forgeFixa = travado?.forge != null;
  const trackerFixo = travado?.tracker != null;

  const trocarForge = (kind: KindForge) => {
    aoMudar({
      ...valor,
      forge:
        kind === "github"
          ? { kind, ...GITHUB_VAZIO }
          : kind === "gitlab"
            ? { kind, ...GITLAB_VAZIO }
            : null,
    });
  };
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
        Opcional. O app guarda só o comando que imprime o token (por exemplo{" "}
        <code>pass show gitlab | head -1</code>), nunca o token.
      </p>

      <div className="field">
        <label>
          <Icon name={forge?.kind ?? "project"} /> Repositório do código-fonte
          <select
            value={forge?.kind ?? ""}
            disabled={forgeFixa}
            onChange={(e) => trocarForge(e.target.value as KindForge)}
          >
            <option value="">Não configurar</option>
            <option value="github">GitHub</option>
            <option value="gitlab">GitLab</option>
          </select>
        </label>
      </div>
      {forge?.kind === "github" && (
        <CampoRepoGithub
          rotulo="Repositório do código (dono/repositório)"
          valor={forge}
          aoMudar={(g) => aoMudar({ ...valor, forge: { kind: "github", ...g } })}
        />
      )}
      {forge?.kind === "gitlab" && (
        <CamposGitlab
          valor={forge}
          fixo={forgeFixa}
          aoMudar={(g) => aoMudar({ ...valor, forge: { kind: "gitlab", ...g } })}
        />
      )}

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
