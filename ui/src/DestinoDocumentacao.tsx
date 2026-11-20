import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { mensagemDe } from "./analise";
import type { PropsConfigProjeto } from "./ConfigProjeto";
import { CampoTexto, ItemRepositorioCodigo } from "./FormRepositorios";
import {
  REPOSITORIO_CODIGO_VAZIO,
  chaveDoRepositorio,
  normalizarSubpasta,
  validarRepositorios,
  type ApiRepositorios,
  type DestinoDocs,
  type Repositorios,
  type SituacaoDocsExternos,
} from "./repositorios";
import {
  FORJA,
  descreverDestino,
  falhou,
  integracaoDoDestino,
  primeiraFalha,
} from "./saudeProjeto";

type Modo = "nenhum" | DestinoDocs["modo"];

const MODOS: { id: Modo; rotulo: string }[] = [
  { id: "nenhum", rotulo: "Nenhum" },
  { id: "subpasta", rotulo: "Pasta num repositório de código" },
  { id: "repositorio", rotulo: "Repositório próprio" },
];

/** O destino como gravado, com a subpasta normalizada (para comparar com o salvo). */
function normalizado(d: DestinoDocs | null): DestinoDocs | null {
  if (!d) return null;
  const subpasta = normalizarSubpasta(d.subpasta ?? "") ?? d.subpasta ?? "";
  if (d.modo === "subpasta") {
    return { ...d, codigo: d.codigo.trim().toLowerCase(), subpasta };
  }
  return { ...d, subpasta };
}

/**
 * Destino da documentação publicada (#274), na aba “Código e demandas”: uma subpasta
 * (`docs/negocio`) de um repositório de código já cadastrado ou um repositório só da
 * documentação, com URL, branch base, token e a pasta do clone. O acesso é conferido
 * junto com os repositórios ao abrir a página; “Testar acesso” confere o repositório
 * próprio antes de salvar.
 */
export function DestinoDocumentacao({
  api,
  projeto,
  estado,
  aoMudar,
  aoConferirProntidao,
  aoIrParaAcessos,
}: PropsConfigProjeto) {
  const codigo = projeto.repositorios.codigo;
  const gravado = projeto.repositorios.docs ?? null;
  const [valor, setValor] = useState<DestinoDocs | null>(gravado);
  const [salvando, setSalvando] = useState(false);
  const [clonando, setClonando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const pilulas = useRef<Partial<Record<Modo, HTMLButtonElement | null>>>({});
  const modo: Modo = valor?.modo ?? "nenhum";
  const mudou =
    JSON.stringify(normalizado(valor)) !== JSON.stringify(normalizado(gravado));
  const integ = mudou ? undefined : integracaoDoDestino(projeto, estado.prontidao);
  const comFalha = falhou(integ);
  const orientacao = integ?.passos.find((p) => p.situacao === "falhou")?.orientacao;

  const escolher = (m: Modo) => {
    if (m === modo) return;
    setAviso(null);
    setFalha(null);
    if (m === gravado?.modo) setValor(gravado);
    else if (m === "subpasta") {
      setValor({
        modo: "subpasta",
        codigo: codigo[0] ? chaveDoRepositorio(codigo[0]) : "",
        subpasta: "",
      });
    } else if (m === "repositorio") {
      setValor({
        modo: "repositorio",
        repositorio: { ...REPOSITORIO_CODIGO_VAZIO },
        subpasta: "",
      });
    } else setValor(null);
  };

  const teclas = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (!["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"].includes(e.key)) return;
    e.preventDefault();
    const i = MODOS.findIndex((m) => m.id === modo);
    const passo = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : -1;
    const proximo = MODOS[(i + passo + MODOS.length) % MODOS.length].id;
    escolher(proximo);
    pilulas.current[proximo]?.focus();
  };

  const salvar = () => {
    const novos: Repositorios = { ...projeto.repositorios, docs: normalizado(valor) };
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
        setAviso(valor ? "Destino da documentação salvo." : "Destino da documentação removido.");
        aoMudar();
        aoConferirProntidao(projeto.id, true);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setSalvando(false));
  };

  const clonar = () => {
    if (valor?.modo !== "repositorio") return;
    setFalha(null);
    setClonando(true);
    api
      .pastaClones()
      .then((base) => {
        if (!base) {
          throw new Error(
            "Escolha antes a pasta dos clones (no cadastro do projeto) ou informe a pasta do clone.",
          );
        }
        return api.clonarRepositorios([valor.repositorio], base, () => {});
      })
      .then(([r]) => {
        if (!r || r.cancelado) return;
        if (r.erro) throw new Error(r.erro);
        setValor({ ...valor, repositorio: { ...valor.repositorio, pasta: r.repositorio.pasta } });
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setClonando(false));
  };

  return (
    <section className="cartao-config" aria-label="Destino da documentação">
      <div className="cartao-config-topo">
        <h4>Onde a documentação é publicada</h4>
        {gravado && !mudou && (
          <span
            className="badge"
            data-tone={
              estado.prontidao === undefined
                ? "running"
                : comFalha
                  ? "error"
                  : integ
                    ? "done"
                    : undefined
            }
          >
            {estado.prontidao === undefined
              ? "conferindo…"
              : comFalha
                ? "✕ sem acesso"
                : integ
                  ? "✓ acesso ok"
                  : "não conferido"}
          </span>
        )}
      </div>
      <div className="linha-config demandas-corpo">
        <div className="pilulas" role="radiogroup" aria-label="Destino da documentação">
          {MODOS.map((m) => (
            <button
              key={m.id}
              ref={(el) => {
                pilulas.current[m.id] = el;
              }}
              type="button"
              role="radio"
              className="pilula"
              aria-checked={modo === m.id}
              tabIndex={modo === m.id ? 0 : -1}
              onClick={() => escolher(m.id)}
              onKeyDown={teclas}
            >
              {m.rotulo}
            </button>
          ))}
        </div>
        {!valor && (
          <p className="muted">
            Opcional: com o destino, a documentação aprovada pode ser publicada num
            repositório, por uma branch própria e um PR/MR.
          </p>
        )}
        {gravado && !mudou && (
          <p className="muted">
            Publicada em{" "}
            <span className="caminho">{descreverDestino(gravado, codigo)}</span>
            {gravado.modo === "repositorio" && (
              <>
                {" "}
                · branch base <strong>{gravado.repositorio.branch || "padrão"}</strong>
                {gravado.repositorio.pasta && (
                  <>
                    {" "}
                    · clone em <span className="caminho">{gravado.repositorio.pasta}</span>
                  </>
                )}
              </>
            )}
            .
          </p>
        )}
        {gravado && !mudou && <SincronizacaoDocs api={api} projeto={projeto.id} />}
        {comFalha && (
          <p className="error-text">
            {primeiraFalha(integ)}
            {orientacao && <span className="muted"> {orientacao}</span>}
          </p>
        )}
        {valor?.modo === "subpasta" &&
          (codigo.length === 0 ? (
            <p className="muted" role="alert">
              Cadastre antes um repositório de código, ou escolha um repositório próprio.
            </p>
          ) : (
            <>
              <div className="field">
                <label>
                  Repositório de código
                  <select
                    value={valor.codigo}
                    onChange={(e) => setValor({ ...valor, codigo: e.target.value })}
                  >
                    {codigo.map((c) => (
                      <option key={chaveDoRepositorio(c)} value={chaveDoRepositorio(c)}>
                        {FORJA[c.forja]} {c.caminho}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              <CampoTexto
                rotulo="Subpasta da documentação"
                valor={valor.subpasta}
                exemplo="docs/negocio"
                mono
                aoMudar={(subpasta) => setValor({ ...valor, subpasta })}
              />
              <p className="muted">
                A subpasta não é código-fonte: fica fora do que o agente analisa, das
                evidências e do aviso de alteração. O PR/MR usa a branch do repositório
                de código.
              </p>
            </>
          ))}
        {valor?.modo === "repositorio" && (
          <>
            <ItemRepositorioCodigo
              api={api}
              numero={1}
              nome="Repositório da documentação"
              valor={valor.repositorio}
              fixo={false}
              aoMudar={(repositorio) => setValor({ ...valor, repositorio })}
            />
            <div className="field-pair">
              <CampoTexto
                rotulo="Pasta do clone (opcional, caminho completo)"
                valor={valor.repositorio.pasta ?? ""}
                exemplo="vazio = clonar depois"
                mono
                aoMudar={(p) =>
                  setValor({
                    ...valor,
                    repositorio: { ...valor.repositorio, pasta: p.trim() ? p : null },
                  })
                }
              />
              <CampoTexto
                rotulo="Subpasta (opcional)"
                valor={valor.subpasta ?? ""}
                exemplo="vazio = a raiz do repositório"
                mono
                aoMudar={(subpasta) => setValor({ ...valor, subpasta })}
              />
            </div>
            {!valor.repositorio.pasta && (
              <div className="acoes">
                <button
                  className="btn btn-sm"
                  type="button"
                  disabled={clonando || !valor.repositorio.url_clone.trim()}
                  onClick={clonar}
                >
                  Clonar na pasta dos clones
                </button>
              </div>
            )}
            {clonando && <p className="muted">Clonando o repositório da documentação…</p>}
            <p className="muted">
              A branch é a base do PR/MR. O clone não é pasta de código do projeto: o
              agente não o analisa. O token global de cada host fica em{" "}
              <button type="button" className="link" onClick={aoIrParaAcessos}>
                Acessos
              </button>
              .
            </p>
          </>
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
              }}
            >
              Cancelar
            </button>
            <button
              className={valor ? "btn btn-primary" : "btn btn-danger"}
              type="button"
              disabled={salvando}
              onClick={salvar}
            >
              {valor ? "Salvar o destino" : "Remover o destino"}
            </button>
          </div>
        )}
      </div>
    </section>
  );
}

/** Data e hora curtas, no fuso do analista. */
function quando(ms: number): string {
  return new Date(ms).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

/**
 * Sincronização do repositório de docs (#277): traz os `.md` publicados de volta
 * para a base (pasta `externos/`), como fonte externa rotulada e só de leitura. O
 * app também sincroniza em segundo plano ao iniciar uma análise.
 */
export function SincronizacaoDocs({
  api,
  projeto,
}: {
  api: Pick<ApiRepositorios, "situacaoDocsExternos" | "sincronizarDocsExternos">;
  projeto: string;
}) {
  const [situacao, setSituacao] = useState<SituacaoDocsExternos | null>(null);
  const [sincronizando, setSincronizando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    let vivo = true;
    api
      .situacaoDocsExternos(projeto)
      .then((s) => vivo && setSituacao(s))
      .catch(() => {});
    return () => {
      vivo = false;
    };
  }, [api, projeto]);

  const sincronizar = () => {
    setSincronizando(true);
    setFalha(null);
    api
      .sincronizarDocsExternos(projeto)
      .then(setSituacao)
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        api
          .situacaoDocsExternos(projeto)
          .then(setSituacao)
          .catch(() => {});
      })
      .finally(() => setSincronizando(false));
  };

  const s = situacao;
  const docs = (n: number) => (n === 1 ? "documento externo" : "documentos externos");
  return (
    <div className="sincronizacao-docs" role="group" aria-label="Sincronização da documentação">
      <p className="muted">
        {s?.sincronizadoMs ? (
          <>
            Na base: {s.importados} {docs(s.importados)} de <strong>{s.repositorio}</strong>,
            commit <code>{s.commit?.slice(0, 7)}</code>, sincronizado em{" "}
            {quando(s.sincronizadoMs)}
            {s.jaNaBase > 0 && ` (${s.jaNaBase} já na base, não duplicados)`}.
          </>
        ) : (
          "A documentação publicada ainda não foi trazida para a base."
        )}{" "}
        Os documentos externos ficam em <span className="caminho">externos/</span>, só para
        leitura.
      </p>
      {s?.aviso && !falha && (
        <p className="attention-text" role="status">
          {s.aviso}
        </p>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {s && s.pulados.length > 0 && (
        <details>
          <summary>
            {s.pulados.length} {s.pulados.length === 1 ? "arquivo pulado" : "arquivos pulados"}
          </summary>
          <ul>
            {s.pulados.map((p) => (
              <li key={p.caminho}>
                <span className="caminho">{p.caminho}</span>: {p.motivo}
              </li>
            ))}
          </ul>
        </details>
      )}
      {s && s.mascarados.length > 0 && (
        <p className="muted">
          Trechos com possível segredo ou dado pessoal mascarados em {s.mascarados.length}{" "}
          {s.mascarados.length === 1 ? "documento" : "documentos"}.
        </p>
      )}
      <div className="acoes">
        <button className="btn btn-sm" type="button" disabled={sincronizando} onClick={sincronizar}>
          {sincronizando ? "Sincronizando…" : "Sincronizar agora"}
        </button>
      </div>
    </div>
  );
}
