import {
  FILTROS,
  type FiltroRevisao,
  type ResumoRevisao,
} from "./resumoRevisao";

interface Props {
  resumo: ResumoRevisao;
  contagem: Record<FiltroRevisao, number>;
  filtro: FiltroRevisao;
  aoFiltrar: (f: FiltroRevisao) => void;
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
export function FaixaRevisao({ resumo, contagem, filtro, aoFiltrar }: Props) {
  const { total, decididas } = resumo;
  return (
    <section className="faixa-revisao" aria-label="Progresso da revisão">
      <div className="faixa-revisao-linha">
        <p className="faixa-revisao-contagem" role="status">
          <strong>
            {decididas} de {total}
          </strong>{" "}
          {total === 1 ? "decidido" : "decididos"}
        </p>
        <div
          className="faixa-revisao-barra"
          role="progressbar"
          aria-label="Afirmações decididas"
          aria-valuemin={0}
          aria-valuemax={total}
          aria-valuenow={decididas}
          aria-valuetext={`${decididas} de ${total} decididas: ${resumo.confirmadas} confirmadas, ${resumo.incorretas} incorretas, ${resumo.aprofundar} a aprofundar`}
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
          {resumo.pendentes > 0 && (
            <span
              className="faixa-revisao-segmento"
              data-segmento="pendentes"
              title={`${resumo.pendentes} pendentes`}
              style={{ flexGrow: resumo.pendentes }}
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
