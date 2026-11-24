import { AjudaTermo } from "./AjudaTermo";
import {
  ambienteAPreparar,
  atividadeVisivel,
  grupoDaAtividade,
  gruposVisiveis,
  type AmbienteConferido,
  type DefinicaoGrupo,
  type GrupoAtividade,
} from "./atividades";
import { ROTULO_FLUXO } from "./conhecimento";
import { contar } from "./formatacao";
import type { Tela } from "./Navegacao";
import type { Atividade, ResumoProjeto } from "./resumoProjeto";

/** Uma linha de um grupo: identificador, título, detalhe e a ação direta. */
interface Item {
  chave: string;
  id?: string;
  titulo: string;
  detalhe: string;
  acao: string;
  principal?: boolean;
  aoAgir: () => void;
}

interface Props {
  projeto: string;
  nomeProjeto: string;
  /** Resumo do projeto (#318); `null` enquanto carrega ou quando a leitura falhou. */
  resumo: ResumoProjeto | null;
  falha: string | null;
  ambiente: AmbienteConferido;
  navegar: (t: Tela) => void;
}

/**
 * Atividades (#321, §7.1 da revisão de UX): a caixa de entrada do analista no projeto
 * escolhido, com tudo o que espera por ele, agrupado e na ordem de prioridade. Cada
 * item tem a ação direta; grupo vazio fica à vista com a frase que ensina o que
 * aparece ali. Os dados vêm do resumo do projeto, que a navegação relê a cada aviso
 * do diário de eventos, e do ambiente do Claude conferido para o projeto.
 */
export function TelaAtividades({ projeto, nomeProjeto, resumo, falha, ambiente, navegar }: Props) {
  const abrirAnalise = (id: string) =>
    navegar({ tipo: "analise", projeto, aberta: { id, abertura: { tipo: "reabrir" } } });

  const fluxoDe = (id: string | null) => {
    const r = resumo?.aguardandoRevisao.find((a) => a.id === id);
    return r ? ROTULO_FLUXO[r.fluxo] : null;
  };

  const item = (a: Atividade): Item | null => {
    const id = a.analise ?? a.demanda ?? undefined;
    const titulo = a.titulo ?? id ?? "";
    switch (a.tipo) {
      case "responder":
        return {
          chave: `responder:${a.analise}`,
          id,
          titulo,
          detalhe: "O agente fez uma pergunta ou precisa de uma decisão sua.",
          acao: "Responder ao agente",
          principal: true,
          aoAgir: () => a.analise && abrirAnalise(a.analise),
        };
      case "revisar": {
        const decididas = a.decididas ?? 0;
        const total = a.total ?? 0;
        const progresso =
          total > 0 ? `${decididas} de ${total} decididas` : "Nenhuma afirmação para decidir";
        return {
          chave: `revisar:${a.analise}`,
          id,
          titulo,
          detalhe: [fluxoDe(a.analise), progresso].filter(Boolean).join(" · "),
          acao: decididas > 0 ? "Continuar a revisão" : "Revisar",
          aoAgir: () => a.analise && abrirAnalise(a.analise),
        };
      }
      case "demanda-sem-analise":
        return {
          chave: `demanda:${a.demanda}`,
          id,
          titulo,
          detalhe: "Aberta, sem análise ligada.",
          acao: "Analisar impacto",
          aoAgir: () => a.demanda && navegar({ tipo: "demanda", projeto, demanda: a.demanda }),
        };
      case "revalidar": {
        const n = a.regras ?? 0;
        return {
          chave: "revalidar",
          titulo: `${contar(n, "regra")} a revalidar`,
          detalhe:
            n === 1
              ? "O código citado pela regra mudou desde a aprovação."
              : "O código citado por essas regras mudou desde a aprovação.",
          acao: "Ver as regras",
          // As regras a revalidar ficam na tela do projeto (a Visão geral, até a #320).
          aoAgir: () => navegar({ tipo: "projeto", projeto }),
        };
      }
      case "retomar":
        return {
          chave: `retomar:${a.analise}`,
          id,
          titulo,
          detalhe: "Criada ou em andamento, sem o agente trabalhando nela.",
          acao: "Retomar",
          aoAgir: () => a.analise && abrirAnalise(a.analise),
        };
      case "primeira-analise":
        return null;
    }
  };

  const porGrupo = new Map<GrupoAtividade, Item[]>();
  for (const a of resumo?.atividades.filter(atividadeVisivel) ?? []) {
    const g = grupoDaAtividade(a);
    const i = item(a);
    if (!g || !i) continue;
    porGrupo.set(g, [...(porGrupo.get(g) ?? []), i]);
  }
  if (ambienteAPreparar(ambiente)) {
    porGrupo.set("ambiente", [
      {
        chave: "ambiente",
        titulo: "Ligar o agente a este projeto",
        detalhe: "Sem o preparo, o agente não consegue registrar as afirmações da análise no documento.",
        acao: "Preparar",
        principal: true,
        // O preparo do ambiente fica em "Configurar projeto" (#319).
        aoAgir: () => navegar({ tipo: "configurar", projeto }),
      },
    ]);
  }

  /** A frase do grupo vazio; a do ambiente diz quando ele ainda não foi conferido. */
  const vazio = (g: DefinicaoGrupo) => {
    if (g.id === "ambiente" && ambiente === undefined) return "Conferindo o ambiente do Claude…";
    if (g.id === "ambiente" && ambiente === null)
      return "Não foi possível conferir o ambiente do Claude agora. Ele é conferido de novo ao voltar a esta tela.";
    return g.vazio;
  };

  const primeira = resumo?.atividades.some((a) => a.tipo === "primeira-analise") ?? false;

  return (
    <section className="page atividades" aria-label="Atividades">
      <div>
        <h2>Atividades</h2>
        <p className="muted">
          {projeto
            ? `O que espera por você em ${nomeProjeto}, do mais urgente ao menos.`
            : "Nenhum projeto cadastrado."}
        </p>
      </div>
      {falha && (
        <p className="error-text" role="alert">
          Não foi possível carregar as atividades: {falha}
        </p>
      )}
      {projeto && !resumo && !falha && <p className="muted">Carregando…</p>}
      {projeto && resumo && (
        <>
          {primeira && (
            <section className="panel atividades-grupo" aria-label="Primeira análise">
              <h2>Comece pela primeira análise</h2>
              <ul className="atividades-linhas">
                <Linha
                  item={{
                    chave: "primeira",
                    titulo: "Faça uma pergunta sobre o projeto",
                    detalhe: "O agente lê o código e registra o que achar; depois você revisa e aprova.",
                    acao: "Nova análise",
                    principal: true,
                    aoAgir: () => navegar({ tipo: "analise", projeto }),
                  }}
                />
              </ul>
            </section>
          )}
          {gruposVisiveis().map((g) => {
            const itens = porGrupo.get(g.id) ?? [];
            return (
              <section key={g.id} className="panel atividades-grupo" aria-label={g.titulo}>
                <h2>
                  {g.titulo}
                  <AjudaTermo termo={g.titulo.toLowerCase()}>{g.ajuda}</AjudaTermo>
                  {itens.length > 0 && (
                    <span className="badge" data-tone="attention">
                      {itens.length}
                    </span>
                  )}
                </h2>
                {itens.length > 0 ? (
                  <ul className="atividades-linhas">
                    {itens.map((i) => (
                      <Linha key={i.chave} item={i} />
                    ))}
                  </ul>
                ) : (
                  <p className="muted atividades-vazio">{vazio(g)}</p>
                )}
              </section>
            );
          })}
        </>
      )}
    </section>
  );
}

function Linha({ item }: { item: Item }) {
  return (
    <li>
      {item.id && <span className="atividades-id mono">{item.id}</span>}
      <span className="atividades-texto">
        <strong>{item.titulo}</strong>
        <span className="muted">{item.detalhe}</span>
      </span>
      <button
        type="button"
        className={item.principal ? "btn btn-primary" : "btn"}
        aria-label={item.id ? `${item.acao}: ${item.id}` : undefined}
        onClick={item.aoAgir}
      >
        {item.acao}
      </button>
    </li>
  );
}
