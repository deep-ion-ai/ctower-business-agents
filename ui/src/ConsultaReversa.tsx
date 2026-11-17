import { useEffect, useState, type ReactNode } from "react";
import { mensagemDe } from "./analise";
import {
  apiRastreabilidadeTauri,
  type AlvoReverso,
  type ApiRastreabilidade,
  type Dependentes,
} from "./rastreabilidade";
import "./rastreabilidade.css";

const ROTULO_ESTADO: Record<string, string> = {
  criada: "criada",
  "em-andamento": "em andamento",
  concluida: "concluída",
  aprovada: "aprovada",
  rascunho: "rascunho",
  "em-revisao": "em revisão",
};

/**
 * Consulta reversa (#121), na tela da regra: o que depende da regra (demandas,
 * análises, critérios de aceite e cenários de teste) ou, pelo seletor, de um dos
 * arquivos das evidências dela. É a mesma consulta da ferramenta MCP
 * `rastreabilidade`, derivada do índice da base.
 */
export function ConsultaReversa({
  projeto,
  regra,
  aoAbrirDocumento,
  aoAbrirAnalise,
  api = apiRastreabilidadeTauri,
}: {
  /** Pasta do projeto na base. */
  projeto: string;
  /** `RN-FAT-007`. */
  regra: string;
  aoAbrirDocumento: (caminho: string) => void;
  aoAbrirAnalise?: (projeto: string, id: string) => void;
  api?: ApiRastreabilidade;
}) {
  const [alvo, setAlvo] = useState<AlvoReverso>({ tipo: "regra", valor: regra });
  const [arquivos, setArquivos] = useState<string[]>([]);
  const [dependentes, setDependentes] = useState<Dependentes | null>(null);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    setAlvo({ tipo: "regra", valor: regra });
    setArquivos([]);
  }, [regra]);

  useEffect(() => {
    let ativo = true;
    api
      .dependentes(projeto, alvo)
      .then((d) => {
        if (!ativo || !d) return;
        setDependentes(d);
        setFalha(null);
        if (alvo.tipo === "regra") setArquivos(d.arquivos ?? []);
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto, alvo]);

  const abrirAnalise = (id: string, documento: string | null) => {
    if (aoAbrirAnalise) aoAbrirAnalise(projeto, id);
    else if (documento) aoAbrirDocumento(documento);
  };

  if (falha && !dependentes) {
    return (
      <section className="consulta-reversa" aria-label="O que depende desta regra">
        <p className="error-text" role="alert">
          Não foi possível consultar o que depende de {regra}: {falha}
        </p>
      </section>
    );
  }
  if (!dependentes) return null;
  const d = dependentes;
  const doArquivo = alvo.tipo === "arquivo";
  const contagem = (n: number, um: string, varios: string) => `${n} ${n === 1 ? um : varios}`;

  return (
    <section
      className="consulta-reversa"
      aria-label={doArquivo ? "O que depende deste arquivo" : "O que depende desta regra"}
    >
      <header className="consulta-reversa-cabecalho">
        <h3>{doArquivo ? "O que depende deste arquivo" : "O que depende desta regra"}</h3>
        <span className="badge">{contagem(d.demandas.length, "demanda", "demandas")}</span>
        <span className="badge">{contagem(d.analises.length, "análise", "análises")}</span>
        <span className="badge">{contagem(d.criterios.length, "critério", "critérios")}</span>
        <span className="badge">{contagem(d.cenarios.length, "cenário", "cenários")}</span>
        <div className="consulta-reversa-alvo" role="group" aria-label="Consultar por">
          <button
            type="button"
            className="btn"
            aria-pressed={!doArquivo}
            onClick={() => setAlvo({ tipo: "regra", valor: regra })}
          >
            Regra
          </button>
          {arquivos.map((a) => (
            <button
              key={a}
              type="button"
              className="btn"
              aria-pressed={doArquivo && alvo.valor === a}
              onClick={() => setAlvo({ tipo: "arquivo", valor: a })}
            >
              Arquivo <span className="mono">{a}</span>
            </button>
          ))}
        </div>
      </header>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {d.demandas.length + d.analises.length + d.criterios.length + d.cenarios.length === 0 ? (
        <p className="muted">
          Nada na base depende {doArquivo ? "deste arquivo" : "desta regra"} ainda: nenhuma
          demanda, análise, critério ou cenário a cita.
        </p>
      ) : (
        <>
          <p className="muted">
            Se {doArquivo ? "este arquivo" : "esta regra"} mudar, estes itens precisam ser
            revistos. Abertos primeiro; aprovados e encerrados em seguida.
          </p>
          <div className="consulta-reversa-grupos">
            {doArquivo && d.regras.length > 0 && (
              <Grupo titulo="Regras com evidência no arquivo">
                {d.regras.map((r) => (
                  <li key={r.id}>
                    {r.documento ? (
                      <button
                        type="button"
                        className="link mono"
                        onClick={() => aoAbrirDocumento(r.documento!)}
                      >
                        {r.id}
                      </button>
                    ) : (
                      <span className="mono">{r.id}</span>
                    )}{" "}
                    {r.titulo}
                  </li>
                ))}
              </Grupo>
            )}
            <Grupo titulo="Demandas">
              {d.demandas.map((x) => (
                <li key={x.id}>
                  {x.documento ? (
                    <button
                      type="button"
                      className="link mono"
                      onClick={() => aoAbrirDocumento(x.documento!)}
                    >
                      {x.id}
                    </button>
                  ) : (
                    <span className="mono">{x.id}</span>
                  )}{" "}
                  {x.titulo}{" "}
                  {x.status && (
                    <span className="badge" data-tone={x.status === "aberta" ? "running" : "done"}>
                      {x.status}
                    </span>
                  )}
                </li>
              ))}
            </Grupo>
            <Grupo titulo="Análises">
              {d.analises.map((a) => (
                <li key={a.id}>
                  <button
                    type="button"
                    className="link mono"
                    onClick={() => abrirAnalise(a.id, a.documento)}
                  >
                    {a.id}
                  </button>{" "}
                  {a.titulo}{" "}
                  {a.estado && (
                    <span
                      className="badge"
                      data-tone={a.estado === "aprovada" ? "done" : "running"}
                    >
                      {ROTULO_ESTADO[a.estado] ?? a.estado}
                    </span>
                  )}
                </li>
              ))}
            </Grupo>
            <Grupo titulo="Histórias e critérios">
              {d.criterios.map((c) => (
                <li key={`${c.historia}/${c.criterio}`}>
                  <button
                    type="button"
                    className="link mono"
                    onClick={() => abrirAnalise(c.analise, null)}
                  >
                    {c.historia} {c.criterio}
                  </button>{" "}
                  {c.texto}{" "}
                  {c.aRevalidar && (
                    <span className="badge" data-tone="attention">
                      revisar
                    </span>
                  )}
                </li>
              ))}
            </Grupo>
            <Grupo titulo="Cenários de teste">
              {d.cenarios.map((c) => (
                <li key={c.id}>
                  <button
                    type="button"
                    className="link mono"
                    onClick={() => abrirAnalise(c.analise, null)}
                  >
                    {c.id}
                  </button>{" "}
                  {c.titulo}{" "}
                  <span className="badge" data-tone={c.aRevalidar ? "attention" : "done"}>
                    {c.aRevalidar ? "revisar" : "ok"}
                  </span>
                </li>
              ))}
            </Grupo>
          </div>
        </>
      )}
      <p className="muted consulta-reversa-mcp">
        O agente consulta o mesmo resultado com a ferramenta{" "}
        <code>
          rastreabilidade({doArquivo ? "arquivo" : "regra"}: "{alvo.valor}")
        </code>{" "}
        ao analisar impacto.
      </p>
    </section>
  );
}

function Grupo({ titulo, children }: { titulo: string; children: ReactNode[] }) {
  return (
    <section aria-label={titulo} className="consulta-reversa-grupo">
      <h4>{titulo}</h4>
      {children.length === 0 ? <p className="muted">Nenhum.</p> : <ul>{children}</ul>}
    </section>
  );
}
