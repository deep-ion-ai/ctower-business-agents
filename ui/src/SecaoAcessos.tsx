import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  descreverUsos,
  hostsDosProjetos,
  repositoriosComTokenProprio,
  situacaoDoToken,
  type ApiAcessos,
  type EstadoToken,
  type HostAcesso,
  type HostCadastrado,
  type RepositorioTokenProprio,
  type SituacaoToken,
} from "./acessos";
import { mensagemDe } from "./analise";
import type { Configuracoes } from "./configuracoes";
import { DialogoConfirmacao } from "./DialogoConfirmacao";
import type { AlvoToken, ApiRepositorios, ForgeKind } from "./repositorios";
import "./acessos.css";

export type ApiSecaoAcessos = ApiAcessos & Pick<ApiRepositorios, "salvarToken">;

const FORJA: Record<ForgeKind, string> = { github: "GitHub", gitlab: "GitLab" };

/** Chave de um alvo no mapa de estados: `host` ou `host/caminho`, em minúsculas. */
function chaveDoAlvo(a: AlvoToken): string {
  const c = a.caminho?.trim().toLowerCase();
  const h = a.host.trim().toLowerCase();
  return c ? `${h}/${c}` : h;
}

/**
 * Acessos ao GitHub e ao GitLab (#193): os tokens (PAT) globais de cada host (quem os
 * usa, o último teste e a validade, Testar, Trocar e Remover), os repositórios com
 * token próprio (que têm precedência sobre o do host) e a alternativa de um comando
 * que imprime o token. O token só é digitado: nunca volta para a tela.
 *
 * Trocar o token do host vale na hora para todos os repositórios que o usam: o app
 * lê o token do chaveiro a cada uso.
 */
export function SecaoAcessos({
  api,
  cfg,
  aoAbrirProjeto,
  agora = Date.now,
}: {
  api: ApiSecaoAcessos;
  cfg: Pick<Configuracoes, "projetos">;
  /** Abre a página do projeto (atalho dos repositórios com token próprio). */
  aoAbrirProjeto?: (projetoId: string) => void;
  /** Relógio (milissegundos), para os testes. */
  agora?: () => number;
}) {
  const [extras, setExtras] = useState<HostCadastrado[]>([]);
  const [estados, setEstados] = useState<Record<string, EstadoToken>>({});
  const [falha, setFalha] = useState<string | null>(null);
  const [adicionando, setAdicionando] = useState(false);
  const [recarga, setRecarga] = useState(0);
  const reler = useCallback(() => setRecarga((r) => r + 1), []);

  const hosts = useMemo(() => hostsDosProjetos(cfg, extras), [cfg, extras]);
  const proprios = useMemo(() => repositoriosComTokenProprio(cfg), [cfg]);
  const alvos = useMemo<AlvoToken[]>(
    () => [
      ...hosts.map((h) => ({ host: h.host, caminho: null })),
      ...proprios.map((p) => ({
        host: p.repositorio.host,
        caminho: p.repositorio.caminho,
      })),
    ],
    [hosts, proprios],
  );
  const chaveAlvos = alvos.map(chaveDoAlvo).join(" ");

  useEffect(() => {
    let ativo = true;
    Promise.resolve()
      .then(() => api.hostsAcessos())
      .then((h) => ativo && setExtras(h ?? []))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, recarga]);

  useEffect(() => {
    let ativo = true;
    Promise.resolve()
      .then(() => api.estadoTokens(alvos))
      .then((r) => {
        if (!ativo) return;
        setEstados(Object.fromEntries(alvos.map((a, i) => [chaveDoAlvo(a), r[i]!])));
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
    // `alvos` muda de identidade a cada leitura; `chaveAlvos` diz quando mudou de fato.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api, chaveAlvos, recarga]);

  return (
    <section className="secao-config acessos" aria-label="Acessos">
      <div>
        <h3>Acessos ao GitHub e GitLab</h3>
        <p className="muted">
          Tokens usados para clonar repositórios e ler demandas. Ficam no chaveiro do sistema; o app
          nunca mostra o valor.
        </p>
      </div>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}

      <section className="acessos-cartao" aria-label="Token global por host">
        <div className="acessos-cabecalho">
          <h4>Token global por host</h4>
          <button
            className="btn btn-sm"
            type="button"
            aria-expanded={adicionando}
            onClick={() => setAdicionando((a) => !a)}
          >
            + Adicionar host
          </button>
        </div>
        {adicionando && (
          <FormAdicionarHost
            api={api}
            existentes={hosts.map((h) => h.host)}
            aoCancelar={() => setAdicionando(false)}
            aoAdicionar={() => {
              setAdicionando(false);
              reler();
            }}
          />
        )}
        <ul className="acessos-lista">
          {hosts.map((h) => (
            <LinhaHost
              key={h.host}
              api={api}
              host={h}
              estado={estados[h.host]}
              agora={agora}
              aoMudar={reler}
            />
          ))}
        </ul>
      </section>

      <section className="acessos-cartao" aria-label="Repositórios com token próprio">
        <div className="acessos-cabecalho">
          <h4>Repositórios com token próprio</h4>
        </div>
        {proprios.length === 0 ? (
          <p className="muted acessos-vazio">
            Nenhum repositório usa token próprio. Para usar um token só de um repositório, escolha
            “Token só deste repositório” no cadastro dele, na página do projeto.
          </p>
        ) : (
          <ul className="acessos-lista">
            {proprios.map((p) => (
              <LinhaRepositorio
                key={`${p.projetoId} ${chaveDoAlvo(p.repositorio)}`}
                api={api}
                item={p}
                estado={estadoDoRepositorio(p, estados[chaveDoAlvo(p.repositorio)])}
                agora={agora}
                aoAbrirProjeto={aoAbrirProjeto}
              />
            ))}
          </ul>
        )}
      </section>

      <details className="acessos-cartao acessos-comando">
        <summary>Prefiro um comando que imprime o token (pass, 1Password, gopass…)</summary>
        <p className="muted">
          Em vez de guardar o token, o app roda o comando quando precisa e usa a primeira linha que
          ele imprime. No host: “Trocar token” › “Comando que imprime o token”. No repositório: no
          cadastro dele, na página do projeto, escolha “Comando que imprime o token”. Exemplo:{" "}
          <code className="mono">pass show github/pat | head -1</code>.
        </p>
      </details>
    </section>
  );
}

/** Com `token = comando`, o comando do cadastro vale como token; o teste é o guardado. */
function estadoDoRepositorio(
  p: RepositorioTokenProprio,
  estado: EstadoToken | undefined,
): EstadoToken | undefined {
  if (p.repositorio.token !== "comando") return estado;
  return {
    guardado: false,
    comando: p.repositorio.comando_credencial.trim() || null,
    teste: estado?.teste ?? null,
  };
}

function ChipSituacao({ situacao }: { situacao: SituacaoToken }) {
  return (
    <span className="acessos-chip" data-tom={situacao.tom}>
      {situacao.texto}
    </span>
  );
}

function LinhaHost({
  api,
  host,
  estado,
  agora,
  aoMudar,
}: {
  api: ApiSecaoAcessos;
  host: HostAcesso;
  estado: EstadoToken | undefined;
  agora: () => number;
  aoMudar: () => void;
}) {
  const [trocando, setTrocando] = useState(false);
  const [confirmando, setConfirmando] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [aviso, setAviso] = useState<{ ok: boolean; texto: string } | null>(null);
  const situacao = situacaoDoToken(estado, agora());
  const temToken = estado?.guardado === true || !!estado?.comando;
  const usos = descreverUsos(host.usos);

  /** Roda a ação e relê os estados; `true` se deu certo. */
  const agir = (acao: () => Promise<unknown>, feito: string | null): Promise<boolean> => {
    setOcupado(true);
    setAviso(null);
    return acao()
      .then(() => {
        if (feito) setAviso({ ok: true, texto: feito });
        aoMudar();
        return true;
      })
      .catch((e: unknown) => {
        setAviso({ ok: false, texto: mensagemDe(e) });
        return false;
      })
      .finally(() => setOcupado(false));
  };

  const vale = usos ? ` Vale agora para ${usos}.` : "";

  return (
    <li className="acessos-linha" aria-label={host.host}>
      <div className="acessos-principal">
        <span className="acessos-forja">{FORJA[host.forja]}</span>
        <div className="acessos-texto">
          <span className="mono">{host.host}</span>
          <span className="muted">
            {usos ? `Usado por ${usos}` : "Nenhum projeto usa este host ainda"}
          </span>
          {estado?.comando && (
            <span className="muted">
              Token pelo comando <code className="mono">{estado.comando}</code>
            </span>
          )}
          {situacao.detalhe && <span className="muted">{situacao.detalhe}</span>}
        </div>
        <ChipSituacao situacao={situacao} />
        <div className="acessos-acoes">
          <button
            className="btn btn-sm"
            type="button"
            aria-label={`Testar o token de ${host.host}`}
            disabled={ocupado || !temToken}
            onClick={() =>
              agir(() => api.testarTokenHost(host.host, host.forja, host.exemplo), null)
            }
          >
            {ocupado ? "Testando…" : "Testar"}
          </button>
          <button
            className="btn btn-sm"
            type="button"
            aria-label={`${temToken ? "Trocar" : "Informar"} o token de ${host.host}`}
            aria-expanded={trocando}
            disabled={ocupado}
            onClick={() => setTrocando((t) => !t)}
          >
            {temToken ? "Trocar token" : "Informar token"}
          </button>
          <button
            className="btn btn-sm"
            type="button"
            aria-label={`Remover o token de ${host.host}`}
            disabled={ocupado || (!temToken && !host.extra)}
            onClick={() => setConfirmando(true)}
          >
            Remover
          </button>
        </div>
      </div>
      {trocando && (
        <FormToken
          rotulo={`Novo token de ${host.host}`}
          comandoAtual={estado?.comando ?? null}
          acao="Guardar"
          aoCancelar={() => setTrocando(false)}
          aoGuardar={(v) =>
            agir(
              () =>
                "token" in v
                  ? api.salvarToken({ host: host.host, caminho: null }, v.token)
                  : api.definirComandoHost(host.host, v.comando),
              `${"token" in v ? "Token guardado." : "Comando guardado."}${vale}`,
            ).then((ok) => {
              if (ok) setTrocando(false);
            })
          }
        />
      )}
      <p role="status" aria-live="polite" className="acessos-aviso">
        {aviso && <span className={aviso.ok ? "ok-text" : "error-text"}>{aviso.texto}</span>}
      </p>
      {confirmando && (
        <DialogoConfirmacao
          titulo={`Remover o token de ${host.host}?`}
          aoCancelar={() => setConfirmando(false)}
          acoes={
            <>
              <button className="btn" type="button" autoFocus onClick={() => setConfirmando(false)}>
                Cancelar
              </button>
              <button
                className="btn btn-danger"
                type="button"
                onClick={() => {
                  setConfirmando(false);
                  void agir(() => api.removerHostAcesso(host.host), "Token removido.");
                }}
              >
                Remover
              </button>
            </>
          }
        >
          {host.usos.length > 0 ? (
            <>
              <p>Perdem o acesso que vem do token global de {host.host}:</p>
              <ul className="acessos-afetados">
                {agruparUsos(host).map((u) => (
                  <li key={u}>{u}</li>
                ))}
              </ul>
              <p className="muted">Os repositórios com token próprio não mudam.</p>
            </>
          ) : (
            <p>Nenhum projeto usa este token. Ele sai do chaveiro do sistema.</p>
          )}
        </DialogoConfirmacao>
      )}
    </li>
  );
}

/** Um item por projeto: “Portal (portal, issues)”. */
function agruparUsos(host: HostAcesso): string[] {
  return [...new Set(host.usos.map((u) => u.projetoId))].map((id) =>
    descreverUsos(host.usos.filter((u) => u.projetoId === id)),
  );
}

function LinhaRepositorio({
  api,
  item,
  estado,
  agora,
  aoAbrirProjeto,
}: {
  api: ApiSecaoAcessos;
  item: RepositorioTokenProprio;
  estado: EstadoToken | undefined;
  agora: () => number;
  aoAbrirProjeto?: (projetoId: string) => void;
}) {
  const r = item.repositorio;
  const [ocupado, setOcupado] = useState(false);
  const [teste, setTeste] = useState<EstadoToken["teste"]>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const efetivo = estado ? { ...estado, teste: teste ?? estado.teste } : undefined;
  const situacao =
    efetivo && !efetivo.guardado && !efetivo.comando
      ? {
          tom: "erro" as const,
          texto: "✕ sem token no chaveiro",
          detalhe: null,
        }
      : situacaoDoToken(efetivo, agora());
  const nome = `${r.host}/${r.caminho}`;

  return (
    <li className="acessos-linha" aria-label={nome}>
      <div className="acessos-principal">
        <span className="acessos-forja">{FORJA[r.forja]}</span>
        <div className="acessos-texto">
          <span className="mono">{nome}</span>
          <span className="muted">
            Projeto {item.projetoNome} ·{" "}
            {r.token === "comando" ? "comando que imprime o token" : "token só deste repositório"} ·
            tem precedência sobre o token do host
          </span>
          {situacao.detalhe && <span className="muted">{situacao.detalhe}</span>}
        </div>
        <ChipSituacao situacao={situacao} />
        <div className="acessos-acoes">
          <button
            className="btn btn-sm"
            type="button"
            aria-label={`Testar o token de ${nome}`}
            disabled={ocupado}
            onClick={() => {
              setOcupado(true);
              setFalha(null);
              api
                .testarTokenRepositorio(r)
                .then(setTeste)
                .catch((e: unknown) => setFalha(mensagemDe(e)))
                .finally(() => setOcupado(false));
            }}
          >
            {ocupado ? "Testando…" : "Testar"}
          </button>
          {aoAbrirProjeto && (
            <button
              className="btn btn-sm"
              type="button"
              aria-label={`Abrir o projeto ${item.projetoNome}`}
              onClick={() => aoAbrirProjeto(item.projetoId)}
            >
              Abrir no projeto
            </button>
          )}
        </div>
      </div>
      {falha && (
        <p className="error-text acessos-aviso" role="alert">
          {falha}
        </p>
      )}
    </li>
  );
}

type ValorToken = { token: string } | { comando: string };

/**
 * Token ou comando que imprime o token. O token digitado fica só no campo (senha) e
 * é limpo ao guardar.
 */
function FormToken({
  rotulo,
  comandoAtual,
  acao,
  antes,
  podeGuardar = true,
  aoGuardar,
  aoCancelar,
}: {
  rotulo: string;
  comandoAtual: string | null;
  acao: string;
  antes?: ReactNode;
  podeGuardar?: boolean;
  aoGuardar: (v: ValorToken) => Promise<void>;
  aoCancelar: () => void;
}) {
  const [usarComando, setUsarComando] = useState(comandoAtual !== null);
  const [token, setToken] = useState("");
  const [comando, setComando] = useState(comandoAtual ?? "");
  const [ocupado, setOcupado] = useState(false);
  const valor = usarComando ? comando.trim() : token.trim();

  return (
    <form
      className="acessos-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (!valor || !podeGuardar) return;
        setOcupado(true);
        aoGuardar(usarComando ? { comando: valor } : { token: valor }).finally(() => {
          setToken("");
          setOcupado(false);
        });
      }}
    >
      {antes}
      <fieldset className="acessos-origem">
        <legend className="sr-only">De onde vem o token</legend>
        <label>
          <input type="radio" checked={!usarComando} onChange={() => setUsarComando(false)} />{" "}
          Guardar o token no chaveiro
        </label>
        <label>
          <input type="radio" checked={usarComando} onChange={() => setUsarComando(true)} /> Comando
          que imprime o token
        </label>
      </fieldset>
      {usarComando ? (
        <label className="field">
          Comando que imprime o token
          <input
            type="text"
            className="mono"
            value={comando}
            placeholder="pass show github/pat | head -1"
            spellCheck={false}
            onChange={(e) => setComando(e.target.value)}
          />
        </label>
      ) : (
        <label className="field">
          {rotulo}
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={token}
            onChange={(e) => setToken(e.target.value)}
          />
        </label>
      )}
      <div className="acessos-acoes">
        <button
          className="btn btn-sm btn-primary"
          type="submit"
          disabled={ocupado || !valor || !podeGuardar}
        >
          {acao}
        </button>
        <button className="btn btn-sm" type="button" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

function FormAdicionarHost({
  api,
  existentes,
  aoAdicionar,
  aoCancelar,
}: {
  api: ApiSecaoAcessos;
  existentes: string[];
  aoAdicionar: () => void;
  aoCancelar: () => void;
}) {
  const [forja, setForja] = useState<ForgeKind>("gitlab");
  const [host, setHost] = useState("");
  const [falha, setFalha] = useState<string | null>(null);
  const alvo = forja === "github" ? "github.com" : host.trim().toLowerCase();
  const invalido = !alvo || /[/:\s]/.test(alvo);

  return (
    <div className="acessos-novo" role="group" aria-label="Adicionar host">
      <FormToken
        rotulo="Token (PAT)"
        comandoAtual={null}
        acao="Adicionar"
        podeGuardar={!invalido}
        antes={
          <div className="acessos-campos">
            <label className="field">
              Forja
              <select value={forja} onChange={(e) => setForja(e.target.value as ForgeKind)}>
                <option value="gitlab">GitLab</option>
                <option value="github">GitHub</option>
              </select>
            </label>
            <label className="field">
              Host
              <input
                type="text"
                className="mono"
                value={forja === "github" ? "github.com" : host}
                disabled={forja === "github"}
                placeholder="gitlab.example.com"
                spellCheck={false}
                onChange={(e) => setHost(e.target.value)}
              />
            </label>
            {existentes.includes(alvo) && (
              <p className="muted">
                {alvo} já está na lista: o token informado aqui substitui o atual.
              </p>
            )}
          </div>
        }
        aoCancelar={aoCancelar}
        aoGuardar={(v) => {
          setFalha(null);
          return api
            .adicionarHostAcesso(alvo, forja)
            .then(() =>
              "token" in v
                ? api.salvarToken({ host: alvo, caminho: null }, v.token)
                : api.definirComandoHost(alvo, v.comando),
            )
            .then(aoAdicionar)
            .catch((e: unknown) => setFalha(mensagemDe(e)));
        }}
      />
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </div>
  );
}
