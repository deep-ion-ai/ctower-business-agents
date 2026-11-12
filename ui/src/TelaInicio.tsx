import { useCallback, useEffect, useState } from "react";
import { mensagemDe, type Fluxo } from "./analise";
import { ClientesIA, type AbrirLink } from "./ClientesIA";
import {
  ROTULO_FLUXO,
  type AnaliseInicio,
  type ApiConhecimento,
  type Inicio,
} from "./conhecimento";
import { ROTULO_SESSAO } from "./DocumentoAoVivo";
import type { Tela } from "./Navegacao";
import { NovaAnalise } from "./NovaAnalise";

const FLUXOS: Fluxo[] = ["perguntar", "explicar", "impacto"];

const ESTADO: Record<AnaliseInicio["estado"], string> = {
  criada: "criada",
  "em-andamento": "em andamento",
  concluida: "concluída, aguardando revisão",
  aprovada: "aprovada",
};

interface Props {
  api: ApiConhecimento;
  navegar: (t: Tela) => void;
  abrirLink?: AbrirLink;
  aoPrimeiroUso?: () => void;
}

/**
 * Início (spec, seção 10): os botões dos fluxos, as análises em andamento e as
 * recentes, os sistemas cadastrados e o estado dos clientes de IA.
 */
export function TelaInicio({
  api,
  navegar,
  abrirLink,
  aoPrimeiroUso,
}: Props) {
  const [inicio, setInicio] = useState<Inicio | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [fluxo, setFluxo] = useState<Fluxo | null>(null);

  const carregar = useCallback(() => {
    api
      .inicio()
      .then((i) => {
        setInicio(i);
        setFalha(null);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api]);

  useEffect(carregar, [carregar]);

  const abrirAnalise = (a: AnaliseInicio) =>
    navegar({
      tipo: "analise",
      sistema: a.sistema,
      aberta: { id: a.id, abertura: { tipo: "reabrir" } },
    });

  const semSistema = inicio !== null && (inicio.base === null || inicio.sistemas.length === 0);

  return (
    <section className="tela-inicio" aria-label="Início">
      {falha && (
        <p className="trecho-erro" role="alert">
          Não foi possível carregar o início: {falha}
        </p>
      )}
      {semSistema && (
        <p>
          {inicio.base === null
            ? "Falta escolher a pasta da base de conhecimento."
            : "Nenhum sistema cadastrado."}{" "}
          {aoPrimeiroUso && (
            <button type="button" onClick={aoPrimeiroUso}>
              Abrir o primeiro uso
            </button>
          )}
        </p>
      )}

      <section aria-label="Nova análise" className="fluxos">
        <h2>Nova análise</h2>
        <div className="botoes-fluxo">
          {FLUXOS.map((f) => (
            <button
              key={f}
              type="button"
              className={fluxo === f ? "primario" : undefined}
              aria-pressed={fluxo === f}
              disabled={!inicio || semSistema}
              onClick={() => setFluxo(fluxo === f ? null : f)}
            >
              {ROTULO_FLUXO[f]}
            </button>
          ))}
        </div>
        {fluxo && inicio && !semSistema && (
          <NovaAnalise
            key={fluxo}
            api={api}
            fluxo={fluxo}
            sistemas={inicio.sistemas}
            aoCancelar={() => setFluxo(null)}
            aoCriar={(sistema, criada) =>
              navegar({
                tipo: "analise",
                sistema,
                aberta: {
                  id: criada.analise.id,
                  abertura: { tipo: "nova", promptInicial: criada.promptInicial },
                },
              })
            }
          />
        )}
      </section>

      {inicio && (
        <>
          <ListaAnalises
            titulo="Em andamento"
            vazio="Nenhuma análise em andamento."
            analises={inicio.emAndamento}
            aoAbrir={abrirAnalise}
          />
          <ListaAnalises
            titulo="Recentes"
            vazio="Nenhuma análise ainda."
            analises={inicio.recentes}
            aoAbrir={abrirAnalise}
          />
          <section aria-label="Sistemas">
            <h2>Sistemas</h2>
            <ul className="lista-sistemas">
              {inicio.sistemas.map((s) => (
                <li key={s.id}>
                  <button type="button" onClick={() => navegar({ tipo: "sistema", sistema: s.id })}>
                    {s.nome}
                  </button>{" "}
                  {s.erro ? (
                    <span className="trecho-erro">{s.erro}</span>
                  ) : (
                    <span className="suave">
                      {s.analises} análise(s) · {s.regras} regra(s) · {s.termos} termo(s)
                    </span>
                  )}
                  {s.aRevalidar > 0 && (
                    <span className="selo selo-revalidar">{s.aRevalidar} a revalidar</span>
                  )}
                </li>
              ))}
            </ul>
          </section>
        </>
      )}

      <ClientesIA abrirLink={abrirLink} />
    </section>
  );
}

function ListaAnalises({
  titulo,
  vazio,
  analises,
  aoAbrir,
}: {
  titulo: string;
  vazio: string;
  analises: AnaliseInicio[];
  aoAbrir: (a: AnaliseInicio) => void;
}) {
  return (
    <section aria-label={titulo}>
      <h2>{titulo}</h2>
      {analises.length === 0 && <p className="suave">{vazio}</p>}
      {analises.length > 0 && (
        <ul className="lista-analises">
          {analises.map((a) => (
            <li key={`${a.sistema}/${a.id}`}>
              <button type="button" onClick={() => aoAbrir(a)}>
                {a.id}: {a.titulo}
              </button>{" "}
              <span className="suave">
                {a.sistemaNome} · {ROTULO_FLUXO[a.fluxo].toLowerCase()} · {ESTADO[a.estado]}
                {a.sessao && a.sessao.estado !== "encerrada" && ` · ${ROTULO_SESSAO[a.sessao.estado]}`}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
