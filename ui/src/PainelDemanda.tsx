import { useCallback, useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { abrirLinkDoProjeto } from "./abrirLinkProjeto";
import {
  mensagemDe,
  type AnaliseCriada,
  type EstadoAnalise,
  type EstadoSessao,
  type Fluxo,
} from "./analise";
import {
  Alerta,
  BarraAcoes,
  EstadoVazio,
  ItemLista,
  ListaItens,
  MenuAcoes,
  Secao,
  Selo,
  type Acao,
  type TomSelo,
} from "./componentes";
import { ROTULO_FLUXO, type ApiConhecimento } from "./conhecimento";
import {
  quandoConferido,
  ROTULO_STATUS_ATA,
  ROTULO_STATUS_DEMANDA,
  ROTULO_TIPO_DEMANDA,
  type DemandaVista,
  type ItemDaAta,
  type ItemLinhaDoTempo,
  type PainelDemanda as Painel,
  type TranscriptGuardado,
} from "./demandas";
import { ROTULO_SESSAO } from "./DocumentoAoVivo";
import { EntregaveisDemanda } from "./EntregaveisDemanda";
import { apiEntregaveisDemandaTauri, type ApiEntregaveisDemanda } from "./entregaveisDemanda";
import { contar } from "./formatacao";
import { IconeReticencias } from "./icones";
import { MatrizRastreabilidade } from "./MatrizRastreabilidade";
import type { Tela } from "./Navegacao";
import { NovaAnalise } from "./NovaAnalise";
import { NovaHistorias } from "./NovaHistorias";
import { NovaPrototipo } from "./NovaPrototipo";
import { NovaReuniao } from "./NovaReuniao";
import { apiPautaTauri, type ApiPauta } from "./pauta";
import { PautaPerguntas } from "./PautaPerguntas";
import { BotoesProximoPasso } from "./ProximoPasso";
import { apiRastreabilidadeTauri, type ApiRastreabilidade } from "./rastreabilidade";
import { apiRegistrosTauri, type ApiRegistros } from "./registros";
import { RegistrosDemanda } from "./RegistrosDemanda";
import { apiReuniaoTauri, type ApiReuniao } from "./reunioes";
import "./painelDemanda.css";

/** Os tipos de "Nova análise ▾": "Analisar impacto" é a ação principal, fora do menu. */
const FLUXOS_DO_MENU: Fluxo[] = [
  "perguntar",
  "explicar",
  "reuniao",
  "revisar",
  "historias",
  "cenarios",
  "prototipo",
];

interface Situacao {
  texto: string;
  tom: TomSelo;
}

/** O selo da análise na linha do tempo: o tom vem do significado (#406). */
const SITUACAO_ANALISE: Record<EstadoAnalise, Situacao> = {
  criada: { texto: "criada", tom: "neutro" },
  "em-andamento": { texto: "em andamento", tom: "andamento" },
  concluida: { texto: "aguardando revisão", tom: "atencao" },
  aprovada: { texto: "aprovada", tom: "concluido" },
};

/** O selo da sessão do agente; a frase inteira fica na dica. */
const SITUACAO_SESSAO: Record<EstadoSessao, Situacao> = {
  ativa: { texto: "agente trabalhando", tom: "andamento" },
  "aguardando-analista": { texto: "aguardando você", tom: "atencao" },
  concluida: { texto: "resposta concluída", tom: "neutro" },
  encerrada: { texto: "encerrada", tom: "neutro" },
};

/** `2026-11-05` → `05/11/2026`; o texto como veio, se não for uma data. */
export function dataDoDia(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso);
  return m ? `${m[3]}/${m[2]}/${m[1]}` : iso;
}

/** "pendências, dúvidas ou próximos passos". */
function juntar(partes: string[]): string {
  if (partes.length <= 1) return partes.join("");
  return `${partes.slice(0, -1).join(", ")} ou ${partes[partes.length - 1]}`;
}

/** Leva à seção do contador: rola até ela e põe o foco nela (leitor de tela e teclado). */
function irPara(ancora: RefObject<HTMLDivElement>) {
  const el = ancora.current;
  if (!el) return;
  el.scrollIntoView?.({ behavior: "smooth", block: "start" });
  el.focus({ preventScroll: true });
}

/**
 * Painel da demanda (#138), reorganizado na #407: o lugar onde o analista acompanha
 * um pedido do cliente sem se perder em conversas espalhadas.
 *
 * - Cabeçalho: a trilha, o título, os metadados (situação, chave no rastreador,
 *   criação, conferência) e, à direita, a ação principal "Analisar impacto", o menu
 *   "Nova análise ▾" com os outros tipos e o "⋯" (demanda.md, exportações, encerrar).
 * - Resumo: só os contadores com valor, cada um leva à sua seção.
 * - Linha do tempo, pauta de perguntas, registros, rastreabilidade e entregáveis
 *   (#463), em largura de leitura.
 * - Pendências, dúvidas e próximos passos das atas aprovadas: cada um com a sua
 *   seção quando há itens; os vazios viram uma frase no fim.
 */
export function PainelDemanda({
  api,
  projeto,
  demanda,
  navegar,
  apiReuniao,
  apiRegistros = apiRegistrosTauri,
  apiRastreabilidade = apiRastreabilidadeTauri,
  apiPauta = apiPautaTauri,
  apiEntregaveis = apiEntregaveisDemandaTauri,
  abrirLink = abrirLinkDoProjeto,
}: {
  api: ApiConhecimento;
  projeto: string;
  demanda: string;
  navegar: (t: Tela) => void;
  /** Nova reunião (#141) e próximos passos (#142); para testes. */
  apiReuniao?: ApiReuniao;
  /** Premissas, decisões, riscos e restrições (#122); para testes. */
  apiRegistros?: ApiRegistros;
  /** Matriz de rastreabilidade (#121); para testes. */
  apiRastreabilidade?: ApiRastreabilidade;
  /** Pauta de perguntas pendentes (#123); para testes. */
  apiPauta?: ApiPauta;
  /** Entregáveis gerados a partir da análise validada (#463); para testes. */
  apiEntregaveis?: ApiEntregaveisDemanda;
  /** Abre a issue da demanda no navegador (#398); para testes. */
  abrirLink?: (projeto: string, link: string) => Promise<void>;
}) {
  const [painel, setPainel] = useState<Painel | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [fluxo, setFluxo] = useState<Fluxo | null>(null);
  const [mudando, setMudando] = useState(false);
  const [perguntas, setPerguntas] = useState(0);
  const [propostas, setPropostas] = useState(0);
  const [lacunas, setLacunas] = useState(0);
  const [comHistorias, setComHistorias] = useState(false);

  const ancoraAnalises = useRef<HTMLDivElement>(null);
  const ancoraPauta = useRef<HTMLDivElement>(null);
  const ancoraRegistros = useRef<HTMLDivElement>(null);
  const ancoraRastreabilidade = useRef<HTMLDivElement>(null);
  const formulario = useRef<HTMLDivElement>(null);

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

  // Ata registrada pelo agente, análise criada em outra aba, `git pull` na base: relê.
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

  // Contadores do resumo: as perguntas em aberto e as propostas do agente, relidas
  // junto com o painel (a pauta e os registros leem as mesmas listas).
  useEffect(() => {
    let ativo = true;
    apiPauta
      .pauta(projeto, demanda)
      .then((p) => {
        if (ativo && p && typeof p.total === "number") setPerguntas(p.total);
      })
      .catch(() => undefined);
    apiRegistros
      .registros(projeto, demanda)
      .then((r) => {
        if (ativo && r && Array.isArray(r.propostas)) setPropostas(r.propostas.length);
      })
      .catch(() => undefined);
    return () => {
      ativo = false;
    };
  }, [apiPauta, apiRegistros, projeto, demanda, painel]);

  // O formulário da análise abre logo abaixo do cabeçalho; vindo do fim da tela
  // ("Gerar histórias e critérios"), a tela rola até ele.
  useEffect(() => {
    if (fluxo) formulario.current?.scrollIntoView?.({ behavior: "smooth", block: "nearest" });
  }, [fluxo]);

  const aoCarregarMatriz = useCallback(
    (m: { lacunas: unknown[]; historias: unknown[] }) => {
      setLacunas(m.lacunas.length);
      setComHistorias(m.historias.length > 0);
    },
    [],
  );

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

  const exportar = (
    nome: string,
    escolher: () => Promise<string | null>,
    gravar: (destino: string) => Promise<string>,
  ) => {
    setAviso(null);
    setFalha(null);
    escolher()
      .then((destino) =>
        destino
          ? gravar(destino).then((gravado) => setAviso(`${nome} exportada em ${gravado}.`))
          : undefined,
      )
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  const semHistorias = comHistorias ? undefined : "A matriz ainda não tem histórias.";
  const semPerguntas = perguntas > 0 ? undefined : "Nenhuma pergunta em aberto.";
  const maisAcoes: Acao[] = [
    { texto: "Abrir demanda.md", aoEscolher: () => abrirDocumento(d.documento) },
    {
      texto: "Exportar a matriz para o Excel",
      desabilitado: !comHistorias,
      dica: semHistorias,
      aoEscolher: () =>
        exportar(
          "Matriz",
          () => apiRastreabilidade.escolherDestino(`matriz-${d.id}`, "xlsx"),
          (destino) => apiRastreabilidade.exportar(projeto, d.id, "xlsx", destino),
        ),
    },
    {
      texto: "Exportar a matriz para o Word",
      desabilitado: !comHistorias,
      dica: semHistorias,
      aoEscolher: () =>
        exportar(
          "Matriz",
          () => apiRastreabilidade.escolherDestino(`matriz-${d.id}`, "word"),
          (destino) => apiRastreabilidade.exportar(projeto, d.id, "word", destino),
        ),
    },
    {
      texto: "Exportar a pauta em Markdown",
      desabilitado: perguntas === 0,
      dica: semPerguntas,
      aoEscolher: () =>
        exportar(
          "Pauta",
          () => apiPauta.escolherDestino(`pauta-${d.id}`, "markdown"),
          (destino) => apiPauta.exportar(projeto, d.id, "markdown", destino),
        ),
    },
    {
      texto: "Exportar a pauta para o Word",
      desabilitado: perguntas === 0,
      dica: semPerguntas,
      aoEscolher: () =>
        exportar(
          "Pauta",
          () => apiPauta.escolherDestino(`pauta-${d.id}`, "word"),
          (destino) => apiPauta.exportar(projeto, d.id, "word", destino),
        ),
    },
    // Com o rastreador, o status vem de lá: não se encerra nem reabre aqui.
    ...(aberta && !d.rastreador
      ? [{ texto: "Encerrar a demanda", aoEscolher: mudarStatus, desabilitado: mudando }]
      : []),
  ];

  const principal: Acao | undefined = aberta
    ? { texto: ROTULO_FLUXO.impacto, aoEscolher: () => setFluxo("impacto") }
    : !d.rastreador
      ? { texto: "Reabrir a demanda", aoEscolher: mudarStatus, desabilitado: mudando }
      : undefined;

  const aguardandoRevisao = painel.linhaDoTempo.filter(
    (i) => i.tipo === "analise" && i.estado === "concluida",
  ).length;
  const contadores = [
    {
      n: perguntas,
      texto: perguntas === 1 ? "pergunta em aberto" : "perguntas em aberto",
      ancora: ancoraPauta,
    },
    {
      n: propostas,
      texto:
        propostas === 1 ? "proposta do agente para revisar" : "propostas do agente para revisar",
      ancora: ancoraRegistros,
      tom: "atencao",
    },
    {
      n: lacunas,
      texto: lacunas === 1 ? "lacuna na rastreabilidade" : "lacunas na rastreabilidade",
      ancora: ancoraRastreabilidade,
    },
    {
      n: aguardandoRevisao,
      texto:
        aguardandoRevisao === 1 ? "análise aguardando revisão" : "análises aguardando revisão",
      ancora: ancoraAnalises,
    },
  ].filter((c) => c.n > 0);

  const vazios = [
    painel.pendencias.length === 0 && "pendências",
    painel.duvidas.length === 0 && "dúvidas",
    painel.proximosPassos.length === 0 && "próximos passos",
  ].filter((v): v is string => !!v);

  return (
    <section className="painel-demanda page" aria-label="Demanda">
      <header className="painel-demanda-cabecalho">
        <div className="painel-demanda-identidade">
          <nav className="painel-demanda-trilha" aria-label="Trilha">
            <button
              type="button"
              className="link"
              onClick={() => navegar({ tipo: "demandas", projeto })}
            >
              Demandas
            </button>
            <span aria-hidden="true">›</span>
            <span className="mono" aria-current="page">
              {d.id}
            </span>
          </nav>
          <h1>{d.titulo}</h1>
          <Metadados
            d={d}
            abrirChave={(link) =>
              abrirLink(projeto, link).catch((e: unknown) => setFalha(mensagemDe(e)))
            }
          />
        </div>
        <BarraAcoes principal={principal} rotulo="Ações da demanda" alinhamento="fim">
          {aberta && (
            <MenuAcoes
              rotulo="Nova análise"
              itens={FLUXOS_DO_MENU.map((f) => ({
                texto: ROTULO_FLUXO[f],
                aoEscolher: () => setFluxo(f),
              }))}
            />
          )}
          <MenuAcoes rotulo="Mais ações" icone={<IconeReticencias />} itens={maisAcoes} />
        </BarraAcoes>
      </header>

      {!aberta && (
        <p className="muted">Reabra a demanda para criar uma análise ligada a ela.</p>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {aviso && <p role="status">{aviso}</p>}

      {aberta && fluxo && (
        <div ref={formulario}>
          <Secao rotulo="Nova análise da demanda">
            <FormularioDoFluxo
              fluxo={fluxo}
              api={api}
              apiReuniao={apiReuniao}
              projeto={projeto}
              demanda={d.id}
              aoCancelar={() => setFluxo(null)}
              aoCriar={abrirCriada}
            />
          </Secao>
        </div>
      )}

      {contadores.length > 0 && (
        <nav className="painel-demanda-resumo" aria-label="Resumo">
          {contadores.map((c) => (
            <button
              key={c.texto}
              type="button"
              className="painel-demanda-contador"
              data-tom={c.tom}
              onClick={() => irPara(c.ancora)}
            >
              <span className="painel-demanda-contador-numero">{c.n}</span>
              <span>{c.texto}</span>
            </button>
          ))}
        </nav>
      )}

      <div ref={ancoraAnalises} tabIndex={-1} className="painel-demanda-ancora">
        <Secao titulo="Linha do tempo">
          {painel.linhaDoTempo.length === 0 ? (
            <EstadoVazio>
              Nada ainda: as atas das reuniões, as análises e as sessões do agente ligadas à
              demanda aparecem aqui.
            </EstadoVazio>
          ) : (
            <ListaItens>
              {painel.linhaDoTempo.map((item) => (
                <ItemLista key={`${item.tipo}:${item.id}`}>
                  <div className="painel-demanda-evento" data-tipo={item.tipo}>
                    <span className="painel-demanda-data">{dataDoDia(item.data)}</span>
                    <EventoDaLinha
                      item={item}
                      abrirDocumento={abrirDocumento}
                      abrirAnalise={abrirAnalise}
                    />
                  </div>
                </ItemLista>
              ))}
            </ListaItens>
          )}
        </Secao>
      </div>

      <div ref={ancoraPauta} tabIndex={-1} className="painel-demanda-ancora">
        <PautaPerguntas
          projeto={projeto}
          demanda={d.id}
          api={apiPauta}
          atualizacao={painel}
          abrirAnalise={abrirAnalise}
          abrirDocumento={abrirDocumento}
        />
      </div>

      <div ref={ancoraRegistros} tabIndex={-1} className="painel-demanda-ancora">
        <RegistrosDemanda
          projeto={projeto}
          demanda={d.id}
          api={apiRegistros}
          atualizacao={painel}
          abrirDocumento={abrirDocumento}
        />
      </div>

      <div ref={ancoraRastreabilidade} tabIndex={-1} className="painel-demanda-ancora">
        <MatrizRastreabilidade
          projeto={projeto}
          demanda={d.id}
          navegar={navegar}
          api={apiRastreabilidade}
          atualizacao={painel}
          aoCarregar={aoCarregarMatriz}
          aoGerarHistorias={aberta ? () => setFluxo("historias") : undefined}
        />
      </div>

      <EntregaveisDemanda
        projeto={projeto}
        demanda={d.id}
        api={apiEntregaveis}
        atualizacao={painel}
        abrirLink={abrirLink}
      />

      {painel.pendencias.length > 0 && (
        <ItensDasAtas titulo="Pendências" itens={painel.pendencias} />
      )}
      {painel.duvidas.length > 0 && (
        <ItensDasAtas titulo="Dúvidas em aberto" itens={painel.duvidas} />
      )}
      {painel.proximosPassos.length > 0 && (
        <ItensDasAtas
          titulo="Próximos passos"
          itens={painel.proximosPassos}
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
      )}

      {painel.problemas.length > 0 && (
        <Alerta tom="erro" titulo="Documentos ilegíveis" role="note">
          <ul className="painel-demanda-problemas">
            {painel.problemas.map((p) => (
              <li key={p.caminho}>
                <code>{p.caminho}</code>: {p.erro}
              </li>
            ))}
          </ul>
        </Alerta>
      )}

      {vazios.length > 0 && (
        <p className="painel-demanda-fim">Sem {juntar(vazios)} nas atas aprovadas.</p>
      )}
    </section>
  );
}

/** Situação, tipo, chave no rastreador ↗, criação e conferência, numa linha só. */
function Metadados({ d, abrirChave }: { d: DemandaVista; abrirChave: (link: string) => void }) {
  const link = d.origem && /^https?:\/\//.test(d.origem) ? d.origem : null;
  return (
    <p className="painel-demanda-meta">
      <Selo tom={d.status === "aberta" ? "andamento" : "concluido"}>
        {ROTULO_STATUS_DEMANDA[d.status]}
      </Selo>
      {d.tipo && <Selo titulo="Tipo da demanda">{ROTULO_TIPO_DEMANDA[d.tipo]}</Selo>}
      {d.chave &&
        (link ? (
          <button
            type="button"
            className="link painel-demanda-meta-item"
            title="Abrir no rastreador"
            onClick={() => abrirChave(link)}
          >
            {d.chave} <span aria-hidden="true">↗</span>
          </button>
        ) : (
          <span className="painel-demanda-meta-item mono">{d.chave}</span>
        ))}
      <span className="painel-demanda-meta-item">criada em {dataDoDia(d.criado)}</span>
      {d.encerrado && (
        <span className="painel-demanda-meta-item">encerrada em {dataDoDia(d.encerrado)}</span>
      )}
      {d.rastreador && (
        <span className="painel-demanda-meta-item">
          conferida no rastreador em {quandoConferido(d.rastreador.conferido)}
        </span>
      )}
      {d.avisoRastreador && (
        <span className="painel-demanda-meta-item aviso-rastreador" role="status">
          {d.rastreador ? d.avisoRastreador : `Status não conferido: ${d.avisoRastreador}`}
        </span>
      )}
    </p>
  );
}

/** O formulário do tipo escolhido, já ligado à demanda. */
function FormularioDoFluxo({
  fluxo,
  api,
  apiReuniao,
  projeto,
  demanda,
  aoCancelar,
  aoCriar,
}: {
  fluxo: Fluxo;
  api: ApiConhecimento;
  apiReuniao?: ApiReuniao;
  projeto: string;
  demanda: string;
  aoCancelar: () => void;
  aoCriar: (projeto: string, criada: AnaliseCriada) => void;
}) {
  const projetos = [{ id: projeto, nome: projeto }];
  if (fluxo === "reuniao") {
    return (
      <NovaReuniao
        api={api}
        apiReuniao={apiReuniao}
        projetos={projetos}
        demandaFixa={demanda}
        aoCancelar={aoCancelar}
        aoCriar={aoCriar}
      />
    );
  }
  if (fluxo === "historias" || fluxo === "cenarios") {
    return (
      <NovaHistorias
        key={fluxo}
        fluxo={fluxo}
        api={api}
        projetos={projetos}
        demandaFixa={demanda}
        aoCancelar={aoCancelar}
        aoCriar={aoCriar}
      />
    );
  }
  if (fluxo === "prototipo") {
    return (
      <NovaPrototipo
        api={api}
        apiPrototipo={api.prototipo}
        projetos={projetos}
        demandaFixa={demanda}
        aoCancelar={aoCancelar}
        aoCriar={aoCriar}
      />
    );
  }
  return (
    <NovaAnalise
      key={fluxo}
      api={api}
      fluxo={fluxo}
      projetos={projetos}
      demandaFixa={demanda}
      aoCancelar={aoCancelar}
      aoCriar={aoCriar}
    />
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
    case "reuniao": {
      const nome = `Reunião ${item.id}${item.objetivo ? `: ${item.objetivo}` : ""}`;
      return (
        <>
          {item.documento ? (
            <button
              type="button"
              className="link painel-demanda-evento-link"
              title="Abrir a ata"
              onClick={() => abrirDocumento(item.documento!)}
            >
              {nome}
            </button>
          ) : (
            <span className="painel-demanda-evento-link">{nome}</span>
          )}
          <Selo tom={item.status === "aprovada" ? "concluido" : "atencao"}>
            {`ata ${ROTULO_STATUS_ATA[item.status]}`}
          </Selo>
          <span className="muted">
            {contar(item.participantes, "participante", "participantes")} ·{" "}
            {contar(item.itens, "item", "itens")}
          </span>
          {item.transcriptGuardado && <AvisoTranscriptGuardado guardado={item.transcriptGuardado} />}
        </>
      );
    }
    case "analise": {
      const s = SITUACAO_ANALISE[item.estado];
      return (
        <>
          <button
            type="button"
            className="link painel-demanda-evento-link"
            title={item.titulo}
            onClick={() => abrirAnalise(item.id)}
          >
            {item.id} · {ROTULO_FLUXO[item.fluxo]}
          </button>
          <Selo tom={s.tom}>{s.texto}</Selo>
        </>
      );
    }
    case "sessao": {
      const s = SITUACAO_SESSAO[item.estado];
      return (
        <>
          <button
            type="button"
            className="link painel-demanda-evento-link"
            onClick={() => abrirAnalise(item.analise)}
          >
            Sessão do agente em {item.analise}
          </button>
          <Selo tom={s.tom} titulo={ROTULO_SESSAO[item.estado]}>
            {s.texto}
          </Selo>
        </>
      );
    }
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

/** Pendências, dúvidas ou próximos passos das atas aprovadas (só com itens). */
function ItensDasAtas({
  titulo,
  itens,
  acoes,
}: {
  titulo: string;
  itens: ItemDaAta[];
  /** Ações de cada item (os botões dos próximos passos). */
  acoes?: (i: ItemDaAta) => ReactNode;
}) {
  return (
    <Secao titulo={titulo}>
      <ListaItens>
        {itens.map((i) => (
          <ItemLista
            key={`${i.reuniao}:${i.id}`}
            meta={`reunião ${i.reuniao} · ${dataDoDia(i.data)}`}
          >
            <span className="mono muted">{i.id}</span> <span>{i.texto}</span>
            {i.responsavel && <span className="muted"> · responsável: {i.responsavel}</span>}
            {i.prazo && <span className="muted"> · prazo: {dataDoDia(i.prazo)}</span>}
            {acoes?.(i)}
          </ItemLista>
        ))}
      </ListaItens>
    </Secao>
  );
}
