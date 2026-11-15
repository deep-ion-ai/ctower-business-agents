import { useCallback, useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import type { Configuracoes } from "./configuracoes";
import type { ApiRepositorios } from "./repositorios";

type Api = Pick<
  ApiRepositorios,
  "pastaClones" | "definirPastaClones" | "salvarToken" | "apagarToken" | "tokensGuardados"
>;

/** github.com e os hosts dos repositórios de código e do GitLab Issues dos projetos. */
export function hostsConhecidos(cfg: Pick<Configuracoes, "projetos">): string[] {
  const hosts = ["github.com"];
  for (const p of cfg.projetos) {
    const r = p.repositorios;
    if (!r) continue;
    for (const c of r.codigo) hosts.push(c.host);
    if (r.tracker?.kind === "gitlab") hosts.push(r.tracker.host);
  }
  return normalizarHosts(hosts);
}

function normalizarHosts(hosts: string[]): string[] {
  const vistos: string[] = [];
  for (const h of hosts) {
    const n = h.trim().toLowerCase();
    if (n && !vistos.includes(n)) vistos.push(n);
  }
  return vistos;
}

/**
 * Clones e tokens (#179): a pasta onde o app clona os repositórios de código e os
 * tokens (PAT) globais de cada host, guardados no chaveiro do sistema. O token próprio
 * de um repositório fica no cadastro do repositório e tem precedência sobre o global.
 */
export function SecaoClones({ api, cfg }: { api: Api; cfg: Pick<Configuracoes, "projetos"> }) {
  return (
    <section className="secao-config" aria-label="Clones e tokens">
      <h3>Clones e tokens</h3>
      <PastaClones api={api} />
      <TokensGlobais api={api} conhecidos={hostsConhecidos(cfg)} />
    </section>
  );
}

function PastaClones({ api }: { api: Api }) {
  const [pasta, setPasta] = useState("");
  const [gravada, setGravada] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [salvo, setSalvo] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    Promise.resolve()
      .then(() => api.pastaClones())
      .then((p) => {
        if (!ativo) return;
        setGravada(p ?? null);
        setPasta(p ?? "");
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api]);

  const salvar = () => {
    setOcupado(true);
    setFalha(null);
    setSalvo(false);
    api
      .definirPastaClones(pasta.trim())
      .then((p) => {
        setGravada(p);
        setPasta(p);
        setSalvo(true);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  return (
    <div role="group" aria-label="Pasta dos clones">
      <div className="field">
        <label>
          Pasta dos clones
          <input
            type="text"
            className="caminho"
            value={pasta}
            placeholder="/Users/voce/Documents/ctower-business-repos"
            spellCheck={false}
            onChange={(e) => {
              setPasta(e.target.value);
              setSalvo(false);
            }}
          />
        </label>
        <p className="muted">
          Onde o app clona os repositórios de código, um por subpasta. Caminho completo.
        </p>
      </div>
      <div className="acoes">
        <button
          className="btn btn-sm"
          type="button"
          disabled={ocupado || !pasta.trim() || pasta.trim() === gravada}
          onClick={salvar}
        >
          Salvar a pasta
        </button>
        <span role="status" aria-live="polite">
          {salvo && <span className="ok-text"> Salvo.</span>}
        </span>
      </div>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </div>
  );
}

function TokensGlobais({ api, conhecidos }: { api: Api; conhecidos: string[] }) {
  const [extras, setExtras] = useState<string[]>([]);
  const [novo, setNovo] = useState("");
  const [guardados, setGuardados] = useState<Record<string, boolean>>({});
  const hosts = normalizarHosts([...conhecidos, ...extras]);
  const chave = hosts.join(" ");

  const reler = useCallback(() => {
    const lista = chave.split(" ").filter(Boolean);
    let ativo = true;
    Promise.resolve()
      .then(() => api.tokensGuardados(lista.map((host) => ({ host, caminho: null }))))
      .then((r) => {
        if (!ativo) return;
        setGuardados(Object.fromEntries(lista.map((h, i) => [h, r?.[i] === true])));
      })
      .catch(() => ativo && setGuardados({}));
    return () => {
      ativo = false;
    };
  }, [api, chave]);

  useEffect(reler, [reler]);

  const acrescentar = () => {
    const h = novo.trim().toLowerCase();
    if (!h) return;
    setExtras((l) => [...l, h]);
    setNovo("");
  };

  return (
    <div role="group" aria-label="Tokens globais por host">
      <h4>Tokens globais por host</h4>
      <p className="muted">
        Um token (PAT) por host, usado pelos repositórios com “Token global do host”. Fica no
        chaveiro do sistema, nunca no projeto, e o app não o mostra de volta.
      </p>
      <ul className="tokens-hosts">
        {hosts.map((h) => (
          <TokenDoHost
            key={h}
            api={api}
            host={h}
            guardado={guardados[h] ?? null}
            aoMudar={reler}
          />
        ))}
      </ul>
      <div className="field">
        <label>
          Outro host
          <input
            type="text"
            value={novo}
            placeholder="gitlab.example.com"
            spellCheck={false}
            onChange={(e) => setNovo(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                acrescentar();
              }
            }}
          />
        </label>
      </div>
      <button className="btn btn-sm" type="button" disabled={!novo.trim()} onClick={acrescentar}>
        Acrescentar host
      </button>
    </div>
  );
}

function TokenDoHost({
  api,
  host,
  guardado,
  aoMudar,
}: {
  api: Api;
  host: string;
  guardado: boolean | null;
  aoMudar: () => void;
}) {
  const [token, setToken] = useState("");
  const [ocupado, setOcupado] = useState(false);
  const [aviso, setAviso] = useState<{ ok: boolean; texto: string } | null>(null);

  const agir = (acao: () => Promise<void>, feito: string) => {
    setOcupado(true);
    setAviso(null);
    acao()
      .then(() => {
        setToken("");
        setAviso({ ok: true, texto: feito });
        aoMudar();
      })
      .catch((e: unknown) => setAviso({ ok: false, texto: mensagemDe(e) }))
      .finally(() => setOcupado(false));
  };

  return (
    <li>
      <div className="field">
        <label>
          Token de {host}
          <input
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={token}
            placeholder={guardado ? "guardado; digite para trocar" : undefined}
            onChange={(e) => setToken(e.target.value)}
          />
        </label>
      </div>
      <div className="acoes">
        <button
          className="btn btn-sm"
          type="button"
          aria-label={`Guardar o token de ${host}`}
          disabled={ocupado || !token.trim()}
          onClick={() =>
            agir(() => api.salvarToken({ host, caminho: null }, token.trim()), "Token guardado.")
          }
        >
          Guardar
        </button>{" "}
        <button
          className="btn btn-sm"
          type="button"
          aria-label={`Apagar o token de ${host}`}
          disabled={ocupado || guardado !== true}
          onClick={() => agir(() => api.apagarToken({ host, caminho: null }), "Token apagado.")}
        >
          Apagar
        </button>{" "}
        {guardado === true && <span className="ok-text">guardado</span>}
        {guardado === false && <span className="muted">sem token</span>}
      </div>
      <p role="status" aria-live="polite">
        {aviso && <span className={aviso.ok ? "ok-text" : "error-text"}>{aviso.texto}</span>}
      </p>
    </li>
  );
}
