import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { mensagemDe } from "./analise";
import {
  partesDoTrecho,
  rotuloTipo,
  type ApiConhecimento,
  type Busca,
  type DocumentoAberto,
  type FiltroTipo,
} from "./conhecimento";
import { DocumentoBase } from "./DocumentoBase";
import type { Tela } from "./Navegacao";
import type { SistemaResumo } from "./terminal";

const FILTROS: { tipo: FiltroTipo; rotulo: string }[] = [
  { tipo: "regra", rotulo: "Regras" },
  { tipo: "termo", rotulo: "Termos" },
  { tipo: "analise", rotulo: "Análises" },
];

/** Por página, igual ao `POR_PAGINA` de `base.rs`. */
const POR_PAGINA = 20;

/**
 * Base de conhecimento (spec, seções 7.5 e 10): busca em regras, termos e análises e
 * abertura do documento, com os links entre documentos navegáveis. Quando a base
 * muda fora do app, a busca e o documento aberto são relidos.
 */
export function TelaBase({
  api,
  sistemas,
  documentoInicial,
  navegar,
}: {
  api: ApiConhecimento;
  sistemas: SistemaResumo[];
  documentoInicial?: string;
  navegar: (t: Tela) => void;
}) {
  const [texto, setTexto] = useState("");
  const [sistema, setSistema] = useState("");
  const [tipos, setTipos] = useState<FiltroTipo[]>([]);
  const [busca, setBusca] = useState<Busca | null>(null);
  const [consulta, setConsulta] = useState<{ texto: string; deslocamento: number } | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [historico, setHistorico] = useState<string[]>(documentoInicial ? [documentoInicial] : []);
  const [documento, setDocumento] = useState<DocumentoAberto | null>(null);
  const [falhaDocumento, setFalhaDocumento] = useState<string | null>(null);
  const [rodada, setRodada] = useState(0);
  const aberto = historico[historico.length - 1];
  const topo = useRef<HTMLDivElement>(null);

  // Busca.
  useEffect(() => {
    if (!consulta) return;
    let ativo = true;
    api
      .buscar(consulta.texto, {
        sistema: sistema || undefined,
        tipos,
        deslocamento: consulta.deslocamento,
      })
      .then((b) => {
        if (!ativo) return;
        setBusca(b);
        setFalha(null);
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, consulta, sistema, tipos, rodada]);

  // Documento aberto.
  useEffect(() => {
    if (!aberto) {
      setDocumento(null);
      return;
    }
    let ativo = true;
    api
      .abrirDocumento(aberto)
      .then((d) => {
        if (!ativo) return;
        setDocumento(d);
        setFalhaDocumento(null);
        topo.current?.scrollIntoView?.({ block: "start" });
      })
      .catch((e: unknown) => ativo && setFalhaDocumento(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, aberto, rodada]);

  // Edições fora do app: relê.
  useEffect(() => {
    let parar: (() => void) | null = null;
    let ativo = true;
    api
      .vigiarBase(() => setRodada((r) => r + 1))
      .then((p) => (ativo ? (parar = p) : p()))
      .catch(() => undefined);
    return () => {
      ativo = false;
      parar?.();
    };
  }, [api]);

  const buscar = (ev: FormEvent) => {
    ev.preventDefault();
    if (!texto.trim()) return;
    setConsulta({ texto: texto.trim(), deslocamento: 0 });
  };

  const alternar = (t: FiltroTipo) =>
    setTipos((atual) => (atual.includes(t) ? atual.filter((x) => x !== t) : [...atual, t]));

  const abrir = useCallback((caminho: string) => setHistorico((h) => [...h, caminho]), []);
  const voltar = () => setHistorico((h) => h.slice(0, -1));

  return (
    <section className="tela-base" aria-label="Base de conhecimento">
      <h2>Base de conhecimento</h2>
      <form className="busca-base" role="search" onSubmit={buscar} aria-label="Busca na base">
        <input
          type="search"
          aria-label="Buscar"
          placeholder="Ex.: multa atraso, carência, RN-COB-001"
          value={texto}
          onChange={(e) => setTexto(e.target.value)}
        />
        <button type="submit" className="primario" disabled={!texto.trim()}>
          Buscar
        </button>
        <label>
          Sistema{" "}
          <select value={sistema} onChange={(e) => setSistema(e.target.value)}>
            <option value="">Todos</option>
            {sistemas.map((s) => (
              <option key={s.id} value={s.id}>
                {s.nome}
              </option>
            ))}
          </select>
        </label>
        <fieldset className="filtros">
          <legend className="suave">Só</legend>
          {FILTROS.map((f) => (
            <label key={f.tipo}>
              <input
                type="checkbox"
                checked={tipos.includes(f.tipo)}
                onChange={() => alternar(f.tipo)}
              />{" "}
              {f.rotulo}
            </label>
          ))}
        </fieldset>
      </form>
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}

      <div className="base-colunas">
        {busca && consulta && (
          <section aria-label="Resultados" className="resultados-base">
            <p className="suave" role="status">
              {busca.total === 0
                ? `Nada encontrado para "${consulta.texto}".`
                : `${busca.total} documento(s) para "${consulta.texto}"${
                    busca.qualquerPalavra
                      ? " (nenhum tinha todas as palavras; estes têm ao menos uma)"
                      : ""
                  }.`}
            </p>
            <ul>
              {busca.resultados.map((r) => (
                <li key={r.caminho} aria-current={r.caminho === aberto ? "true" : undefined}>
                  <button type="button" className="link" onClick={() => abrir(r.caminho)}>
                    {r.identificador && r.identificador !== r.titulo && `${r.identificador}: `}
                    {r.titulo}
                  </button>{" "}
                  <span className="suave">
                    {rotuloTipo(r.tipo)} · {r.sistema}
                    {r.area && ` · ${r.area}`}
                  </span>
                  {r.status === "a-revalidar" && (
                    <span className="selo selo-revalidar">a revalidar</span>
                  )}
                  {r.trecho && (
                    <p className="trecho-busca">
                      {partesDoTrecho(r.trecho).map((p, i) =>
                        p.destaque ? <mark key={i}>{p.texto}</mark> : <span key={i}>{p.texto}</span>,
                      )}
                    </p>
                  )}
                </li>
              ))}
            </ul>
            {busca.total > POR_PAGINA && (
              <div className="acoes">
                <button
                  type="button"
                  disabled={busca.deslocamento === 0}
                  onClick={() =>
                    setConsulta({
                      texto: consulta.texto,
                      deslocamento: Math.max(0, busca.deslocamento - POR_PAGINA),
                    })
                  }
                >
                  Anteriores
                </button>
                <button
                  type="button"
                  disabled={busca.deslocamento + POR_PAGINA >= busca.total}
                  onClick={() =>
                    setConsulta({
                      texto: consulta.texto,
                      deslocamento: busca.deslocamento + POR_PAGINA,
                    })
                  }
                >
                  Próximos
                </button>
              </div>
            )}
          </section>
        )}

        {aberto && (
          <div className="documento-aberto" ref={topo}>
            <div className="acoes">
              {historico.length > 1 && (
                <button type="button" onClick={voltar}>
                  Voltar
                </button>
              )}
              <button type="button" onClick={() => setHistorico([])}>
                Fechar o documento
              </button>
            </div>
            {falhaDocumento && (
              <p className="trecho-erro" role="alert">
                Não foi possível abrir {aberto}: {falhaDocumento}
              </p>
            )}
            {documento && !falhaDocumento && (
              <DocumentoBase
                documento={documento}
                aoAbrirDocumento={abrir}
                aoAbrirAnalise={(s, id) =>
                  navegar({
                    tipo: "analise",
                    sistema: s,
                    aberta: { id, abertura: { tipo: "reabrir" } },
                  })
                }
              />
            )}
          </div>
        )}
      </div>
    </section>
  );
}
