import { useEffect, useState, type ClipboardEvent } from "react";
import { mensagemDe } from "./analise";
import { Alerta, BotaoIcone, Cartao, ItemLista, ListaItens, Selo } from "./componentes";
import { IconeDescartar } from "./icones";
import {
  ROTULO_FORJA,
  clonesPreenchidos,
  hostsDistintos,
  linhas,
  nomeDaPasta,
  novoRepoClone,
  pastaDoClone,
  plural,
  repositoriosDetectados,
  sugestaoDeNome,
  todosClonados,
  type ApiAssistenteProjeto,
  type EstadoCodigo,
  type OrigemCodigo,
  type RepoClone,
} from "./estadoCadastro";
import { useTokensGuardados } from "./useTokensGuardados";
import {
  REPOSITORIO_CODIGO_VAZIO,
  repositorioDaUrl,
  type OrigemToken,
  type ProgressoClone,
  type RepositorioCodigo,
} from "./repositorios";

type Api = Pick<
  ApiAssistenteProjeto,
  | "escolherPasta"
  | "detectarRepositorios"
  | "pastaClones"
  | "tokensGuardados"
  | "salvarToken"
  | "testarAcesso"
>;

type Mudar = (f: (e: EstadoCodigo) => EstadoCodigo) => void;

const ORIGENS: { id: OrigemCodigo; nome: string; descricao: string }[] = [
  {
    id: "clonar",
    nome: "Clonar do GitHub ou GitLab",
    descricao: "Informe as URLs e onde guardar. O app faz o clone; atualizar fica com você.",
  },
  {
    id: "pasta",
    nome: "Usar uma pasta que já tenho",
    descricao: "Um clone feito por você. O app não mexe nele.",
  },
];

/**
 * Passo 1 do cadastro: de onde vem o código. Clonar do GitHub ou GitLab (URLs, branch,
 * pasta dos clones, tokens globais por host ou só do repositório, progresso do clone) ou
 * usar uma pasta existente (com os repositórios git achados nela). Sugere o nome.
 */
export function PassoCodigo({
  api,
  codigo,
  mudar,
  clonando,
  cancelando,
  nomeEmUso,
  aviso,
  aoTentarDeNovo,
  aoCancelarClone,
}: {
  api: Api;
  codigo: EstadoCodigo;
  mudar: Mudar;
  clonando: boolean;
  /** O cancelamento do clone foi pedido e o git está sendo interrompido. */
  cancelando: boolean;
  /** Já existe um projeto com o nome do campo. */
  nomeEmUso: boolean;
  /** Recado do último clone (cancelado, por exemplo). */
  aviso: string | null;
  aoTentarDeNovo: (chave: number) => void;
  aoCancelarClone: () => void;
}) {
  const carregarPastaPadrao = codigo.pastaPadrao === null;
  useEffect(() => {
    if (!carregarPastaPadrao) return;
    let ativo = true;
    Promise.resolve()
      .then(() => api.pastaClones())
      .then((p) => {
        if (!ativo) return;
        mudar((e) => ({ ...e, pastaPadrao: p ?? "", pastaBase: e.pastaBase || p || "" }));
      })
      .catch(() => ativo && mudar((e) => ({ ...e, pastaPadrao: "" })));
    return () => {
      ativo = false;
    };
  }, [api, mudar, carregarPastaPadrao]);

  return (
    <>
      <fieldset className="opcoes-cartao">
        <legend className="sr-only">Origem do código</legend>
        {ORIGENS.map((o) => (
          <label key={o.id} className={codigo.origem === o.id ? "opcao escolhida" : "opcao"}>
            <input
              type="radio"
              name="origem-codigo"
              value={o.id}
              checked={codigo.origem === o.id}
              disabled={clonando}
              onChange={() => mudar((e) => ({ ...e, origem: o.id }))}
            />
            <strong>{o.nome}</strong>
            <span className="muted">{o.descricao}</span>
          </label>
        ))}
      </fieldset>
      {codigo.origem === "pasta" ? (
        <PastaExistente api={api} codigo={codigo} mudar={mudar} nomeEmUso={nomeEmUso} />
      ) : (
        <Clonar
          api={api}
          codigo={codigo}
          mudar={mudar}
          clonando={clonando}
          cancelando={cancelando}
          nomeEmUso={nomeEmUso}
          aviso={aviso}
          aoTentarDeNovo={aoTentarDeNovo}
          aoCancelarClone={aoCancelarClone}
        />
      )}
    </>
  );
}

function CampoNome({
  codigo,
  mudar,
  dica,
  nomeEmUso,
}: {
  codigo: EstadoCodigo;
  mudar: Mudar;
  dica: string;
  nomeEmUso: boolean;
}) {
  const nome = codigo.nomeTocado ? codigo.nome : sugestaoDeNome(codigo);
  return (
    <div className="field">
      <label htmlFor="nome-projeto">Nome do projeto</label>
      <input
        id="nome-projeto"
        type="text"
        value={nome}
        placeholder="Ex.: Faturamento"
        aria-describedby={nomeEmUso ? "erro-nome-projeto dica-nome-projeto" : "dica-nome-projeto"}
        aria-invalid={nomeEmUso || undefined}
        onChange={(ev) => {
          const nome = ev.target.value;
          mudar((e) => ({ ...e, nome, nomeTocado: true }));
        }}
      />
      {nomeEmUso && (
        <span id="erro-nome-projeto" className="error-text" role="alert">
          Já existe um projeto cadastrado com o nome “{nome.trim()}”. Use outro nome.
        </span>
      )}
      <span id="dica-nome-projeto" className="dica">
        {dica}
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Pasta existente

function PastaExistente({
  api,
  codigo,
  mudar,
  nomeEmUso,
}: {
  api: Api;
  codigo: EstadoCodigo;
  mudar: Mudar;
  nomeEmUso: boolean;
}) {
  const [falha, setFalha] = useState<string | null>(null);

  const escolher = () => {
    setFalha(null);
    api
      .escolherPasta("Pasta do código do projeto", codigo.pasta ?? undefined)
      .then((pasta) => {
        if (!pasta) return;
        mudar((e) => ({ ...e, pasta, detectados: null }));
        return api
          .detectarRepositorios(pasta)
          .catch(() => [])
          .then((lista) => mudar((e) => (e.pasta === pasta ? { ...e, detectados: lista } : e)));
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  if (!codigo.pasta) {
    return (
      <div className="pasta-existente">
        <button type="button" className="escolher-pasta-grande" onClick={escolher}>
          <span className="titulo-botao">Escolher a pasta do código…</span>
          <span className="muted">
            Pode ser a pasta de um repositório ou uma pasta com vários clones dentro
          </span>
        </button>
        {falha && (
          <Alerta tom="erro" titulo="Não deu para escolher a pasta.">
            {falha}
          </Alerta>
        )}
      </div>
    );
  }

  const detectados = codigo.detectados;
  return (
    <div className="pasta-existente">
      <Cartao como="div">
        <div className="linha-pasta-escolhida">
          <code className="caminho-escolhido">{codigo.pasta}</code>
          <button
            type="button"
            className="btn btn-sm"
            aria-label="Trocar a pasta do código"
            onClick={escolher}
          >
            Trocar
          </button>
        </div>
        <div role="status" aria-live="polite">
          {detectados === null ? (
            <span className="muted">Procurando repositórios git na pasta…</span>
          ) : detectados.length === 0 ? (
            <span className="muted">
              Não encontrei repositórios git nela. O app lê a pasta do mesmo jeito.
            </span>
          ) : (
            <span className="dica">
              Encontrei {plural(detectados.length, "repositório git", "repositórios git")} dentro
              dela:
            </span>
          )}
        </div>
        {detectados && detectados.length > 0 && (
          <ListaItens rotulo="Repositórios git encontrados">
            {detectados.map((d) => (
              <ItemLista key={d.caminho}>
                <span className="linha-repo-lido">
                  <code>{nomeDaPasta(d.caminho)}/</code>
                  {d.forja && d.host ? (
                    <Selo tom="neutro">
                      {ROTULO_FORJA[d.forja]} · {d.host}
                    </Selo>
                  ) : (
                    <Selo tom="neutro">sem remoto conhecido</Selo>
                  )}
                  <span className="muted">
                    {d.branch ? `branch ${d.branch}` : "sem branch"}
                    {d.caminhoRepo ? ` · ${d.caminhoRepo}` : ""}
                  </span>
                </span>
              </ItemLista>
            ))}
          </ListaItens>
        )}
        {detectados !== null &&
          repositoriosDetectados(detectados).length < detectados.length &&
          detectados.length > 0 && (
            <p className="dica">
              Os sem remoto do GitHub ou do GitLab entram só como pastas do código.
            </p>
          )}
      </Cartao>
      {falha && (
        <Alerta tom="erro" titulo="Não deu para escolher a pasta.">
          {falha}
        </Alerta>
      )}
      <CampoNome
        codigo={codigo}
        mudar={mudar}
        nomeEmUso={nomeEmUso}
        dica="Sugerido pelo nome da pasta. É como o projeto aparece no app."
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Clonar

/** Como o token do repositório aparece: o global só se há token guardado para o host. */
function textoToken(r: RepositorioCodigo, guardados: Record<string, boolean | null | undefined>) {
  if (r.token === "proprio") return "token próprio";
  if (r.token === "comando") return "token por comando";
  const g = guardados[r.host.trim().toLowerCase()];
  if (g === false) return "sem token";
  if (g === undefined) return "verificando o token…";
  return `token global de ${r.host}`;
}

function Clonar({
  api,
  codigo,
  mudar,
  clonando,
  cancelando,
  nomeEmUso,
  aviso,
  aoTentarDeNovo,
  aoCancelarClone,
}: {
  api: Api;
  codigo: EstadoCodigo;
  mudar: Mudar;
  clonando: boolean;
  cancelando: boolean;
  nomeEmUso: boolean;
  aviso: string | null;
  aoTentarDeNovo: (chave: number) => void;
  aoCancelarClone: () => void;
}) {
  const [focar, setFocar] = useState<number | null>(null);
  const preenchidos = clonesPreenchidos(codigo);
  const guardados = useTokensGuardados(
    api,
    hostsDistintos(preenchidos.map((c) => c.repo).filter((r) => r.caminho && r.token === "global")),
  );
  const dicaNome = "Sugerido pelo primeiro repositório. É como o projeto aparece no app.";

  if (todosClonados(codigo) && !codigo.editandoClones) {
    return (
      <div className="clonar">
        <Cartao rotulo="Repositórios clonados">
          <div className="linha-resumo-clone">
            <span className="ok-text" aria-hidden="true">
              ✓
            </span>
            <span>
              {plural(preenchidos.length, "repositório clonado", "repositórios clonados")} em{" "}
              <code>{codigo.pastaBase}</code>
            </span>
            <button
              type="button"
              className="btn btn-sm"
              onClick={() => mudar((e) => ({ ...e, editandoClones: true }))}
            >
              Alterar repositórios
            </button>
          </div>
          <ListaItens>
            {preenchidos.map((c) => (
              <ItemLista key={c.chave}>
                <span className="linha-repo-lido">
                  <code>{nomeDaPasta(c.repo.pasta ?? "")}/</code>
                  <Selo tom="neutro">{ROTULO_FORJA[c.repo.forja]}</Selo>
                  <span className="muted">
                    {c.repo.branch ? `branch ${c.repo.branch}` : "branch padrão"} ·{" "}
                    {textoToken(c.repo, guardados)}
                  </span>
                </span>
              </ItemLista>
            ))}
          </ListaItens>
        </Cartao>
        <CampoNome codigo={codigo} mudar={mudar} dica={dicaNome} nomeEmUso={nomeEmUso} />
      </div>
    );
  }

  const mudarClone = (chave: number, f: (c: RepoClone) => RepoClone) =>
    mudar((e) => ({ ...e, clones: e.clones.map((c) => (c.chave === chave ? f(c) : c)) }));

  const colarVarias = (chave: number, urls: string[]) =>
    mudar((e) => {
      const i = e.clones.findIndex((c) => c.chave === chave);
      if (i < 0) return e;
      const [primeira, ...resto] = urls.map((u) => novoRepoClone(u));
      const atual = e.clones[i]!;
      const trocado: RepoClone = {
        ...atual,
        repo: { ...primeira!.repo, branch: atual.repo.branch, token: atual.repo.token },
        progresso: null,
      };
      return {
        ...e,
        clones: [...e.clones.slice(0, i), trocado, ...resto, ...e.clones.slice(i + 1)],
      };
    });

  const adicionar = () => {
    const novo = novoRepoClone();
    setFocar(novo.chave);
    mudar((e) => ({ ...e, clones: [...e.clones, novo] }));
  };

  const remover = (chave: number) =>
    mudar((e) => {
      const resto = e.clones.filter((c) => c.chave !== chave);
      return { ...e, clones: resto.length > 0 ? resto : [novoRepoClone()] };
    });

  const escolherBase = () => {
    api
      .escolherPasta("Onde guardar os clones", codigo.pastaBase.trim() || undefined)
      .then((p) => p && mudar((e) => ({ ...e, pastaBase: p })))
      .catch(() => undefined);
  };

  return (
    <div className="clonar">
      <fieldset className="sem-borda" disabled={clonando}>
        <legend className="subtitulo-passo">Repositórios</legend>
        <ul className="cartoes-repo">
          {codigo.clones.map((c, i) => (
            <CartaoRepo
              key={c.chave}
              api={api}
              clone={c}
              indice={i}
              focar={focar === c.chave}
              aoMudar={(f) => mudarClone(c.chave, f)}
              aoColarVarias={(urls) => colarVarias(c.chave, urls)}
              aoRemover={() => remover(c.chave)}
              aoTentarDeNovo={() => aoTentarDeNovo(c.chave)}
              clonando={clonando}
            />
          ))}
        </ul>
        <p>
          <button type="button" className="link" onClick={adicionar}>
            + Adicionar repositório
          </button>{" "}
          <span className="dica">ou cole várias URLs, uma por linha, no campo da URL</span>
        </p>
      </fieldset>

      <fieldset className="sem-borda" disabled={clonando}>
        <legend className="subtitulo-passo">Onde guardar os clones</legend>
        <div className="linha-campo-botao">
          <div className="field">
            <label htmlFor="pasta-clones">Pasta dos repositórios</label>
            <input
              id="pasta-clones"
              type="text"
              className="caminho"
              spellCheck={false}
              value={codigo.pastaBase}
              aria-describedby="dica-pasta-clones"
              onChange={(ev) => {
                const pastaBase = ev.target.value;
                mudar((e) => ({ ...e, pastaBase }));
              }}
            />
          </div>
          <button type="button" className="btn" onClick={escolherBase}>
            Escolher pasta…
          </button>
        </div>
        <span id="dica-pasta-clones" className="dica">
          Cada repositório vira uma subpasta. O padrão vem de Configurações e pode ser trocado
          aqui.
        </span>
      </fieldset>

      <TokensGlobais api={api} codigo={codigo} mudar={mudar} desabilitado={clonando} />

      {clonando && (
        <div className="acoes-clone">
          <button type="button" className="btn" disabled={cancelando} onClick={aoCancelarClone}>
            {cancelando ? "Cancelando…" : "Cancelar clone"}
          </button>
          <span className="dica">
            Interrompe o git e apaga a pasta que este clone criou. O que já estava pronto fica.
          </span>
        </div>
      )}
      {aviso && !clonando && (
        <p role="status" className="muted">
          {aviso}
        </p>
      )}

      <CampoNome codigo={codigo} mudar={mudar} dica={dicaNome} nomeEmUso={nomeEmUso} />
    </div>
  );
}

const MODOS_TOKEN: { valor: Exclude<OrigemToken, "comando">; rotulo: string }[] = [
  { valor: "global", rotulo: "Global do host" },
  { valor: "proprio", rotulo: "Só deste repositório" },
];

function CartaoRepo({
  api,
  clone,
  indice,
  focar,
  aoMudar,
  aoColarVarias,
  aoRemover,
  aoTentarDeNovo,
  clonando,
}: {
  api: Api;
  clone: RepoClone;
  indice: number;
  focar: boolean;
  aoMudar: (f: (c: RepoClone) => RepoClone) => void;
  aoColarVarias: (urls: string[]) => void;
  aoRemover: () => void;
  aoTentarDeNovo: () => void;
  clonando: boolean;
}) {
  const [teste, setTeste] = useState<{ ok: boolean; texto: string } | null>(null);
  const [testando, setTestando] = useState(false);
  const r = clone.repo;
  const n = indice + 1;
  const id = `repo-${clone.chave}`;
  const nome = r.caminho ? pastaDoClone(r) : `repositório ${n}`;

  const mudarRepo = (p: Partial<RepositorioCodigo>, manterClone = true) =>
    aoMudar((c) => ({
      ...c,
      repo: { ...c.repo, ...p, pasta: manterClone ? c.repo.pasta : null },
      progresso: manterClone ? c.progresso : null,
    }));

  const mudarUrl = (url: string) => {
    const lido = repositorioDaUrl(url);
    aoMudar((c) => ({
      ...c,
      repo: lido
        ? { ...lido, branch: c.repo.branch, token: c.repo.token, comando_credencial: c.repo.comando_credencial }
        : { ...REPOSITORIO_CODIGO_VAZIO, ...c.repo, url_clone: url, caminho: "", pasta: null },
      progresso: null,
    }));
  };

  const colar = (ev: ClipboardEvent<HTMLInputElement>) => {
    const urls = linhas(ev.clipboardData.getData("text"));
    if (urls.length > 1) {
      ev.preventDefault();
      aoColarVarias(urls);
    }
  };

  const testar = () => {
    setTestando(true);
    setTeste(null);
    api
      .testarAcesso(r, clone.token.trim() || null)
      .then(() => setTeste({ ok: true, texto: "Acesso OK." }))
      .catch((e: unknown) => setTeste({ ok: false, texto: mensagemDe(e) }))
      .finally(() => setTestando(false));
  };

  const urlInvalida = r.url_clone.trim() !== "" && !r.caminho;

  return (
    <Cartao como="li" rotulo={`Repositório ${n}`}>
      <div className="linha-repo">
        <div className="field campo-url">
          <label htmlFor={`${id}-url`}>URL do repositório {n}</label>
          <input
            id={`${id}-url`}
            type="text"
            className="caminho"
            spellCheck={false}
            autoFocus={focar}
            value={r.url_clone}
            placeholder="https://github.com/dono/repo.git"
            aria-describedby={`${id}-chip`}
            onChange={(ev) => mudarUrl(ev.target.value)}
            onPaste={colar}
          />
        </div>
        <div className="field campo-branch">
          <label htmlFor={`${id}-branch`}>Branch (opcional)</label>
          <input
            id={`${id}-branch`}
            type="text"
            className="caminho"
            spellCheck={false}
            placeholder="padrão"
            value={r.branch}
            onChange={(ev) => mudarRepo({ branch: ev.target.value }, false)}
          />
        </div>
        <BotaoIcone rotulo={`Remover o repositório ${n}`} dica="Remover" tom="perigo" onClick={aoRemover}>
          <IconeDescartar />
        </BotaoIcone>
      </div>
      <div className="linha-chip" id={`${id}-chip`}>
        {r.caminho ? (
          <>
            <Selo tom="neutro">
              {ROTULO_FORJA[r.forja]} · {r.host}
            </Selo>
            <span className="dica">
              — vira a pasta <code>{pastaDoClone(r)}</code>
            </span>
          </>
        ) : urlInvalida ? (
          <span className="error-text">
            Não reconheci esta URL. Use HTTPS ou SSH, como https://github.com/dono/repo.git.
          </span>
        ) : (
          <span className="dica">Cole a URL de clone (HTTPS ou SSH).</span>
        )}
      </div>

      {r.token === "comando" ? (
        <div className="linha-campo-botao">
          <div className="field">
            <label htmlFor={`${id}-cmd`}>Comando que imprime o token de {nome}</label>
            <input
              id={`${id}-cmd`}
              type="text"
              className="caminho"
              spellCheck={false}
              placeholder="pass show gitlab | head -1"
              value={r.comando_credencial}
              onChange={(ev) => mudarRepo({ comando_credencial: ev.target.value })}
            />
          </div>
          <button
            type="button"
            className="link"
            onClick={() => mudarRepo({ token: "global", comando_credencial: "" })}
          >
            Usar o token do chaveiro
          </button>
        </div>
      ) : (
        <fieldset className="segmentado">
          <legend>Token de {nome}</legend>
          {MODOS_TOKEN.map((m) => (
            <label key={m.valor} className={r.token === m.valor ? "escolhido" : undefined}>
              <input
                type="radio"
                className="sr-only"
                name={`${id}-token`}
                value={m.valor}
                checked={r.token === m.valor}
                onChange={() => mudarRepo({ token: m.valor })}
              />
              {m.rotulo}
            </label>
          ))}
        </fieldset>
      )}

      {r.token === "proprio" && (
        <div className="token-proprio">
          <div className="linha-campo-botao">
            <div className="field">
              <label htmlFor={`${id}-pat`}>Token de acesso deste repositório (PAT)</label>
              <input
                id={`${id}-pat`}
                type="password"
                className="caminho"
                autoComplete="off"
                spellCheck={false}
                value={clone.token}
                onChange={(ev) => {
                  const token = ev.target.value;
                  aoMudar((c) => ({ ...c, token }));
                }}
              />
            </div>
            <button
              type="button"
              className="btn"
              disabled={!r.caminho || testando}
              onClick={testar}
            >
              Testar acesso
            </button>
          </div>
          <span className="dica">
            Use quando este repositório pede outro acesso que não o do host (outro grupo, conta de
            serviço). Basta permissão de leitura. Fica no chaveiro do sistema ao clonar.
          </span>
          <div role="status" aria-live="polite">
            {testando && <span className="muted">Testando o acesso…</span>}
            {teste?.ok && <span className="ok-text">{teste.texto}</span>}
            {teste && !teste.ok && (
              <Alerta tom="erro" titulo="Sem acesso ao repositório.">
                {teste.texto}
              </Alerta>
            )}
          </div>
        </div>
      )}

      {clone.progresso && (
        <ProgressoDoClone
          nome={nome}
          progresso={clone.progresso}
          aoTentarDeNovo={aoTentarDeNovo}
          clonando={clonando}
        />
      )}
    </Cartao>
  );
}

function ProgressoDoClone({
  nome,
  progresso,
  aoTentarDeNovo,
  clonando,
}: {
  nome: string;
  progresso: ProgressoClone;
  aoTentarDeNovo: () => void;
  clonando: boolean;
}) {
  const pct =
    progresso.situacao === "concluido"
      ? 100
      : progresso.percentual === null
        ? null
        : Math.max(0, Math.min(100, Math.round(progresso.percentual)));
  const texto =
    progresso.situacao === "aguardando"
      ? "Na fila"
      : progresso.situacao === "concluido"
        ? "Clonado"
        : progresso.situacao === "falhou"
          ? "Falhou"
          : `${progresso.etapa ?? "Clonando"}${pct !== null ? ` · ${pct}%` : "…"}`;
  return (
    <div className="progresso-clone" data-situacao={progresso.situacao}>
      <div className="linha-progresso">
        <div
          className="barra-progresso"
          role="progressbar"
          aria-label={`Clone de ${nome}`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={pct ?? undefined}
          aria-valuetext={texto}
        >
          <div className="preenchimento" style={{ width: `${pct ?? 0}%` }} />
        </div>
        <span className="muted texto-progresso">{texto}</span>
      </div>
      {progresso.situacao === "falhou" && (
        <Alerta
          tom="erro"
          titulo={`Não deu para clonar ${nome}:`}
          acoes={
            <button
              type="button"
              className="btn btn-sm"
              disabled={clonando}
              aria-label={`Tentar de novo o clone de ${nome}`}
              onClick={aoTentarDeNovo}
            >
              Tentar de novo
            </button>
          }
        >
          {progresso.erro}
        </Alerta>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Tokens globais

function TokensGlobais({
  api,
  codigo,
  mudar,
  desabilitado,
}: {
  api: Api;
  codigo: EstadoCodigo;
  mudar: Mudar;
  desabilitado: boolean;
}) {
  const repos = clonesPreenchidos(codigo)
    .map((c) => c.repo)
    .filter((r) => r.caminho);
  const hosts = hostsDistintos(repos);
  const chaveHosts = hosts.join(" ");
  const [guardados, setGuardados] = useState<Record<string, boolean>>({});
  const [vez, setVez] = useState(0);

  useEffect(() => {
    const lista = chaveHosts ? chaveHosts.split(" ") : [];
    if (lista.length === 0) return;
    let ativo = true;
    // Espera a digitação das URLs assentar antes de consultar o chaveiro.
    const t = setTimeout(() => {
      Promise.resolve()
        .then(() => api.tokensGuardados(lista.map((host) => ({ host, caminho: null }))))
        .then((r) => {
          if (!ativo) return;
          setGuardados(Object.fromEntries(lista.map((h, i) => [h, r?.[i] === true])));
        })
        .catch(() => undefined);
    }, 150);
    return () => {
      ativo = false;
      clearTimeout(t);
    };
  }, [api, chaveHosts, vez]);

  if (hosts.length === 0) return null;

  const usarComando = (host: string) =>
    mudar((e) => ({
      ...e,
      clones: e.clones.map((c) =>
        c.repo.host.toLowerCase() === host && c.repo.token === "global"
          ? { ...c, repo: { ...c.repo, token: "comando" } }
          : c,
      ),
    }));

  return (
    <fieldset className="sem-borda" disabled={desabilitado}>
      <legend className="subtitulo-passo">Tokens globais</legend>
      <p className="dica">
        Um token por host vale para todos os repositórios dele, neste e nos próximos projetos.
        Fica no chaveiro do sistema, nunca na base.
      </p>
      <Cartao como="div">
        <ListaItens rotulo="Tokens por host">
          {hosts.map((host) => {
            const deste = repos.filter((r) => r.host.toLowerCase() === host);
            const usam = deste.filter((r) => r.token === "global");
            return (
              <TokenDoHost
                key={host}
                api={api}
                host={host}
                guardado={guardados[host]}
                usadoPor={usam.map(pastaDoClone)}
                semUso={deste.map(pastaDoClone)}
                aoSalvar={() => setVez((v) => v + 1)}
                aoPreferirComando={usam.length > 0 ? () => usarComando(host) : undefined}
              />
            );
          })}
        </ListaItens>
        <p className="dica">
          Escopo mínimo: <code>read_repository</code> no GitLab, <code>Contents: read</code> no
          GitHub.
        </p>
      </Cartao>
    </fieldset>
  );
}

function TokenDoHost({
  api,
  host,
  guardado,
  usadoPor,
  semUso,
  aoSalvar,
  aoPreferirComando,
}: {
  api: Api;
  host: string;
  guardado: boolean | undefined;
  usadoPor: string[];
  semUso: string[];
  aoSalvar: () => void;
  aoPreferirComando?: () => void;
}) {
  const [token, setToken] = useState("");
  const [trocando, setTrocando] = useState(false);
  const [salvando, setSalvando] = useState(false);
  const [aviso, setAviso] = useState<{ ok: boolean; texto: string } | null>(null);
  const id = `token-host-${host.replace(/[^a-z0-9]/g, "-")}`;

  const salvar = () => {
    if (!token.trim()) return;
    setSalvando(true);
    setAviso(null);
    api
      .salvarToken({ host, caminho: null }, token.trim())
      .then(() => {
        setToken("");
        setTrocando(false);
        setAviso({ ok: true, texto: `Token de ${host} guardado no chaveiro.` });
        aoSalvar();
      })
      .catch((e: unknown) => setAviso({ ok: false, texto: mensagemDe(e) }))
      .finally(() => setSalvando(false));
  };

  const uso =
    usadoPor.length > 0
      ? `usado por ${usadoPor.join(", ")}`
      : `sem uso aqui: ${semUso.join(", ")} ${semUso.length === 1 ? "tem" : "têm"} token próprio ou comando`;

  return (
    <ItemLista>
      <div className="linha-token">
        <strong className="mono">{host}</strong>
        {guardado && !trocando ? (
          <>
            <Selo tom="concluido">configurado</Selo>
            <span className="dica">{uso}</span>
            <button
              type="button"
              className="btn btn-sm"
              aria-label={`Trocar o token de ${host}`}
              onClick={() => setTrocando(true)}
            >
              Trocar
            </button>
          </>
        ) : (
          <>
            <Selo tom={guardado === undefined ? "neutro" : "atencao"}>
              {guardado === undefined ? "verificando…" : guardado ? "trocar token" : "sem token"}
            </Selo>
            <input
              id={id}
              type="password"
              className="caminho"
              autoComplete="off"
              spellCheck={false}
              placeholder="cole o token"
              aria-label={`Token global de ${host}`}
              value={token}
              onChange={(ev) => setToken(ev.target.value)}
              onKeyDown={(ev) => {
                if (ev.key === "Enter") {
                  ev.preventDefault();
                  salvar();
                }
              }}
            />
            <button
              type="button"
              className="btn btn-sm btn-primary"
              aria-label={`Salvar o token de ${host}`}
              disabled={!token.trim() || salvando}
              onClick={salvar}
            >
              Salvar
            </button>
          </>
        )}
        {aoPreferirComando && (
          <button
            type="button"
            className="link"
            aria-label={`Prefiro um comando que imprime o token de ${host}`}
            onClick={aoPreferirComando}
          >
            Prefiro um comando que imprime o token
          </button>
        )}
        <div className="aviso-token" role="status" aria-live="polite">
          {aviso?.ok && <span className="ok-text">{aviso.texto}</span>}
          {aviso && !aviso.ok && (
            <Alerta tom="erro" titulo={`Não deu para guardar o token de ${host}.`}>
              {aviso.texto}
            </Alerta>
          )}
          {!aviso && guardado === false && host === "github.com" && (
            <span className="dica">Sem token, vale o login do gh (gh auth), se houver.</span>
          )}
        </div>
      </div>
    </ItemLista>
  );
}

