import { useCallback, useEffect, useState, type ReactNode } from "react";
import { mensagemDe, type AnaliseCriada, type EstadoAnalise, type Fluxo } from "./analise";
import { ROTULO_FLUXO, type ApiConhecimento } from "./conhecimento";
import {
  ROTULO_STATUS_ATA,
  ROTULO_STATUS_DEMANDA,
  type ItemDaAta,
  type ItemLinhaDoTempo,
  type PainelDemanda as Painel,
  type TranscriptGuardado,
} from "./demandas";
import { ROTULO_SESSAO } from "./DocumentoAoVivo";
import type { Tela } from "./Navegacao";
import { NovaAnalise } from "./NovaAnalise";
import { NovaReuniao } from "./NovaReuniao";
import { BotoesProximoPasso } from "./ProximoPasso";
import { apiReuniaoTauri, type ApiReuniao } from "./reunioes";

const FLUXOS: Fluxo[] = ["perguntar", "explicar", "impacto", "reuniao"];

const ESTADO_ANALISE: Record<EstadoAnalise, string> = {
  criada: "criada",
  "em-andamento": "em andamento",
  concluida: "concluída, aguardando revisão",
  aprovada: "aprovada",
};

/**
 * Painel da demanda (#138): o lugar onde o analista acompanha um pedido do cliente
 * sem se perder em conversas espalhadas. Cabeçalho (id, título, status, chave), a
 * linha do tempo em ordem cronológica (atas, análises e sessões do Claude), as
 * pendências, as dúvidas em aberto e os próximos passos das atas aprovadas (cada
 * passo com os botões que abrem análises ligadas à demanda, #142), e a criação de
 * uma análise já ligada à demanda.
 */
export function PainelDemanda({
  api,
  projeto,
  demanda,
  navegar,
  apiReuniao,
}: {
  api: ApiConhecimento;
  projeto: string;
  demanda: string;
  navegar: (t: Tela) => void;
  /** Nova reunião (#141) e próximos passos (#142); para testes. */
  apiReuniao?: ApiReuniao;
}) {
  const [painel, setPainel] = useState<Painel | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [fluxo, setFluxo] = useState<Fluxo | null>(null);
  const [mudando, setMudando] = useState(false);

  const carregar = useCallback(() => {
    api
      .painelDemanda(projeto, demanda)
      .then((p) => {
        setPainel(p);
        setFalha(null);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto, demanda]);

  useEffect(carregar, [carregar]);

  // Ata registrada pelo Claude, análise criada em outra aba, `git pull` na base: relê.
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

  const abrirCriada = (p: string, criada: AnaliseCriada) =>
    navegar({
      tipo: "analise",
      projeto: p,
      aberta: {
        id: criada.analise.id,
        abertura: { tipo: "nova", promptInicial: criada.promptInicial },
      },
    });
  const abrirDocumento = (caminho: string) => navegar({ tipo: "base", documento: caminho });
  const abrirAnalise = (id: string) =>
    navegar({ tipo: "analise", projeto, aberta: { id, abertura: { tipo: "reabrir" } } });

  if (falha && !painel) {
    return (
      <section className="painel-demanda page" aria-label="Demanda">
        <p className="error-text" role="alert">
          Não foi possível ler a demanda {demanda}: {falha}
        </p>
      </section>
    );
  }
  if (!painel) {
    return (
      <section className="painel-demanda page" aria-label="Demanda">
        <p className="muted">Carregando…</p>
      </section>
    );
  }

  const d = painel.demanda;
  const aberta = d.status === "aberta";

  const mudarStatus = () => {
    setMudando(true);
    setFalha(null);
    (aberta ? api.encerrarDemanda(projeto, d.id) : api.reabrirDemanda(projeto, d.id))
      .then(carregar)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setMudando(false));
  };

  return (
    <section className="painel-demanda page" aria-label="Demanda">
      <header className="cabecalho-demanda">
        <h2>
          {d.id}: {d.titulo}
        </h2>
        <p>
          <span className="badge" data-tone={aberta ? "running" : "done"}>
            {ROTULO_STATUS_DEMANDA[d.status]}
          </span>{" "}
          {d.chave && <span className="muted">Chave {d.chave} · </span>}
          <span className="muted">
            criada em {d.criado}
            {d.encerrado && ` · encerrada em ${d.encerrado}`}
          </span>{" "}
          <button type="button" className="link" onClick={() => abrirDocumento(d.documento)}>
            abrir o demanda.md
          </button>{" "}
          <button type="button" className="btn" onClick={mudarStatus} disabled={mudando}>
            {aberta ? "Encerrar a demanda" : "Reabrir a demanda"}
          </button>
        </p>
      </header>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}

      <section aria-label="Nova análise da demanda" className="fluxos">
        <h3>Nova análise</h3>
        {aberta ? (
          <>
            <div className="botoes-fluxo">
              {FLUXOS.map((f) => (
                <button
                  key={f}
                  type="button"
                  className="btn"
                  aria-pressed={fluxo === f}
                  onClick={() => setFluxo(fluxo === f ? null : f)}
                >
                  {ROTULO_FLUXO[f]}
                </button>
              ))}
            </div>
            {fluxo === "reuniao" && (
              <NovaReuniao
                api={api}
                apiReuniao={apiReuniao}
                projetos={[{ id: projeto, nome: projeto }]}
                demandaFixa={d.id}
                aoCancelar={() => setFluxo(null)}
                aoCriar={abrirCriada}
              />
            )}
            {fluxo && fluxo !== "reuniao" && (
              <NovaAnalise
                key={fluxo}
                api={api}
                fluxo={fluxo}
                projetos={[{ id: projeto, nome: projeto }]}
                demandaFixa={d.id}
                aoCancelar={() => setFluxo(null)}
                aoCriar={abrirCriada}
              />
            )}
          </>
        ) : (
          <p className="muted">Reabra a demanda para criar uma análise ligada a ela.</p>
        )}
      </section>

      <section aria-label="Linha do tempo">
        <h3>Linha do tempo</h3>
        {painel.linhaDoTempo.length === 0 ? (
          <p className="muted">
            Nada ainda: as atas das reuniões, as análises e as sessões do Claude ligadas à demanda
            aparecem aqui.
          </p>
        ) : (
          <ol className="linha-do-tempo">
            {painel.linhaDoTempo.map((item) => (
              <li key={`${item.tipo}:${item.id}`} data-tipo={item.tipo}>
                <span className="muted mono">{item.data}</span>{" "}
                <EventoDaLinha
                  item={item}
                  abrirDocumento={abrirDocumento}
                  abrirAnalise={abrirAnalise}
                />
              </li>
            ))}
          </ol>
        )}
      </section>

      <ItensDasAtas
        titulo="Pendências"
        itens={painel.pendencias}
        vazio="Nenhuma pendência nas atas aprovadas."
      />
      <ItensDasAtas
        titulo="Dúvidas em aberto"
        itens={painel.duvidas}
        vazio="Nenhuma dúvida em aberto nas atas aprovadas."
      />
      <ItensDasAtas
        titulo="Próximos passos"
        itens={painel.proximosPassos}
        vazio="Nenhum próximo passo nas atas aprovadas."
        acoes={
          aberta
            ? (i) => (
                <BotoesProximoPasso
                  api={apiReuniao ?? apiReuniaoTauri}
                  projeto={projeto}
                  reuniao={i.reuniao}
                  passo={i}
                  aoCriar={(c) => abrirCriada(projeto, c)}
                />
              )
            : undefined
        }
      />

      {painel.problemas.length > 0 && (
        <section aria-label="Documentos ilegíveis">
          <h3>Documentos ilegíveis</h3>
          <ul>
            {painel.problemas.map((p) => (
              <li key={p.caminho}>
                <code>{p.caminho}</code>: {p.erro}
              </li>
            ))}
          </ul>
        </section>
      )}
    </section>
  );
}

function EventoDaLinha({
  item,
  abrirDocumento,
  abrirAnalise,
}: {
  item: ItemLinhaDoTempo;
  abrirDocumento: (caminho: string) => void;
  abrirAnalise: (id: string) => void;
}) {
  switch (item.tipo) {
    case "reuniao":
      return (
        <>
          <span>
            Reunião {item.id}
            {item.objetivo && `: ${item.objetivo}`}
          </span>{" "}
          <span className="badge" data-tone={item.status === "aprovada" ? "done" : "attention"}>
            ata {ROTULO_STATUS_ATA[item.status]}
          </span>{" "}
          <span className="muted">
            {item.participantes} participante(s) · {item.itens} item(ns)
          </span>
          {item.documento && (
            <>
              {" "}
              <button
                type="button"
                className="link"
                onClick={() => abrirDocumento(item.documento!)}
              >
                abrir a ata
              </button>
            </>
          )}
          {item.transcriptGuardado && <AvisoTranscriptGuardado guardado={item.transcriptGuardado} />}
        </>
      );
    case "analise":
      return (
        <>
          <button type="button" className="link" onClick={() => abrirAnalise(item.id)}>
            Análise {item.id}: {item.titulo}
          </button>{" "}
          <span className="muted">
            {ROTULO_FLUXO[item.fluxo]} · {ESTADO_ANALISE[item.estado]}
          </span>
        </>
      );
    case "sessao":
      return (
        <>
          <button type="button" className="link" onClick={() => abrirAnalise(item.analise)}>
            Sessão do Claude na análise {item.analise}
          </button>{" "}
          <span className="muted">{ROTULO_SESSAO[item.estado]}</span>
        </>
      );
  }
}

/**
 * Aviso de transcript guardado sem a ata aprovada, com a data em que ele é apagado
 * (`cba_reunioes::Guarda::situacao`).
 */
export function AvisoTranscriptGuardado({ guardado }: { guardado: TranscriptGuardado }) {
  return (
    <p
      className="aviso-transcript"
      role={guardado.venceEmBreve ? "alert" : "note"}
      data-tone={guardado.venceEmBreve ? "attention" : undefined}
    >
      Transcript guardado sem aprovação, apagado em {guardado.apagaEm}.
      {guardado.venceEmBreve && " Aprove a ata antes disso para guardar o que foi registrado."}
    </p>
  );
}

function ItensDasAtas({
  titulo,
  itens,
  vazio,
  acoes,
}: {
  titulo: string;
  itens: ItemDaAta[];
  vazio: string;
  /** Ações de cada item (os botões dos próximos passos). */
  acoes?: (i: ItemDaAta) => ReactNode;
}) {
  return (
    <section aria-label={titulo}>
      <h3>{titulo}</h3>
      {itens.length === 0 ? (
        <p className="muted">{vazio}</p>
      ) : (
        <ul className="lista-itens-ata">
          {itens.map((i) => (
            <li key={`${i.reuniao}:${i.id}`}>
              <span className="badge mono">{i.id}</span> <span>{i.texto}</span>
              {i.responsavel && <span className="muted"> · responsável: {i.responsavel}</span>}
              {i.prazo && <span className="muted"> · prazo: {i.prazo}</span>}
              <span className="muted">
                {" "}
                · reunião {i.reuniao} ({i.data})
              </span>
              {acoes?.(i)}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
