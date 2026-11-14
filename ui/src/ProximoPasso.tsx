import { useId, useState } from "react";
import { mensagemDe, type AnaliseCriada } from "./analise";
import type { AcaoProximoPasso, ApiReuniao } from "./reunioes";

/** O que o botão precisa saber do próximo passo (da ata ou do painel da demanda). */
export interface PassoDaAta {
  /** `PP-1`. */
  id: string;
  texto: string;
  /** Itens da mesma ata a que o passo se refere (`N-3`). */
  referencias: string[];
}

/** Necessidades (`N-3`) entre as referências do passo. */
export function necessidadesDe(passo: PassoDaAta): string[] {
  return passo.referencias.filter((r) => /^N-\d+$/.test(r));
}

/**
 * Botões de um próximo passo da ata aprovada (#142): "Analisar impacto da
 * necessidade N-3" (um por necessidade citada; sem nenhuma, "Analisar impacto
 * deste passo") e "Perguntar ao sistema…", com a pergunta para o analista ajustar.
 * Cada um abre uma análise já ligada à demanda da reunião, com a ata como contexto.
 */
export function BotoesProximoPasso({
  api,
  projeto,
  reuniao,
  passo,
  aoCriar,
}: {
  api: ApiReuniao;
  projeto: string;
  reuniao: string;
  passo: PassoDaAta;
  aoCriar: (criada: AnaliseCriada) => void;
}) {
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [pergunta, setPergunta] = useState<string | null>(null);
  const id = useId();
  const necessidades = necessidadesDe(passo);

  const abrir = (acao: AcaoProximoPasso) => {
    setOcupado(true);
    setFalha(null);
    api
      .analisarProximoPasso(projeto, reuniao, passo.id, acao)
      .then(aoCriar)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  return (
    <div className="acoes-proximo-passo" role="group" aria-label={`Próximo passo ${passo.id}`}>
      {necessidades.map((n) => (
        <button
          key={n}
          type="button"
          className="btn"
          disabled={ocupado}
          onClick={() => abrir({ tipo: "impacto", necessidade: n })}
        >
          Analisar impacto da necessidade {n}
        </button>
      ))}
      {necessidades.length === 0 && (
        <button
          type="button"
          className="btn"
          disabled={ocupado}
          onClick={() => abrir({ tipo: "impacto", necessidade: null })}
        >
          Analisar impacto deste passo
        </button>
      )}{" "}
      {pergunta === null ? (
        <button
          type="button"
          className="btn"
          disabled={ocupado}
          onClick={() => setPergunta(passo.texto)}
        >
          Perguntar ao sistema…
        </button>
      ) : (
        <form
          className="form-revisao"
          onSubmit={(e) => {
            e.preventDefault();
            if (pergunta.trim()) abrir({ tipo: "perguntar", pergunta: pergunta.trim() });
          }}
        >
          <label htmlFor={id}>Pergunta ao sistema ({passo.id})</label>
          <textarea
            id={id}
            rows={2}
            value={pergunta}
            onChange={(e) => setPergunta(e.target.value)}
          />
          <div>
            <button className="btn" type="submit" disabled={ocupado || !pergunta.trim()}>
              Perguntar
            </button>{" "}
            <button className="btn" type="button" onClick={() => setPergunta(null)}>
              Cancelar
            </button>
          </div>
        </form>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </div>
  );
}
