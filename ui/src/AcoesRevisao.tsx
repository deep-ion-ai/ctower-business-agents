import { useId, useState } from "react";
import type { Afirmacao, Pergunta } from "./analise";
import type { DecisaoRevisao } from "./revisao";

/** Como o documento leva as decisões do analista à revisão (#37). */
export interface Revisao {
  /** Aplica a decisão; a promessa rejeita com a mensagem da recusa. */
  aoDecidir: (d: DecisaoRevisao) => Promise<void>;
  /** Uma decisão está em curso: os botões ficam desabilitados. */
  ocupado: boolean;
}

type Formulario = { tipo: "incorreta" } | { tipo: "editar" } | null;

/**
 * Ações do analista numa afirmação: confirmar (vira regra na aprovação), marcar
 * como incorreta (com comentário; fica registrada e não vira regra), pedir
 * aprofundamento, desfazer a decisão e editar o texto (as evidências ficam).
 */
export function AcoesAfirmacao({ afirmacao, revisao }: { afirmacao: Afirmacao; revisao: Revisao }) {
  const [form, setForm] = useState<Formulario>(null);
  const [texto, setTexto] = useState("");
  const id = useId();
  const { numero, estado } = afirmacao;
  const decidida = estado === "confirmada" || estado === "incorreta" || estado === "aprofundar";

  const decidir = (d: DecisaoRevisao) =>
    revisao
      .aoDecidir(d)
      .then(() => setForm(null))
      .catch(() => undefined);

  const abrir = (tipo: "incorreta" | "editar") => {
    if (tipo === "editar") setTexto(afirmacao.texto);
    else setTexto(afirmacao.estado === "incorreta" ? afirmacao.comentario : "");
    setForm({ tipo });
  };

  return (
    <div className="acoes-revisao" role="group" aria-label={`Revisão da afirmação ${numero}`}>
      {!form && (
        <>
          <button
            type="button"
            disabled={revisao.ocupado || estado === "confirmada"}
            onClick={() => decidir({ tipo: "afirmacao", numero, estado: "confirmada" })}
          >
            Confirmar
          </button>
          <button type="button" disabled={revisao.ocupado} onClick={() => abrir("incorreta")}>
            Marcar incorreta
          </button>
          <button
            type="button"
            disabled={revisao.ocupado || estado === "aprofundar"}
            onClick={() => decidir({ tipo: "afirmacao", numero, estado: "aprofundar" })}
          >
            Aprofundar
          </button>
          <button type="button" disabled={revisao.ocupado} onClick={() => abrir("editar")}>
            Editar texto
          </button>
          {decidida && (
            <button
              type="button"
              disabled={revisao.ocupado}
              onClick={() => decidir({ tipo: "afirmacao", numero, estado: "registrada" })}
            >
              Desfazer decisão
            </button>
          )}
        </>
      )}
      {form && (
        <form
          className="form-revisao"
          onSubmit={(e) => {
            e.preventDefault();
            const valor = texto.trim();
            if (!valor) return;
            decidir(
              form.tipo === "incorreta"
                ? { tipo: "afirmacao", numero, estado: "incorreta", comentario: valor }
                : { tipo: "editar-afirmacao", numero, texto: valor },
            );
          }}
        >
          <label htmlFor={id}>
            {form.tipo === "incorreta"
              ? "Por que está incorreta? O comentário fica registrado na análise."
              : "Texto da afirmação (as evidências ficam como estão)"}
          </label>
          <textarea id={id} value={texto} rows={3} onChange={(e) => setTexto(e.target.value)} />
          <div>
            <button type="submit" disabled={revisao.ocupado || !texto.trim()}>
              {form.tipo === "incorreta" ? "Marcar como incorreta" : "Salvar texto"}
            </button>{" "}
            <button type="button" onClick={() => setForm(null)}>
              Cancelar
            </button>
          </div>
        </form>
      )}
    </div>
  );
}

/** Responder ou descartar uma pergunta aberta. */
export function AcoesPergunta({ pergunta, revisao }: { pergunta: Pergunta; revisao: Revisao }) {
  const [respondendo, setRespondendo] = useState(false);
  const [resposta, setResposta] = useState("");
  const id = useId();
  const { numero } = pergunta;
  if (pergunta.estado !== "aberta") return null;
  const decidir = (d: DecisaoRevisao) =>
    revisao
      .aoDecidir(d)
      .then(() => setRespondendo(false))
      .catch(() => undefined);
  if (respondendo)
    return (
      <form
        className="form-revisao"
        onSubmit={(e) => {
          e.preventDefault();
          if (resposta.trim())
            decidir({ tipo: "pergunta", numero, estado: "respondida", resposta: resposta.trim() });
        }}
      >
        <label htmlFor={id}>Resposta</label>
        <textarea id={id} value={resposta} rows={2} onChange={(e) => setResposta(e.target.value)} />
        <div>
          <button type="submit" disabled={revisao.ocupado || !resposta.trim()}>
            Salvar resposta
          </button>{" "}
          <button type="button" onClick={() => setRespondendo(false)}>
            Cancelar
          </button>
        </div>
      </form>
    );
  return (
    <div className="acoes-revisao" role="group" aria-label={`Revisão da pergunta ${numero}`}>
      <button type="button" disabled={revisao.ocupado} onClick={() => setRespondendo(true)}>
        Responder
      </button>
      <button
        type="button"
        disabled={revisao.ocupado}
        onClick={() => decidir({ tipo: "pergunta", numero, estado: "descartada" })}
      >
        Descartar
      </button>
    </div>
  );
}

/** Aceitar ou recusar uma sugestão ou um termo proposto. */
export function AcoesAceite({
  tipo,
  numero,
  rotulo,
  revisao,
}: {
  tipo: "sugestao" | "termo";
  numero: number;
  rotulo: string;
  revisao: Revisao;
}) {
  const decidir = (aceitar: boolean) => {
    const d: DecisaoRevisao =
      tipo === "sugestao"
        ? { tipo, numero, estado: aceitar ? "aceita" : "recusada" }
        : { tipo, numero, estado: aceitar ? "aceito" : "recusado" };
    revisao.aoDecidir(d).catch(() => undefined);
  };
  return (
    <div className="acoes-revisao" role="group" aria-label={`Revisão ${rotulo}`}>
      <button type="button" disabled={revisao.ocupado} onClick={() => decidir(true)}>
        Aceitar
      </button>
      <button type="button" disabled={revisao.ocupado} onClick={() => decidir(false)}>
        Recusar
      </button>
    </div>
  );
}
