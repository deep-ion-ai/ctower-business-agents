import { useCallback, useEffect, useState } from "react";
import { mensagemDe, type AnaliseCriada, type Fluxo } from "./analise";
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
import { NovaReuniao } from "./NovaReuniao";
import type { ApiReuniao } from "./reunioes";

const FLUXOS: Fluxo[] = ["perguntar", "explicar", "impacto", "reuniao"];

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
  /** Nova reunião (#141); para testes. */
  apiReuniao?: ApiReuniao;
}

/**
 * Início (spec, seção 10): os botões dos fluxos, as análises em andamento e as
 * recentes, os projetos cadastrados e o estado dos clientes de IA.
 */
export function TelaInicio({
  api,
  navegar,
  abrirLink,
  aoPrimeiroUso,
  apiReuniao,
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
      projeto: a.projeto,
      aberta: { id: a.id, abertura: { tipo: "reabrir" } },
    });

  const abrirCriada = (projeto: string, criada: AnaliseCriada) =>
    navegar({
      tipo: "analise",
      projeto,
      aberta: {
        id: criada.analise.id,
        abertura: { tipo: "nova", promptInicial: criada.promptInicial },
      },
    });

  const semProjeto = inicio !== null && (inicio.base === null || inicio.projetos.length === 0);

  return (
    <section className="tela-inicio page" aria-label="Início">
      {falha && (
        <p className="error-text" role="alert">
          Não foi possível carregar o início: {falha}
        </p>
      )}
      {semProjeto && (
        <p>
          {inicio.base === null
            ? "Falta escolher a pasta da base de conhecimento."
            : "Nenhum projeto cadastrado."}{" "}
          {aoPrimeiroUso && (
            <button className="btn" type="button" onClick={aoPrimeiroUso}>
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
              className="btn"
              aria-pressed={fluxo === f}
              disabled={!inicio || semProjeto}
              onClick={() => setFluxo(fluxo === f ? null : f)}
            >
              {ROTULO_FLUXO[f]}
            </button>
          ))}
        </div>
        {fluxo === "reuniao" && inicio && !semProjeto && (
          <NovaReuniao
            api={api}
            apiReuniao={apiReuniao}
            projetos={inicio.projetos}
            aoCancelar={() => setFluxo(null)}
            aoCriar={abrirCriada}
          />
        )}
        {fluxo && fluxo !== "reuniao" && inicio && !semProjeto && (
          <NovaAnalise
            key={fluxo}
            api={api}
            fluxo={fluxo}
            projetos={inicio.projetos}
            aoCancelar={() => setFluxo(null)}
            aoCriar={abrirCriada}
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
          <section aria-label="Projetos">
            <h2>Projetos</h2>
            <ul className="lista-projetos">
              {inicio.projetos.map((s) => (
                <li key={s.id}>
                  <button
                    className="link"
                    type="button"
                    onClick={() => navegar({ tipo: "projeto", projeto: s.id })}
                  >
                    {s.nome}
                  </button>{" "}
                  {s.erro ? (
                    <span className="error-text">{s.erro}</span>
                  ) : (
                    <span className="muted">
                      {s.analises} análise(s) · {s.regras} regra(s) · {s.termos} termo(s)
                    </span>
                  )}
                  {s.aRevalidar > 0 && (
                    <span className="badge" data-tone="attention">
                      {s.aRevalidar} a revalidar
                    </span>
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
      {analises.length === 0 && <p className="muted">{vazio}</p>}
      {analises.length > 0 && (
        <ul className="lista-analises">
          {analises.map((a) => (
            <li key={`${a.projeto}/${a.id}`}>
              <button className="link" type="button" onClick={() => aoAbrir(a)}>
                {a.id}: {a.titulo}
              </button>{" "}
              <span className="muted">
                {a.projetoNome} · {ROTULO_FLUXO[a.fluxo].toLowerCase()} · {ESTADO[a.estado]}
                {a.sessao && a.sessao.estado !== "encerrada" && ` · ${ROTULO_SESSAO[a.sessao.estado]}`}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
