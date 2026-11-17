import type { Pendencia } from "./revisao";
import {
  contarPendencias,
  FILTROS,
  type FiltroRevisao,
  type ResumoRevisao,
} from "./resumoRevisao";

interface Props {
  resumo: ResumoRevisao;
  contagem: Record<FiltroRevisao, number>;
  filtro: FiltroRevisao;
  aoFiltrar: (f: FiltroRevisao) => void;
  /** Pendências da aprovação: o número é o mesmo do cabeçalho ({@link contarPendencias}). */
  pendencias?: readonly Pendencia[];
}

/** "26 pendentes", com o que entra na conta além das afirmações (termos, por exemplo). */
function TextoPendentes({ pendencias }: { pendencias: readonly Pendencia[] }) {
  const n = contarPendencias(pendencias);
  const termos = pendencias.filter((p) => p.tipo === "termo").length;
  const detalhe =
    termos > 0 ? ` (inclui ${termos} ${termos === 1 ? "termo" : "termos"} do glossário)` : "";
  return (
    <span className="faixa-revisao-pendentes">
      {" · "}
      <strong>{n}</strong> {n === 1 ? "pendente" : "pendentes"}
      {detalhe && <span className="muted">{detalhe}</span>}
    </span>
  );
}

/** Atalhos de teclado da revisão (dica visível na faixa). */
export const ATALHOS: { tecla: string; acao: string }[] = [
  { tecla: "J", acao: "próxima" },
  { tecla: "K", acao: "anterior" },
  { tecla: "C", acao: "confirmar" },
  { tecla: "X", acao: "incorreta" },
  { tecla: "A", acao: "aprofundar" },
];

const SEGMENTOS = [
  { chave: "confirmadas", rotulo: "confirmadas" },
  { chave: "incorretas", rotulo: "incorretas" },
  { chave: "aprofundar", rotulo: "a aprofundar" },
] as const;

/**
 * Faixa de revisão (#184): quantas afirmações já têm decisão, a barra de progresso
 * dividida por resultado, os filtros e a dica dos atalhos.
 */
export function FaixaRevisao({ resumo, contagem, filtro, aoFiltrar, pendencias }: Props) {
  const { total, decididas } = resumo;
  // As a aprofundar já têm o próprio segmento na barra.
  const semDecisao = resumo.pendentes - resumo.aprofundar;
  return (
    <section className="faixa-revisao" aria-label="Progresso da revisão">
      <div className="faixa-revisao-linha">
        <p className="faixa-revisao-contagem" role="status">
          <strong>
            {decididas} de {total}
          </strong>{" "}
          {total === 1 ? "decidido" : "decididos"}
          {pendencias && <TextoPendentes pendencias={pendencias} />}
        </p>
        <div
          className="faixa-revisao-barra"
          role="progressbar"
          aria-label="Afirmações decididas"
          aria-valuemin={0}
          aria-valuemax={total}
          aria-valuenow={decididas}
          aria-valuetext={`${decididas} de ${total} decididas: ${resumo.confirmadas} confirmadas, ${resumo.incorretas} incorretas; ${resumo.aprofundar} a aprofundar`}
        >
          {SEGMENTOS.map(({ chave, rotulo }) =>
            resumo[chave] > 0 ? (
              <span
                key={chave}
                className="faixa-revisao-segmento"
                data-segmento={chave}
                title={`${resumo[chave]} ${rotulo}`}
                style={{ flexGrow: resumo[chave] }}
              />
            ) : null,
          )}
          {semDecisao > 0 && (
            <span
              className="faixa-revisao-segmento"
              data-segmento="pendentes"
              title={`${semDecisao} sem decisão`}
              style={{ flexGrow: semDecisao }}
            />
          )}
        </div>
      </div>
      <div className="faixa-revisao-linha">
        <div className="faixa-revisao-filtros" role="group" aria-label="Filtrar afirmações">
          {FILTROS.map(({ id, rotulo }) => (
            <button
              key={id}
              type="button"
              className="btn btn-sm filtro-revisao"
              aria-pressed={filtro === id}
              onClick={() => aoFiltrar(id)}
            >
              {rotulo} <span className="filtro-revisao-n">{contagem[id]}</span>
            </button>
          ))}
        </div>
        <p className="faixa-revisao-atalhos muted" aria-label="Atalhos de teclado">
          {ATALHOS.map(({ tecla, acao }) => (
            <span key={tecla} className="atalho">
              <kbd>{tecla}</kbd> {acao}
            </span>
          ))}
        </p>
      </div>
    </section>
  );
}
