import { useCallback, useEffect, useState, type ReactNode } from "react";
import { contar } from "./formatacao";
import { mensagemDe } from "./analise";
import { type ApiConhecimento, type MapaVista, type VisaoProjeto } from "./conhecimento";
import type { ApiCobertura } from "./cobertura";
import { CoberturaProjeto } from "./CoberturaProjeto";
import { ListaDemandas } from "./ListaDemandas";
import { simbolosEmNegocio } from "./mapaNegocio";
import type { Tela } from "./Navegacao";
import type { ProjetoResumo } from "./terminal";

/**
 * Projeto (spec, seção 10): as demandas (#138), áreas, regras (as "a revalidar" primeiro e destacadas),
 * glossário e mapa. Ao abrir, confere as evidências contra o código atual (spec,
 * seção 6.5) e relê a lista se algo mudou; o analista pode conferir de novo.
 */
export function TelaProjeto({
  api,
  projetos,
  projetoInicial,
  navegar,
  cabecalho,
}: {
  api: ApiConhecimento;
  projetos: ProjetoResumo[];
  projetoInicial?: string;
  navegar: (t: Tela) => void;
  /** Topo da tela, acima de tudo (ex.: o cabeçalho da Visão geral, #319). */
  cabecalho?: ReactNode;
}) {
  const [escolhido, setEscolhido] = useState(projetoInicial ?? "");
  const projeto = escolhido || projetos[0]?.id || "";

  return (
    <section className="tela-projeto page" aria-label="Projeto">
      {cabecalho}
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

  const carregar = useCallback(() => {
    api
      .visaoProjeto(projeto)
      .then((v) => {
        setVisao(v);
        setFalha(null);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto]);

  useEffect(carregar, [carregar]);

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

  if (falha) {
    return (
      <p className="error-text" role="alert">
        Não foi possível ler o projeto: {falha}
      </p>
    );
  }
  if (!visao) return <p className="muted">Carregando…</p>;

  // As regras, o glossário, as áreas e a conferência das evidências moraram aqui; agora
  // estão na Base (#328).
  return (
    <>
      <h2>
        {visao.nome}{" "}
        <button
          type="button"
          className="link"
          onClick={() => navegar({ tipo: "base", documento: visao.documento })}
        >
          abrir o projeto.md
        </button>
      </h2>

      <ListaDemandas api={api} projeto={projeto} navegar={navegar} />

      {api.cobertura && (
        <CoberturaProjeto
          api={api as ApiCobertura}
          projeto={projeto}
          navegar={navegar}
          versao={visao}
        />
      )}

      <MapaProjeto api={api} projeto={projeto} />
    </>
  );
}

/** Mapa estrutural por pasta (sem IA, sem custo), com as subpastas navegáveis. */
export function MapaProjeto({ api, projeto }: { api: ApiConhecimento; projeto: string }) {
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

  /**
   * Entra numa subpasta pulando as cadeias de pasta única (sem arquivo próprio e com
   * uma só subpasta): `src/main/java/com/x` não pede um clique por pasta (#227).
   */
  const entrar = async (caminho: string) => {
    let atual = caminho;
    for (let i = 0; i < 64; i++) {
      const m = await api.mapa(projeto, atual).catch(() => null);
      const sub = m?.modulo.subpastas;
      if (!m || m.modulo.arquivosDiretos.length > 0 || sub?.length !== 1) break;
      atual = sub[0].caminho;
    }
    setPasta(atual);
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
            {contar(mapa.modulo.arquivos, "arquivo", "arquivos")}
            {mapa.modulo.linguagens.length > 0 &&
              `: ${mapa.modulo.linguagens.map((l) => `${l.rotulo} ${l.quantidade}`).join(", ")}`}
            {mapa.modulo.falhas > 0 && ` · ${mapa.modulo.falhas} com falha de extração`}
          </p>
          {mapa.modulo.simbolos.length > 0 && (
            <p className="muted">
              {simbolosEmNegocio(mapa.modulo.simbolos)
                .map((s) => `${s.quantidade} ${s.rotulo}`)
                .join(" · ")}
            </p>
          )}
          {mapa.modulo.subpastas.length > 0 && (
            <ul className="lista-pastas">
              {mapa.modulo.subpastas.map((s) => (
                <li key={s.caminho}>
                  <button type="button" className="link" onClick={() => void entrar(s.caminho)}>
                    {s.caminho.split("/").pop()}/
                  </button>{" "}
                  <span className="muted">{contar(s.arquivos, "arquivo", "arquivos")}</span>
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
                    <span className="muted">{contar(d.referencias, "referência", "referências")}</span>
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
