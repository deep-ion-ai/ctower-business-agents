import { useRef, useState, type KeyboardEvent } from "react";
import { mensagemDe } from "./analise";
import { DestinoDocumentacao } from "./DestinoDocumentacao";
import type { PropsConfigProjeto } from "./ConfigProjeto";
import {
  CampoRepoGithub,
  CamposGitlab,
  CamposJira,
  ItemRepositorioCodigo,
} from "./FormRepositorios";
import {
  GITHUB_VAZIO,
  GITLAB_VAZIO,
  JIRA_VAZIO,
  REPOSITORIO_CODIGO_VAZIO,
  validarRepositorios,
  type IntegracaoProntidao,
  type RepositorioCodigo,
  type Repositorios,
  type TrackerConfig,
} from "./repositorios";
import {
  FORJA,
  ORIGEM_DEMANDAS,
  ROTULO_SITUACAO_DEMANDAS,
  dataDoTeste,
  resumoDoTeste,
  situacaoDasDemandas,
  falhaDoToken,
  falhou,
  integracaoDasDemandas,
  integracaoDoRepositorio,
  primeiraFalha,
} from "./saudeProjeto";

/** Como a linha do repositório descreve o token que ele usa. */
export function descreverToken(r: RepositorioCodigo): string {
  if (r.token === "proprio") return "próprio deste repositório";
  if (r.token === "comando") return "comando de credencial";
  return `global de ${r.host || "host"}`;
}

type Edicao =
  | { indice: number; valor: RepositorioCodigo }
  | { indice: null; valor: RepositorioCodigo };

/**
 * Aba “Código e demandas” (#192): os repositórios de código, com o acesso conferido
 * sozinho ao abrir a página, de onde vêm as demandas (origem em pílulas, campos
 * essenciais, “Mais opções” e “Testar conexão”) e onde a documentação é publicada
 * (#274).
 */
export function AbaCodigoDemandas(props: PropsConfigProjeto) {
  return (
    <div className="aba-codigo">
      <RepositoriosDeCodigo {...props} />
      <Demandas {...props} />
      <DestinoDocumentacao {...props} />
    </div>
  );
}

function RepositoriosDeCodigo({
  api,
  projeto,
  estado,
  aoMudar,
  aoConferirProntidao,
  aoIrParaAcessos,
}: PropsConfigProjeto) {
  const [edicao, setEdicao] = useState<Edicao | null>(null);
  const [salvando, setSalvando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const codigo = projeto.repositorios.codigo;
  const prontidao = estado.prontidao;

  const salvar = () => {
    if (!edicao) return;
    const novos: Repositorios = {
      ...projeto.repositorios,
      codigo:
        edicao.indice === null
          ? [...codigo, edicao.valor]
          : codigo.map((c, i) => (i === edicao.indice ? edicao.valor : c)),
    };
    const invalido = validarRepositorios(novos);
    if (invalido) {
      setFalha(invalido);
      return;
    }
    setSalvando(true);
    setFalha(null);
    api
      .salvarRepositorios(projeto.id, novos)
      .then(() => {
        setAviso(
          edicao.indice === null
            ? "Repositório adicionado."
            : "Repositório salvo.",
        );
        setEdicao(null);
        aoMudar();
        aoConferirProntidao(projeto.id, true);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setSalvando(false));
  };

  const editor = (rotulo: string, fixo: boolean) =>
    edicao && (
      <div className="repositorio-edicao">
        <ItemRepositorioCodigo
          api={api}
          numero={(edicao.indice ?? codigo.length) + 1}
          nome={rotulo}
          valor={edicao.valor}
          fixo={fixo}
          aoMudar={(valor) => setEdicao({ ...edicao, valor })}
        />
        {falha && (
          <p className="error-text" role="alert">
            {falha}
          </p>
        )}
        <div className="acoes">
          <button
            className="btn"
            type="button"
            disabled={salvando}
            onClick={() => {
              setEdicao(null);
              setFalha(null);
            }}
          >
            Cancelar
          </button>
          <button
            className="btn btn-primary"
            type="button"
            disabled={salvando}
            onClick={salvar}
          >
            {edicao.indice === null ? "Adicionar" : "Salvar"}
          </button>
        </div>
      </div>
    );

  return (
    <section className="cartao-config" aria-label="Repositórios de código">
      <div className="cartao-config-topo">
        <h4>Repositórios de código</h4>
        {codigo.length > 0 && (
          <button
            className="btn btn-sm"
            type="button"
            disabled={prontidao === undefined}
            onClick={() => aoConferirProntidao(projeto.id, true)}
          >
            Conferir de novo
          </button>
        )}
        <button
          className="btn btn-sm"
          type="button"
          disabled={edicao !== null}
          onClick={() => {
            setAviso(null);
            setFalha(null);
            setEdicao({ indice: null, valor: { ...REPOSITORIO_CODIGO_VAZIO } });
          }}
        >
          + Adicionar repositório
        </button>
      </div>
      {codigo.length === 0 && edicao?.indice !== null && (
        <p className="muted linha-config">Nenhum repositório de código.</p>
      )}
      <ul className="lista-config">
        {codigo.map((r, i) => {
          const integ = integracaoDoRepositorio(prontidao, i);
          const comFalha = falhou(integ);
          const orientacao = integ?.passos.find(
            (p) => p.situacao === "falhou",
          )?.orientacao;
          const trocarToken =
            comFalha && r.token === "global" && falhaDoToken(integ);
          if (edicao?.indice === i) {
            return (
              <li key={`${r.host}/${r.caminho}`} className="linha-config">
                {editor(`Editar ${r.caminho}`, true)}
              </li>
            );
          }
          return (
            <li
              key={`${r.host}/${r.caminho}`}
              className="linha-config linha-repositorio"
            >
              <span className="badge">{FORJA[r.forja]}</span>
              <div className="linha-config-texto">
                <span className="caminho">
                  {r.url_clone || `${r.host}/${r.caminho}`}
                </span>
                <span className="muted">
                  Branch <strong>{r.branch || "padrão"}</strong>
                  {r.pasta && (
                    <>
                      {" "}
                      · pasta <span className="caminho">{r.pasta}</span>
                    </>
                  )}{" "}
                  · token {descreverToken(r)}
                </span>
                {comFalha && (
                  <span className="error-text">
                    {primeiraFalha(integ)}
                    {orientacao && <span className="muted"> {orientacao}</span>}
                  </span>
                )}
              </div>
              <span
                className="badge"
                data-tone={
                  prontidao === undefined
                    ? "running"
                    : comFalha
                      ? "error"
                      : integ
                        ? "done"
                        : undefined
                }
              >
                {prontidao === undefined
                  ? "conferindo…"
                  : comFalha
                    ? "✕ sem acesso"
                    : integ
                      ? "✓ acesso ok"
                      : "não conferido"}
              </span>
              {trocarToken ? (
                <button
                  className="btn btn-sm btn-primary"
                  type="button"
                  aria-label={`Trocar o token global de ${r.host}`}
                  onClick={aoIrParaAcessos}
                >
                  Trocar o token
                </button>
              ) : (
                <button
                  className="btn btn-sm"
                  type="button"
                  aria-label={`Editar ${r.caminho}`}
                  disabled={edicao !== null}
                  onClick={() => {
                    setAviso(null);
                    setFalha(null);
                    setEdicao({ indice: i, valor: r });
                  }}
                >
                  Editar
                </button>
              )}
            </li>
          );
        })}
        {edicao?.indice === null && (
          <li className="linha-config">{editor("Repositório novo", false)}</li>
        )}
      </ul>
      {aviso && (
        <p className="ok-text linha-config" role="status">
          {aviso}
        </p>
      )}
      {prontidao === null && (
        <p className="error-text linha-config" role="alert">
          Não foi possível conferir o acesso aos repositórios.
        </p>
      )}
      <p className="muted linha-config">
        O acesso é conferido sozinho ao abrir esta página. Endereço e forja não
        mudam depois de adicionados: eles identificam o repositório nas análises
        já feitas. Branch, pasta e token podem ser trocados em Editar; o token
        global de cada host fica em{" "}
        <button type="button" className="link" onClick={aoIrParaAcessos}>
          Acessos
        </button>
        .
      </p>
    </section>
  );
}

type Origem = "nenhuma" | TrackerConfig["kind"];

const ORIGENS: { id: Origem; rotulo: string }[] = [
  { id: "nenhuma", rotulo: "Nenhuma" },
  { id: "github", rotulo: ORIGEM_DEMANDAS.github },
  { id: "gitlab", rotulo: ORIGEM_DEMANDAS.gitlab },
  { id: "jira", rotulo: ORIGEM_DEMANDAS.jira },
];

function trackerVazio(kind: Origem): TrackerConfig | null {
  if (kind === "github") return { kind, ...GITHUB_VAZIO };
  if (kind === "gitlab") return { kind, ...GITLAB_VAZIO };
  if (kind === "jira") return { kind, ...JIRA_VAZIO };
  return null;
}

const TOM_PASSO = { ok: "done", falhou: "error", pulado: "attention" } as const;

function Demandas({
  api,
  projeto,
  estado,
  aoMudar,
  aoConferirProntidao,
  aoIrParaAcessos,
}: PropsConfigProjeto) {
  const gravado = projeto.repositorios.tracker;
  const [valor, setValor] = useState<TrackerConfig | null>(gravado);
  const [salvando, setSalvando] = useState(false);
  const [testando, setTestando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [teste, setTeste] = useState<{
    integ: IntegracaoProntidao;
    em: number;
  } | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const pilulas = useRef<Partial<Record<Origem, HTMLButtonElement | null>>>({});
  const mudou = JSON.stringify(valor) !== JSON.stringify(gravado);
  const origem: Origem = valor?.kind ?? "nenhuma";
  // O tipo da origem não muda depois de salvo (o cadastro recusa).
  const travada = (o: Origem) => gravado !== null && o !== gravado.kind;
  // O selo reflete o último teste feito nesta página; sem teste, a conferência ao abrir.
  const integ =
    (!mudou && teste?.integ) ||
    integracaoDasDemandas(projeto, estado.prontidao);

  const escolher = (o: Origem) => {
    if (travada(o) || o === origem) return;
    setValor(o === gravado?.kind ? gravado : trackerVazio(o));
    setTeste(null);
    setAviso(null);
  };

  const teclas = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (!["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"].includes(e.key))
      return;
    e.preventDefault();
    const livres = ORIGENS.filter((o) => !travada(o.id));
    const i = livres.findIndex((o) => o.id === origem);
    const passo = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : -1;
    const proxima = livres[(i + passo + livres.length) % livres.length].id;
    escolher(proxima);
    pilulas.current[proxima]?.focus();
  };

  const salvar = () => {
    const novos: Repositorios = { ...projeto.repositorios, tracker: valor };
    const invalido = validarRepositorios(novos);
    if (invalido) {
      setFalha(invalido);
      return;
    }
    setSalvando(true);
    setFalha(null);
    api
      .salvarRepositorios(projeto.id, novos)
      .then(() => {
        setAviso("Demandas salvas.");
        setTeste(null);
        aoMudar();
        aoConferirProntidao(projeto.id, true);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setSalvando(false));
  };

  const testar = () => {
    if (!valor) return;
    setFalha(null);
    setTeste(null);
    setTestando(true);
    const pedido =
      valor.kind === "jira"
        ? api.testarJira(valor, null)
        : api.testarDemandas(valor, null);
    pedido
      .then((r) => setTeste({ integ: r, em: Date.now() }))
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setTestando(false));
  };

  const resultado = teste?.integ ?? (mudou ? undefined : integ);
  const origemTexto = valor ? ORIGEM_DEMANDAS[valor.kind] : "";
  const situacao = integ ? situacaoDasDemandas(integ) : null;
  const hostDoToken =
    valor?.kind === "github"
      ? "github.com"
      : valor?.kind === "gitlab"
        ? valor.host
        : null;

  return (
    <section className="cartao-config" aria-label="Demandas">
      <div className="cartao-config-topo">
        <h4>De onde vêm as demandas</h4>
        {gravado && !mudou && (
          <span
            className="badge"
            data-tone={
              estado.prontidao === undefined && !teste
                ? "running"
                : falhou(integ)
                  ? "error"
                  : integ
                    ? "done"
                    : undefined
            }
          >
            {estado.prontidao === undefined && !teste
              ? "conferindo…"
              : integ && situacao !== "ok"
                ? `✕ ${ROTULO_SITUACAO_DEMANDAS[situacao ?? "falhou"]}`
                : integ
                  ? teste
                    ? `✓ conectado · testado em ${dataDoTeste(teste.em)}`
                    : "✓ conectado"
                  : "não conferido"}
          </span>
        )}
      </div>
      <div className="linha-config demandas-corpo">
        <div
          className="pilulas"
          role="radiogroup"
          aria-label="Origem das demandas"
        >
          {ORIGENS.map((o) => (
            <button
              key={o.id}
              ref={(el) => {
                pilulas.current[o.id] = el;
              }}
              type="button"
              role="radio"
              className="pilula"
              aria-checked={origem === o.id}
              tabIndex={origem === o.id ? 0 : -1}
              disabled={travada(o.id)}
              onClick={() => escolher(o.id)}
              onKeyDown={teclas}
            >
              {o.rotulo}
            </button>
          ))}
        </div>
        {gravado && (
          <p className="muted">
            A origem não muda depois de salva: as demandas já trazidas dependem
            dela.
          </p>
        )}
        {valor?.kind === "github" && (
          <CampoRepoGithub
            rotulo="Repositório das issues (dono/repositório)"
            valor={valor}
            aoMudar={(g) => setValor({ kind: "github", ...g })}
          />
        )}
        {valor?.kind === "gitlab" && (
          <CamposGitlab
            parte="essenciais"
            valor={valor}
            fixo={gravado?.kind === "gitlab"}
            aoMudar={(g) => setValor({ kind: "gitlab", ...g })}
          />
        )}
        {valor?.kind === "jira" && (
          <CamposJira
            parte="essenciais"
            valor={valor}
            aoMudar={(j) => setValor({ kind: "jira", ...j })}
          />
        )}
        {valor && (
          <div className="acoes">
            <button
              className="btn btn-sm"
              type="button"
              disabled={testando}
              onClick={testar}
            >
              Testar conexão
            </button>
            {hostDoToken && (
              <span className="muted">
                Token: global de {hostDoToken} (em{" "}
                <button
                  type="button"
                  className="link"
                  onClick={aoIrParaAcessos}
                >
                  Acessos
                </button>
                )
              </span>
            )}
          </div>
        )}
        {testando && <p className="muted">Testando a conexão…</p>}
        {teste && (
          <p
            role="status"
            className={falhou(teste.integ) ? "error-text" : "ok-text"}
            data-testid="retorno-teste"
          >
            {resumoDoTeste(teste.integ, origemTexto)}{" "}
            <span className="muted">Testado em {dataDoTeste(teste.em)}.</span>
          </p>
        )}
        {resultado && (teste || falhou(resultado)) && (
          <ul className="passos-prontidao" aria-label="Resultado do teste">
            {resultado.passos.map((p) => (
              <li key={p.id}>
                <span className="badge" data-tone={TOM_PASSO[p.situacao]}>
                  {p.situacao === "ok"
                    ? "ok"
                    : p.situacao === "falhou"
                      ? "falhou"
                      : "não conferido"}
                </span>{" "}
                {p.rotulo}
                {p.detalhe && <span className="muted"> · {p.detalhe}</span>}
                {p.orientacao && p.situacao === "falhou" && (
                  <span className="muted"> {p.orientacao}</span>
                )}
              </li>
            ))}
          </ul>
        )}
        {valor && valor.kind !== "github" && (
          <details className="mais-opcoes">
            <summary>
              {valor.kind === "jira"
                ? "Mais opções (JQL, campos, tipos)"
                : "Mais opções (URL de clone, credencial)"}
            </summary>
            {valor.kind === "jira" ? (
              <CamposJira
                parte="mais"
                valor={valor}
                aoMudar={(j) => setValor({ kind: "jira", ...j })}
              />
            ) : (
              <CamposGitlab
                parte="mais"
                valor={valor}
                fixo={gravado?.kind === "gitlab"}
                aoMudar={(g) => setValor({ kind: "gitlab", ...g })}
              />
            )}
          </details>
        )}
        {falha && (
          <p className="error-text" role="alert">
            {falha}
          </p>
        )}
        {aviso && !mudou && (
          <p className="ok-text" role="status">
            {aviso}
          </p>
        )}
        {mudou && (
          <div className="acoes">
            <button
              className="btn"
              type="button"
              disabled={salvando}
              onClick={() => {
                setValor(gravado);
                setFalha(null);
                setTeste(null);
              }}
            >
              Cancelar
            </button>
            <button
              className="btn btn-primary"
              type="button"
              disabled={salvando}
              onClick={salvar}
            >
              Salvar as demandas
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
