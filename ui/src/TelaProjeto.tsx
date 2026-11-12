import { useCallback, useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import {
  ROTULO_STATUS_REGRA,
  type ApiConhecimento,
  type MapaVista,
  type ResumoRevalidacao,
  type VisaoProjeto,
} from "./conhecimento";
import type { Tela } from "./Navegacao";
import type { ProjetoResumo } from "./terminal";

type Revalidacao =
  | { tipo: "conferindo" }
  | { tipo: "feita"; resumo: ResumoRevalidacao }
  | { tipo: "falha"; mensagem: string };

/**
 * Projeto (spec, seção 10): áreas, regras (as "a revalidar" primeiro e destacadas),
 * glossário e mapa. Ao abrir, confere as evidências contra o código atual (spec,
 * seção 6.5) e relê a lista se algo mudou; o analista pode conferir de novo.
 */
export function TelaProjeto({
  api,
  projetos,
  projetoInicial,
  navegar,
}: {
  api: ApiConhecimento;
  projetos: ProjetoResumo[];
  projetoInicial?: string;
  navegar: (t: Tela) => void;
}) {
  const [escolhido, setEscolhido] = useState(projetoInicial ?? "");
  const projeto = escolhido || projetos[0]?.id || "";

  return (
    <section className="tela-projeto page" aria-label="Projeto">
      {projetos.length > 1 && (
        <label>
          Projeto{" "}
          <select value={projeto} onChange={(e) => setEscolhido(e.target.value)}>
            {projetos.map((s) => (
              <option key={s.id} value={s.id}>
                {s.nome}
              </option>
            ))}
          </select>
        </label>
      )}
      {projetos.length === 0 && !projetoInicial && (
        <p className="muted">Nenhum projeto cadastrado.</p>
      )}
      {projeto && <Projeto key={projeto} api={api} projeto={projeto} navegar={navegar} />}
    </section>
  );
}

function Projeto({
  api,
  projeto,
  navegar,
}: {
  api: ApiConhecimento;
  projeto: string;
  navegar: (t: Tela) => void;
}) {
  const [visao, setVisao] = useState<VisaoProjeto | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [revalidacao, setRevalidacao] = useState<Revalidacao>({ tipo: "conferindo" });
  const [area, setArea] = useState<string | null>(null);

  const carregar = useCallback(() => {
    api
      .visaoProjeto(projeto)
      .then((v) => {
        setVisao(v);
        setFalha(null);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto]);

  const revalidar = useCallback(() => {
    setRevalidacao({ tipo: "conferindo" });
    api
      .revalidar(projeto)
      .then((resumo) => {
        setRevalidacao({ tipo: "feita", resumo });
        if (resumo.mudou) carregar();
      })
      .catch((e: unknown) => setRevalidacao({ tipo: "falha", mensagem: mensagemDe(e) }));
  }, [api, projeto, carregar]);

  // Ao abrir: a lista na hora e a revalidação em seguida.
  useEffect(() => {
    carregar();
    revalidar();
  }, [carregar, revalidar]);

  // Edições fora do app (regra editada, `git pull` na base): relê.
  useEffect(() => {
    let parar: (() => void) | null = null;
    let ativo = true;
    api
      .vigiarBase(carregar)
      .then((p) => (ativo ? (parar = p) : p()))
      .catch(() => undefined);
    return () => {
      ativo = false;
      parar?.();
    };
  }, [api, carregar]);

  const abrir = (caminho: string) => navegar({ tipo: "base", documento: caminho });

  if (falha) {
    return (
      <p className="error-text" role="alert">
        Não foi possível ler o projeto: {falha}
      </p>
    );
  }
  if (!visao) return <p className="muted">Carregando…</p>;

  const aRevalidar = visao.regras.filter((r) => r.status === "a-revalidar").length;
  const regras = area ? visao.regras.filter((r) => r.area === area) : visao.regras;
  const termos = area ? visao.termos.filter((t) => t.area === area) : visao.termos;

  return (
    <>
      <h2>
        {visao.nome}{" "}
        <button type="button" className="link" onClick={() => abrir(visao.documento)}>
          abrir o projeto.md
        </button>
      </h2>

      <div className="revalidacao" role="status" aria-label="Revalidação">
        {revalidacao.tipo === "conferindo" && (
          <span className="muted">Conferindo as evidências com o código atual…</span>
        )}
        {revalidacao.tipo === "falha" && (
          <span className="error-text">
            Não foi possível conferir as evidências: {revalidacao.mensagem}
          </span>
        )}
        {revalidacao.tipo === "feita" && <TextoRevalidacao resumo={revalidacao.resumo} />}{" "}
        <button
          className="btn"
          type="button"
          onClick={revalidar}
          disabled={revalidacao.tipo === "conferindo"}
        >
          Conferir de novo
        </button>
      </div>

      {aRevalidar > 0 && (
        <p className="aviso-revalidar" role="alert">
          {aRevalidar === 1
            ? "1 regra está a revalidar: o código citado mudou desde a aprovação."
            : `${aRevalidar} regras estão a revalidar: o código citado mudou desde a aprovação.`}
        </p>
      )}

      <section aria-label="Áreas">
        <h3>Áreas</h3>
        {visao.areas.length === 0 && <p className="muted">Nenhuma área declarada ainda.</p>}
        <ul className="lista-areas">
          {visao.areas.map((a) => (
            <li key={a.id}>
              <button
                className="btn"
                type="button"
                aria-pressed={area === a.id}
                onClick={() => setArea(area === a.id ? null : a.id)}
              >
                {a.nome}
                {a.sigla && ` (${a.sigla})`}
              </button>{" "}
              <span className="muted">
                {a.regras} regra(s) · {a.termos} termo(s)
                {!a.declarada && " · fora do projeto.md"}
              </span>
              {a.aRevalidar > 0 && (
                <span className="badge" data-tone="attention">
                  {a.aRevalidar} a revalidar
                </span>
              )}
              {a.memoria && (
                <>
                  {" "}
                  <button type="button" className="link" onClick={() => abrir(a.memoria!)}>
                    memória
                  </button>
                </>
              )}
            </li>
          ))}
        </ul>
      </section>

      <section aria-label="Regras">
        <h3>Regras{area && ` da área ${area}`}</h3>
        {regras.length === 0 && (
          <p className="muted">
            Nenhuma regra ainda: as afirmações confirmadas viram regras quando a análise é
            aprovada.
          </p>
        )}
        <ul className="lista-regras">
          {regras.map((r) => (
            <li
              key={r.id}
              className="regra"
              data-status={r.status}
            >
              <button type="button" className="link" onClick={() => abrir(r.caminho)}>
                {r.id}: {r.titulo}
              </button>{" "}
              <span
                className="badge"
                data-tone={r.status === "a-revalidar" ? "attention" : r.status === "confirmada" ? "done" : undefined}
              >
                {ROTULO_STATUS_REGRA[r.status]}
              </span>{" "}
              <span className="muted">{r.area}</span>
              {r.origem && (
                <>
                  {" · "}
                  <button type="button" className="link" onClick={() => abrir(r.origem!)}>
                    origem
                  </button>
                </>
              )}
            </li>
          ))}
        </ul>
      </section>

      <section aria-label="Glossário">
        <h3>Glossário</h3>
        {termos.length === 0 && (
          <p className="muted">
            Nenhum termo ainda: os termos aceitos viram glossário quando a análise é aprovada.
          </p>
        )}
        <ul className="lista-termos">
          {termos.map((t) => (
            <li key={t.caminho}>
              <button type="button" className="link" onClick={() => abrir(t.caminho)}>
                {t.termo}
              </button>
              {t.sinonimos.length > 0 && (
                <span className="muted"> ({t.sinonimos.join(", ")})</span>
              )}
              {t.area && <span className="muted"> · {t.area}</span>}
            </li>
          ))}
        </ul>
      </section>

      {visao.problemas.length > 0 && (
        <section aria-label="Documentos ilegíveis">
          <h3>Documentos ilegíveis</h3>
          <ul>
            {visao.problemas.map((p) => (
              <li key={p.caminho}>
                <code>{p.caminho}</code>: {p.erro}
              </li>
            ))}
          </ul>
        </section>
      )}

      <MapaProjeto api={api} projeto={projeto} />
    </>
  );
}

function TextoRevalidacao({ resumo }: { resumo: ResumoRevalidacao }) {
  const marcadas = resumo.regras.filter((r) => r.depois === "a-revalidar");
  const restauradas = resumo.regras.filter((r) => r.depois === "confirmada");
  const partes = [`${resumo.evidencias} evidência(s) conferida(s)`];
  if (resumo.desatualizadas > 0) partes.push(`${resumo.desatualizadas} não confere(m)`);
  if (marcadas.length > 0)
    partes.push(`passaram a revalidar: ${marcadas.map((r) => r.id).join(", ")}`);
  if (restauradas.length > 0)
    partes.push(`voltaram a confirmadas: ${restauradas.map((r) => r.id).join(", ")}`);
  if (resumo.problemas.length > 0) partes.push(`${resumo.problemas.length} documento(s) com problema`);
  return <span className="muted">{partes.join("; ")}.</span>;
}

/** Mapa estrutural por pasta (sem IA, sem custo), com as subpastas navegáveis. */
function MapaProjeto({ api, projeto }: { api: ApiConhecimento; projeto: string }) {
  const [pasta, setPasta] = useState("");
  const [mapa, setMapa] = useState<MapaVista | null | undefined>(undefined);
  const [mapeando, setMapeando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    api
      .mapa(projeto, pasta)
      .then((m) => {
        if (!ativo) return;
        setMapa(m);
        setFalha(null);
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto, pasta]);

  const mapear = () => {
    setMapeando(true);
    setFalha(null);
    api
      .mapear(projeto)
      .then((m) => {
        setPasta("");
        setMapa(m);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setMapeando(false));
  };

  const trilha = pasta ? pasta.split("/") : [];

  return (
    <section aria-label="Mapa do projeto" className="mapa-projeto">
      <h3>Mapa</h3>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {mapa === undefined && !falha && <p className="muted">Carregando…</p>}
      {mapa === null && (
        <p className="muted">O mapa do projeto ainda não foi gerado.</p>
      )}
      {mapa && (
        <>
          <nav aria-label="Pasta do mapa" className="trilha">
            <button type="button" className="link" onClick={() => setPasta("")}>
              (raiz)
            </button>
            {trilha.map((p, i) => (
              <span key={i}>
                {" / "}
                <button
                  type="button"
                  className="link"
                  onClick={() => setPasta(trilha.slice(0, i + 1).join("/"))}
                >
                  {p}
                </button>
              </span>
            ))}
          </nav>
          <p>
            {mapa.modulo.arquivos} arquivo(s)
            {mapa.modulo.linguagens.length > 0 &&
              `: ${mapa.modulo.linguagens.map((l) => `${l.rotulo} ${l.quantidade}`).join(", ")}`}
            {mapa.modulo.falhas > 0 && ` · ${mapa.modulo.falhas} com falha de extração`}
          </p>
          {mapa.modulo.simbolos.length > 0 && (
            <p className="muted">
              {mapa.modulo.simbolos.map((s) => `${s.rotulo} ${s.quantidade}`).join(" · ")}
            </p>
          )}
          {mapa.modulo.subpastas.length > 0 && (
            <ul className="lista-pastas">
              {mapa.modulo.subpastas.map((s) => (
                <li key={s.caminho}>
                  <button type="button" className="link" onClick={() => setPasta(s.caminho)}>
                    {s.caminho.split("/").pop()}/
                  </button>{" "}
                  <span className="muted">{s.arquivos} arquivo(s)</span>
                </li>
              ))}
            </ul>
          )}
          {mapa.modulo.arquivosDiretos.length > 0 && (
            <ul className="lista-arquivos">
              {mapa.modulo.arquivosDiretos.map((a) => (
                <li key={a}>
                  <code>{a.split("/").pop()}</code>
                </li>
              ))}
            </ul>
          )}
          {mapa.modulo.dependencias.length > 0 && (
            <>
              <h4>Dependências entre as pastas</h4>
              <ul>
                {mapa.modulo.dependencias.map((d) => (
                  <li key={`${d.de}->${d.para}`}>
                    {d.de || "(raiz)"} → {d.para || "(raiz)"}{" "}
                    <span className="muted">{d.referencias} referência(s)</span>
                  </li>
                ))}
              </ul>
            </>
          )}
          <p className="muted">
            Gerado em {new Date(mapa.geradoEm * 1000).toLocaleString("pt-BR")}.
          </p>
        </>
      )}
      <button className="btn" type="button" onClick={mapear} disabled={mapeando}>
        {mapeando ? "Mapeando…" : mapa ? "Mapear de novo" : "Mapear agora"}
      </button>
    </section>
  );
}
