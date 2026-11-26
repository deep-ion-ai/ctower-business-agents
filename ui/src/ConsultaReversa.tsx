import { useEffect, useState, type ReactNode } from "react";
import { mensagemDe } from "./analise";
import { Alerta, Cartao, EstadoVazio, Selo, TituloSecao, type TomSelo } from "./componentes";
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

/** O tom do selo da análise pelo significado: concluída ainda aguarda a revisão. */
const TOM_ESTADO: Record<string, TomSelo> = {
  "em-andamento": "andamento",
  concluida: "atencao",
  "em-revisao": "atencao",
  aprovada: "concluido",
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
  comCodigo = false,
}: {
  /** Pasta do projeto na base. */
  projeto: string;
  /** `RN-FAT-007`. */
  regra: string;
  aoAbrirDocumento: (caminho: string) => void;
  aoAbrirAnalise?: (projeto: string, id: string) => void;
  api?: ApiRastreabilidade;
  /** Mostra antes "Onde está no código" (#328): os arquivos das evidências da regra. */
  comCodigo?: boolean;
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
        <Alerta tom="erro" titulo={`Não foi possível consultar o que depende de ${regra}.`}>
          {falha}
        </Alerta>
      </section>
    );
  }
  if (!dependentes) return null;
  const d = dependentes;
  const doArquivo = alvo.tipo === "arquivo";
  const contagem = (n: number, um: string, varios: string) => `${n} ${n === 1 ? um : varios}`;

  return (
    <>
      {comCodigo && (
        <section className="onde-no-codigo" aria-label="Onde está no código">
          <TituloSecao nivel={3}>Onde está no código</TituloSecao>
          {arquivos.length === 0 ? (
            <EstadoVazio>Esta regra não cita trechos de código.</EstadoVazio>
          ) : (
            <ul>
              {arquivos.map((a) => (
                <li key={a}>
                  <span className="mono" title={a}>
                    {a.split("/").pop()}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}
      <section
        className="consulta-reversa"
        aria-label={doArquivo ? "O que depende deste arquivo" : "O que depende desta regra"}
      >
        <header className="consulta-reversa-cabecalho">
          <TituloSecao nivel={3}>
            {doArquivo ? "O que depende deste arquivo" : "Se esta regra mudar"}
          </TituloSecao>
          <Selo>{contagem(d.demandas.length, "demanda", "demandas")}</Selo>
          <Selo>{contagem(d.analises.length, "análise", "análises")}</Selo>
          <Selo>{contagem(d.criterios.length, "critério", "critérios")}</Selo>
          <Selo>{contagem(d.cenarios.length, "cenário", "cenários")}</Selo>
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
                Arquivo{" "}
                <span className="mono" title={a}>
                  {a.split("/").pop()}
                </span>
              </button>
            ))}
          </div>
        </header>
        {falha && <Alerta tom="erro" titulo={falha} />}
        {d.demandas.length + d.analises.length + d.criterios.length + d.cenarios.length === 0 ? (
          <EstadoVazio>
            Nada na base depende {doArquivo ? "deste arquivo" : "desta regra"} ainda: nenhuma
            demanda, análise, critério ou cenário a cita.
          </EstadoVazio>
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
                      <Selo tom={x.status === "aberta" ? "andamento" : "concluido"}>{x.status}</Selo>
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
                      <Selo tom={TOM_ESTADO[a.estado] ?? "neutro"}>
                        {ROTULO_ESTADO[a.estado] ?? a.estado}
                      </Selo>
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
                    {c.aRevalidar && <Selo tom="atencao">revisar</Selo>}
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
                    <Selo tom={c.aRevalidar ? "atencao" : "concluido"}>
                      {c.aRevalidar ? "revisar" : "ok"}
                    </Selo>
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
    </>
  );
}

function Grupo({ titulo, children }: { titulo: string; children: ReactNode[] }) {
  return (
    <Cartao rotulo={titulo} className="consulta-reversa-grupo">
      <TituloSecao nivel={4}>{titulo}</TituloSecao>
      {children.length === 0 ? <EstadoVazio>Nenhum.</EstadoVazio> : <ul>{children}</ul>}
    </Cartao>
  );
}
