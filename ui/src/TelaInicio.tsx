import { AjudaTermo } from "./AjudaTermo";
import { useCallback, useEffect, useState } from "react";
import { contar } from "./formatacao";
import { mensagemDe, ROTULO_ESTADO_ANALISE, type AnaliseCriada, type Fluxo } from "./analise";
import type { ApiAssistenteProjeto } from "./AssistenteProjeto";
import { ClientesIA, type AbrirLink } from "./ClientesIA";
import { Alerta, BarraAcoes, EstadoVazio, ItemLista, ListaItens, Secao, Selo } from "./componentes";
import { apiConfiguracoesTauri } from "./configuracoes";
import {
  ROTULO_FLUXO,
  type AnaliseInicio,
  type ApiConhecimento,
  type Inicio,
} from "./conhecimento";
import { ROTULO_SESSAO } from "./DocumentoAoVivo";
import type { Tela } from "./Navegacao";
import { NovaAnalise } from "./NovaAnalise";
import { NovaHistorias } from "./NovaHistorias";
import { ModalNovoProjeto } from "./ModalNovoProjeto";
import { NovaReuniao } from "./NovaReuniao";
import {
  aguardaRevisao,
  emAndamento,
  proximoPasso,
  type AmbientesConferidos,
  type PassoInicio,
} from "./proximoPassoInicio";
import type { ApiReuniao } from "./reunioes";

const FLUXOS: Fluxo[] = [
  "perguntar",
  "explicar",
  "impacto",
  "reuniao",
  "revisar",
  "historias",
  "cenarios",
];

const ESTADO = ROTULO_ESTADO_ANALISE;

interface Props {
  api: ApiConhecimento;
  navegar: (t: Tela) => void;
  abrirLink?: AbrirLink;
  aoPrimeiroUso?: () => void;
  /** Nova reunião (#141); para testes. */
  apiReuniao?: ApiReuniao;
  /** Assistente de cadastro do botão “Novo projeto”; para testes. */
  apiProjeto?: ApiAssistenteProjeto;
  /** Um projeto foi cadastrado (o painel lateral recarrega a lista). */
  aoMudarProjetos?: () => void;
  /**
   * Abaixo das Atividades (#321): sem o “Próximo passo” e sem as listas “Em
   * andamento” e “Aguardando revisão”, que os grupos das Atividades já mostram.
   */
  semConducao?: boolean;
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
  apiProjeto = apiConfiguracoesTauri,
  aoMudarProjetos,
  semConducao = false,
}: Props) {
  const [inicio, setInicio] = useState<Inicio | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [fluxo, setFluxo] = useState<Fluxo | null>(null);
  const [novoProjeto, setNovoProjeto] = useState(false);
  const [ambientes, setAmbientes] = useState<AmbientesConferidos>({});

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

  // O preparo do ambiente de cada projeto alimenta o “próximo passo”.
  useEffect(() => {
    if (!inicio || semConducao) return;
    let ativo = true;
    for (const p of inicio.projetos) {
      if (p.erro) continue;
      api
        .verificarAmbiente(p.id)
        .then((a) => ativo && setAmbientes((m) => ({ ...m, [p.id]: a })))
        .catch(() => ativo && setAmbientes((m) => ({ ...m, [p.id]: null })));
    }
    return () => {
      ativo = false;
    };
  }, [api, inicio, semConducao]);

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

  const projetoCadastrado = () => {
    setNovoProjeto(false);
    carregar();
    aoMudarProjetos?.();
  };

  const passo = inicio && !semConducao ? proximoPasso(inicio, ambientes) : null;
  const executarPasso = (p: PassoInicio) => {
    switch (p.tipo) {
      case "responder":
      case "revisar":
      case "retomar":
        abrirAnalise(p.analise);
        break;
      case "revalidar":
        navegar({ tipo: "projeto", projeto: p.projeto });
        break;
      case "preparar":
        // O preparo do ambiente fica em "Configurar projeto" (#319).
        navegar({ tipo: "configurar", projeto: p.projeto });
        break;
      case "primeira-analise":
        setFluxo("perguntar");
        break;
    }
  };

  const semProjeto = inicio !== null && (inicio.base === null || inicio.projetos.length === 0);

  return (
    <section className="tela-inicio page" aria-label="Início">
      {falha && (
        <Alerta tom="erro" titulo="Não foi possível carregar o início.">
          {falha}
        </Alerta>
      )}
      {semProjeto && (
        <EstadoVazio acao={aoPrimeiroUso && { texto: "Abrir o primeiro uso", aoEscolher: aoPrimeiroUso }}>
          {inicio.base === null
            ? "Falta escolher a pasta da base de conhecimento."
            : "Nenhum projeto cadastrado."}
        </EstadoVazio>
      )}

      {passo && <ProximoPassoInicio passo={passo} aoExecutar={() => executarPasso(passo)} />}

      {!semConducao && (
      <Secao titulo="Nova análise" rotulo="Nova análise">
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
        {(fluxo === "historias" || fluxo === "cenarios") && inicio && !semProjeto && (
          <NovaHistorias
            key={fluxo}
            fluxo={fluxo}
            api={api}
            projetos={inicio.projetos}
            aoCancelar={() => setFluxo(null)}
            aoCriar={abrirCriada}
          />
        )}
        {fluxo &&
          fluxo !== "reuniao" &&
          fluxo !== "historias" &&
          fluxo !== "cenarios" && inicio && !semProjeto && (
          <NovaAnalise
            key={fluxo}
            api={api}
            fluxo={fluxo}
            projetos={inicio.projetos}
            aoCancelar={() => setFluxo(null)}
            aoCriar={abrirCriada}
          />
        )}
      </Secao>
      )}

      {inicio && (
        <>
          {!semConducao && (
            <ListaAnalises
              titulo="Em andamento"
              vazio="Nenhuma análise em andamento."
              analises={inicio.emAndamento.filter(emAndamento)}
              aoAbrir={abrirAnalise}
            />
          )}
          {!semConducao && inicio.emAndamento.some(aguardaRevisao) && (
            <ListaAnalises
              titulo="Aguardando revisão"
              vazio=""
              analises={inicio.emAndamento.filter(aguardaRevisao)}
              aoAbrir={abrirAnalise}
            />
          )}
          {!semConducao && (
            <ListaAnalises
              titulo="Recentes"
              vazio="Nenhuma análise ainda."
              analises={inicio.recentes}
              aoAbrir={abrirAnalise}
            />
          )}
          <Secao
            titulo="Projetos"
            rotulo="Projetos"
            acoes={
              inicio.base !== null && (
                <button className="btn" type="button" onClick={() => setNovoProjeto(true)}>
                  + Novo projeto
                </button>
              )
            }
          >
            {inicio.projetos.length === 0 ? (
              <EstadoVazio>Nenhum projeto cadastrado.</EstadoVazio>
            ) : (
              <ListaItens>
                {inicio.projetos.map((s) => (
                  <ItemLista
                    key={s.id}
                    meta={s.aRevalidar > 0 && <Selo tom="atencao">{`${s.aRevalidar} a revalidar`}</Selo>}
                  >
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
                        {contar(s.analises, "análise", "análises")} · {contar(s.regras, "regra", "regras")} · {contar(s.termos, "termo", "termos")}
                      </span>
                    )}
                  </ItemLista>
                ))}
              </ListaItens>
            )}
          </Secao>
        </>
      )}

      <ClientesIA abrirLink={abrirLink} />

      {novoProjeto && (
        <ModalNovoProjeto
          api={apiProjeto}
          aoFechar={() => setNovoProjeto(false)}
          aoConcluir={projetoCadastrado}
          aoPrepararAmbiente={(p) => {
            // O preparo do ambiente do Claude fica em "Configurar projeto" (#319).
            projetoCadastrado();
            navegar({ tipo: "configurar", projeto: p.id });
          }}
        />
      )}
    </section>
  );
}

/** O texto e o botão de cada próximo passo sugerido. */
function descreverPasso(p: PassoInicio): { titulo: string; texto: string; acao: string } {
  switch (p.tipo) {
    case "responder":
      return {
        titulo: `O agente aguarda você na ${p.analise.id}`,
        texto: `${p.analise.titulo} (${p.analise.projetoNome}): o agente fez uma pergunta ou precisa de uma decisão sua.`,
        acao: "Responder ao agente",
      };
    case "revisar":
      return {
        titulo: `Revisar e aprovar a ${p.analise.id}`,
        texto: `${p.analise.titulo} (${p.analise.projetoNome}) foi concluída pelo agente. Confira as afirmações e aprove para elas virarem regras da base.`,
        acao: "Revisar a análise",
      };
    case "revalidar":
      return {
        titulo: `Revalidar as regras de ${p.nome}`,
        texto:
          p.regras === 1
            ? "1 regra tem o código citado alterado desde a aprovação."
            : `${p.regras} regras têm o código citado alterado desde a aprovação.`,
        acao: "Ver as regras",
      };
    case "preparar":
      return {
        titulo: `Ligar o agente ao projeto ${p.nome}`,
        texto:
          "Sem o preparo, o agente não consegue registrar as afirmações da análise no documento.",
        acao: "Preparar",
      };
    case "retomar":
      return {
        titulo: `Retomar a ${p.analise.id}`,
        texto: `${p.analise.titulo} (${p.analise.projetoNome}) está em andamento.`,
        acao: "Abrir a análise",
      };
    case "primeira-analise":
      return {
        titulo: `Fazer a primeira análise de ${p.nome}`,
        texto: "Comece com uma pergunta sobre o projeto; o agente lê o código e registra o que achar.",
        acao: "Fazer uma pergunta",
      };
  }
}

function ProximoPassoInicio({ passo, aoExecutar }: { passo: PassoInicio; aoExecutar: () => void }) {
  const { titulo, texto, acao } = descreverPasso(passo);
  return (
    <Secao titulo="Próximo passo" rotulo="Próximo passo" className="proximo-passo-inicio">
      <p>
        <strong>{titulo}</strong>
      </p>
      <p className="muted">{texto}</p>
      <BarraAcoes rotulo="Ação do próximo passo" principal={{ texto: acao, aoEscolher: aoExecutar }} />
    </Secao>
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
    <Secao
      rotulo={titulo}
      titulo={
        <>
          {titulo}
          {titulo === "Aguardando revisão" && (
            <AjudaTermo termo="aguardando revisão">
              O agente terminou a análise e falta você conferir e aprovar.
            </AjudaTermo>
          )}
        </>
      }
    >
      {analises.length === 0 && vazio && <EstadoVazio>{vazio}</EstadoVazio>}
      {analises.length > 0 && (
        <ListaItens>
          {analises.map((a) => (
            <ItemLista key={`${a.projeto}/${a.id}`} meta={<Selo>{ESTADO[a.estado]}</Selo>}>
              <button className="link" type="button" onClick={() => aoAbrir(a)}>
                {a.id}: {a.titulo}
              </button>{" "}
              <span className="muted">
                {a.projetoNome} · {ROTULO_FLUXO[a.fluxo].toLowerCase()}
                {a.sessao && a.sessao.estado !== "encerrada" && ` · ${ROTULO_SESSAO[a.sessao.estado]}`}
              </span>
            </ItemLista>
          ))}
        </ListaItens>
      )}
    </Secao>
  );
}
