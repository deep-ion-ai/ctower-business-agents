import { useEffect, useId, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, Cartao, TituloSecao } from "./componentes";
import { jiraComConexao } from "./conexoes";
import { SeletorConexao } from "./SeletorConexao";
import {
  OUTRO_REPOSITORIO,
  ROTULO_FORJA,
  chaveRepositorio,
  rotuloRepositorio,
  trackerDoProjeto,
  type ApiAssistenteProjeto,
  type EstadoDemandas,
  type TipoDemanda,
} from "./estadoCadastro";
import {
  ROTULO_SITUACAO_DEMANDAS,
  falhou,
  resumoDoTeste,
  situacaoDasDemandas,
} from "./saudeProjeto";
import {
  MAX_ISSUES_TETO,
  hostDaUrl,
  mapaDeTipos,
  textoDoMapa,
  type IntegracaoProntidao,
  type JiraConfig,
  type RepositorioCodigo,
  type SituacaoPasso,
} from "./repositorios";

type Api = Pick<
  ApiAssistenteProjeto,
  "testarJira" | "testarDemandas" | "tokensGuardados"
>;
type Mudar = (f: (d: EstadoDemandas) => EstadoDemandas) => void;

const TIPOS: { id: TipoDemanda; nome: string; descricao: string }[] = [
  {
    id: "nenhum",
    nome: "Nenhum",
    descricao: "Demandas criadas no próprio app",
  },
  {
    id: "github",
    nome: "GitHub Issues",
    descricao: "De um dos repositórios do projeto",
  },
  {
    id: "gitlab",
    nome: "GitLab Issues",
    descricao: "De um dos repositórios do projeto",
  },
  { id: "jira", nome: "Jira", descricao: "Projeto, sprint ativa ou JQL" },
];

/**
 * Passo 3 do cadastro (opcional): de onde vêm as demandas. O código já vem do passo 1;
 * as issues do GitHub ou do GitLab vêm de um daqueles repositórios, com o mesmo token; o
 * Jira pede só endereço, chave e token (guardado no chaveiro), e o resto fica em "Mais
 * opções".
 */
export function PassoDemandas({
  api,
  codigo,
  demandas,
  mudar,
}: {
  api: Api;
  codigo: RepositorioCodigo[];
  demandas: EstadoDemandas;
  mudar: Mudar;
}) {
  const escolher = (tipo: TipoDemanda) =>
    mudar((d) => {
      if (tipo !== "github" && tipo !== "gitlab") return { ...d, tipo };
      const atual = codigo.find((r) => chaveRepositorio(r) === d.repoIssues);
      const primeiro = codigo.find((r) => r.forja === tipo);
      const repoIssues =
        atual && atual.forja === tipo
          ? d.repoIssues
          : primeiro
            ? chaveRepositorio(primeiro)
            : OUTRO_REPOSITORIO;
      return { ...d, tipo, repoIssues };
    });

  return (
    <>
      <div className="secao-passo">
        <TituloSecao nivel={3}>Código-fonte</TituloSecao>
        <Cartao como="div" className="linha-info">
          <span className="ok-text" aria-hidden="true">
            ✓
          </span>
          {codigo.length > 0 ? (
            <div>
              <p>{codigo.map(rotuloRepositorio).join(" e ")}</p>
              <p className="dica">
                Vem dos repositórios do passo 1, com os mesmos tokens. Nada para
                preencher.
              </p>
            </div>
          ) : (
            <div>
              <p>
                As pastas do passo 1, sem repositório do GitHub ou do GitLab.
              </p>
              <p className="dica">
                Para usar issues, informe o repositório abaixo.
              </p>
            </div>
          )}
        </Cartao>
      </div>

      <fieldset className="opcoes-cartao opcoes-demandas">
        <legend className="subtitulo-passo">De onde vêm as demandas?</legend>
        {TIPOS.map((t) => (
          <label
            key={t.id}
            className={demandas.tipo === t.id ? "opcao escolhida" : "opcao"}
          >
            <input
              type="radio"
              name="origem-demandas"
              value={t.id}
              checked={demandas.tipo === t.id}
              onChange={() => escolher(t.id)}
            />
            <strong>{t.nome}</strong>
            <span className="muted">{t.descricao}</span>
          </label>
        ))}
      </fieldset>

      {(demandas.tipo === "github" || demandas.tipo === "gitlab") && (
        <IssuesDoRepositorio
          api={api}
          forja={demandas.tipo}
          codigo={codigo}
          demandas={demandas}
          mudar={mudar}
        />
      )}
      {demandas.tipo === "jira" && (
        <Jira api={api} codigo={codigo} demandas={demandas} mudar={mudar} />
      )}
    </>
  );
}

function Campo({
  rotulo,
  valor,
  aoMudar,
  exemplo,
  mono,
  tipo = "text",
}: {
  rotulo: string;
  valor: string;
  aoMudar: (v: string) => void;
  exemplo?: string;
  mono?: boolean;
  tipo?: "text" | "password";
}) {
  const id = useId();
  return (
    <div className="field">
      <label htmlFor={id}>{rotulo}</label>
      <input
        id={id}
        type={tipo}
        className={mono ? "caminho" : undefined}
        value={valor}
        placeholder={exemplo}
        spellCheck={false}
        autoComplete="off"
        onChange={(e) => aoMudar(e.target.value)}
      />
    </div>
  );
}

function IssuesDoRepositorio({
  api,
  forja,
  codigo,
  demandas,
  mudar,
}: {
  api: Api;
  forja: "github" | "gitlab";
  codigo: RepositorioCodigo[];
  demandas: EstadoDemandas;
  mudar: Mudar;
}) {
  const id = useId();
  const opcoes = codigo.filter((r) => r.forja === forja);
  const outro = demandas.repoIssues === OUTRO_REPOSITORIO;
  const tracker = trackerDoProjeto(demandas, codigo);
  const [testando, setTestando] = useState(false);
  const [resultado, setResultado] = useState<IntegracaoProntidao | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  // Trocar o repositório invalida o resultado anterior.
  const alvo = JSON.stringify(tracker);
  useEffect(() => {
    setResultado(null);
    setFalha(null);
  }, [alvo]);
  const testar = () => {
    if (!tracker) return;
    setTestando(true);
    setFalha(null);
    setResultado(null);
    api
      .testarDemandas(tracker, null)
      .then(setResultado)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setTestando(false));
  };
  const origem = forja === "github" ? "GitHub" : "GitLab";
  const vazio =
    forja === "github"
      ? tracker?.kind === "github" && !tracker.repo
      : tracker?.kind === "gitlab" && !tracker.path;
  return (
    <Cartao como="div">
      <div className="field">
        <label htmlFor={id}>Repositório das issues</label>
        <select
          id={id}
          value={demandas.repoIssues}
          aria-describedby={`${id}-dica`}
          onChange={(e) => {
            const repoIssues = e.target.value;
            mudar((d) => ({ ...d, repoIssues }));
          }}
        >
          {opcoes.map((r) => (
            <option key={chaveRepositorio(r)} value={chaveRepositorio(r)}>
              {r.caminho} ({ROTULO_FORJA[r.forja]})
            </option>
          ))}
          <option value={OUTRO_REPOSITORIO}>Outro repositório…</option>
        </select>
        <span id={`${id}-dica`} className="dica">
          {outro
            ? `Usa o token global de ${forja === "github" ? "github.com" : "o host"} guardado no chaveiro.`
            : "Usa o mesmo token do repositório escolhido."}
        </span>
      </div>
      {outro && forja === "github" && (
        <Campo
          rotulo="Repositório no GitHub (dono/repositório)"
          valor={demandas.outroGithub}
          exemplo="acme/faturamento"
          mono
          aoMudar={(outroGithub) => mudar((d) => ({ ...d, outroGithub }))}
        />
      )}
      {outro && forja === "gitlab" && (
        <div className="field-pair">
          <Campo
            rotulo="Host do GitLab"
            valor={demandas.outroGitlab.host}
            exemplo="gitlab.example.com"
            aoMudar={(host) =>
              mudar((d) => ({ ...d, outroGitlab: { ...d.outroGitlab, host } }))
            }
          />
          <Campo
            rotulo="Caminho do projeto no GitLab"
            valor={demandas.outroGitlab.path}
            exemplo="grupo/projeto/app"
            mono
            aoMudar={(path) =>
              mudar((d) => ({ ...d, outroGitlab: { ...d.outroGitlab, path } }))
            }
          />
        </div>
      )}
      <div className="linha-campo-botao">
        <button
          type="button"
          className="btn"
          disabled={testando || !tracker || vazio}
          onClick={testar}
        >
          Testar conexão
        </button>
        <span className="dica">
          Confere o token, o repositório e se as issues estão ligadas.
        </span>
      </div>
      <div role="status" aria-live="polite">
        {testando && (
          <span className="muted">Testando a conexão com o {origem}…</span>
        )}
        {falha && (
          <Alerta tom="erro" titulo={`Não deu para testar a conexão com o ${origem}.`}>
            {falha}
          </Alerta>
        )}
        {resultado && (
          <>
            <p className={falhou(resultado) ? "error-text" : "ok-text"}>
              {resumoDoTeste(
                resultado,
                forja === "github" ? "GitHub" : "GitLab",
              )}{" "}
              <span className="muted">
                ({ROTULO_SITUACAO_DEMANDAS[situacaoDasDemandas(resultado)]})
              </span>
            </p>
            {falhou(resultado) && (
              <p className="dica orientacao">
                {
                  resultado.passos.find((p) => p.situacao === "falhou")
                    ?.orientacao
                }
              </p>
            )}
          </>
        )}
      </div>
    </Cartao>
  );
}

const SIMBOLO: Record<SituacaoPasso, string> = {
  ok: "✓",
  falhou: "✗",
  pulado: "–",
};
const ROTULO_SITUACAO: Record<SituacaoPasso, string> = {
  ok: "ok",
  falhou: "falhou",
  pulado: "pulado",
};

function Jira({
  api,
  codigo,
  demandas,
  mudar,
}: {
  api: Api;
  codigo: RepositorioCodigo[];
  demandas: EstadoDemandas;
  mudar: Mudar;
}) {
  const j = demandas.jira;
  const idMais = useId();
  const [mais, setMais] = useState(false);
  const [tipos, setTipos] = useState(() => textoDoMapa(j.type_map));
  const [testando, setTestando] = useState(false);
  const [resultado, setResultado] = useState<IntegracaoProntidao | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const campo = <K extends keyof JiraConfig>(k: K, v: JiraConfig[K]) =>
    mudar((d) => ({ ...d, jira: { ...d.jira, [k]: v } }));
  const host = /^https?:\/\/.{3,}/i.test(j.url.trim()) ? hostDaUrl(j.url) : "";

  useEffect(() => {
    if (!host) {
      mudar((d) =>
        d.jiraGuardado === null ? d : { ...d, jiraGuardado: null },
      );
      return;
    }
    let ativo = true;
    const t = setTimeout(() => {
      Promise.resolve()
        .then(() => api.tokensGuardados([{ host, caminho: null }]))
        .then((r) => {
          if (ativo) mudar((d) => ({ ...d, jiraGuardado: r?.[0] === true }));
        })
        .catch(() => undefined);
    }, 250);
    return () => {
      ativo = false;
      clearTimeout(t);
    };
  }, [api, host, mudar]);

  const testar = () => {
    const tracker = trackerDoProjeto(demandas, codigo);
    if (tracker?.kind !== "jira") return;
    const { kind: _kind, ...jira } = tracker;
    setTestando(true);
    setFalha(null);
    setResultado(null);
    api
      .testarJira(jira, demandas.tokenJira.trim() || null)
      .then(setResultado)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setTestando(false));
  };

  const comConexao = Boolean(j.conexao);
  return (
    <Cartao como="div" className="campos-jira">
      <SeletorConexao
        capacidade="rastreador"
        tipo={demandas.tipo}
        valor={j.conexao ?? ""}
        aoMudar={(_, c) =>
          mudar((d) => ({ ...d, jira: jiraComConexao(d.jira, c), tokenJira: "" }))
        }
      />
      <div className="field-pair">
        {!comConexao && (
          <Campo
            rotulo="Endereço do Jira"
            valor={j.url}
            exemplo="https://jira.suaempresa.com"
            aoMudar={(v) => campo("url", v)}
          />
        )}
        <Campo
          rotulo="Chave do projeto"
          valor={j.project_key}
          exemplo="FAT"
          aoMudar={(v) => campo("project_key", v.toUpperCase())}
        />
      </div>
      {!comConexao && (
      <div className="field">
        <label htmlFor={`${idMais}-token`}>Token do Jira</label>
        <input
          id={`${idMais}-token`}
          type="password"
          className="caminho"
          autoComplete="off"
          spellCheck={false}
          value={demandas.tokenJira}
          placeholder={
            demandas.jiraGuardado
              ? "guardado; digite para trocar"
              : "cole o token"
          }
          aria-describedby={`${idMais}-dica-token`}
          onChange={(e) => {
            const tokenJira = e.target.value;
            mudar((d) => ({ ...d, tokenJira }));
          }}
        />
        <span id={`${idMais}-dica-token`} className="dica">
          Fica no chaveiro do sistema, como os tokens do GitHub e do GitLab.
          {demandas.jiraGuardado === true && (
            <span className="ok-text">
              {" "}
              Já há um token guardado para {host}.
            </span>
          )}
        </span>
      </div>
      )}
      <label className="linha-checkbox">
        <input
          type="checkbox"
          checked={j.active_sprint_only}
          onChange={(e) => campo("active_sprint_only", e.target.checked)}
        />{" "}
        Usar só as issues da sprint ativa
      </label>
      <div className="linha-campo-botao">
        <button
          type="button"
          className="btn"
          disabled={testando || !j.url.trim() || !j.project_key.trim()}
          onClick={testar}
        >
          Testar conexão
        </button>
        <span className="dica">
          Confere o acesso e detecta sozinho os campos de épico e de sprint.
        </span>
      </div>
      <div role="status" aria-live="polite">
        {testando && (
          <span className="muted">Testando a conexão com o Jira…</span>
        )}
        {falha && (
          <Alerta tom="erro" titulo="Não deu para testar a conexão com o Jira.">
            {falha}
          </Alerta>
        )}
        {resultado && (
          <ul
            className="passos-teste"
            aria-label={resultado.titulo || "Resultado do teste"}
          >
            {resultado.passos.map((p) => (
              <li key={p.id} data-situacao={p.situacao}>
                <span className="simbolo" aria-hidden="true">
                  {SIMBOLO[p.situacao]}
                </span>
                <span>
                  {p.rotulo}
                  <span className="sr-only">
                    {" "}
                    ({ROTULO_SITUACAO[p.situacao]})
                  </span>
                  {p.detalhe && <span className="muted">: {p.detalhe}</span>}
                  {p.orientacao && (
                    <span className="dica orientacao">{p.orientacao}</span>
                  )}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div>
        <button
          type="button"
          className="link"
          aria-expanded={mais}
          aria-controls={idMais}
          onClick={() => setMais((m) => !m)}
        >
          {mais
            ? "Menos opções"
            : "Mais opções do Jira (JQL, campos personalizados, tipos de issue)"}
        </button>
        <div id={idMais} hidden={!mais} className="mais-opcoes">
          <Campo
            rotulo={
              j.active_sprint_only
                ? "JQL (quando não for só a sprint ativa)"
                : "JQL"
            }
            valor={j.jql}
            exemplo="labels = minha-equipe AND updated >= -90d"
            mono
            aoMudar={(v) => campo("jql", v)}
          />
          <div className="field">
            <label htmlFor={`${idMais}-max`}>
              Máximo de issues (as mais recentes)
            </label>
            <input
              id={`${idMais}-max`}
              type="number"
              min={1}
              max={MAX_ISSUES_TETO}
              value={j.max_issues}
              onChange={(e) => campo("max_issues", Number(e.target.value))}
            />
          </div>
          <Campo
            rotulo="Configuração do jira-cli"
            valor={j.config_file}
            exemplo="~/.config/.jira/.config.yml"
            mono
            aoMudar={(v) => campo("config_file", v)}
          />
          <div className="field-pair">
            <Campo
              rotulo="Campo do Epic Link"
              valor={j.epic_link_field}
              exemplo="detectado"
              mono
              aoMudar={(v) => campo("epic_link_field", v)}
            />
            <Campo
              rotulo="Campo do Epic Name"
              valor={j.epic_name_field}
              exemplo="detectado"
              mono
              aoMudar={(v) => campo("epic_name_field", v)}
            />
            <Campo
              rotulo="Campo da sprint"
              valor={j.sprint_field}
              exemplo="detectado"
              mono
              aoMudar={(v) => campo("sprint_field", v)}
            />
          </div>
          <div className="field">
            <label htmlFor={`${idMais}-tipos`}>
              Tipos de issue (tipo = epic | issue | bug | chore | ignore)
            </label>
            <textarea
              id={`${idMais}-tipos`}
              rows={4}
              spellCheck={false}
              value={tipos}
              onChange={(e) => {
                setTipos(e.target.value);
                campo("type_map", mapaDeTipos(e.target.value));
              }}
            />
          </div>
          <Campo
            rotulo="Comando de credencial (alternativa ao token)"
            valor={j.credential_cmd}
            exemplo="pass show jira | head -1"
            mono
            aoMudar={(v) => campo("credential_cmd", v)}
          />
        </div>
      </div>
    </Cartao>
  );
}
