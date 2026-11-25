import { useEffect, useState } from "react";
import type { Conducao, Fluxo } from "./analise";
import { textoDaEstimativa, textoDaFase, type ApiConducao, type ConducaoDisponivel } from "./conducao";

interface Props {
  api: ApiConducao;
  projeto: string;
  fluxo: Fluxo;
  valor: Conducao;
  aoMudar: (c: Conducao) => void;
}

/**
 * "Condução: Simples | Aprofundada (workflow)" em Nova análise (#371), nos tipos com
 * workflow. A Aprofundada fica desabilitada, com o motivo, sem a versão mínima do
 * Claude Code, sem os workflows instalados pelo preparo ou com o Desktop como cliente
 * preferido; ao lado, o número máximo de agentes e, depois da medição, a faixa de
 * tokens.
 */
export function EscolhaConducao({ api, projeto, fluxo, valor, aoMudar }: Props) {
  const [disponivel, setDisponivel] = useState<ConducaoDisponivel | null>(null);

  useEffect(() => {
    if (!projeto) return;
    let ativo = true;
    setDisponivel(null);
    api
      .disponivel(projeto, fluxo)
      .then((d) => ativo && setDisponivel(d))
      // Sem a verificação, a escolha some e vale a condução simples.
      .catch(() => ativo && setDisponivel(null));
    return () => {
      ativo = false;
    };
  }, [api, projeto, fluxo]);

  // A escolha guardada pode não valer mais (versão, preparo): volta à simples.
  useEffect(() => {
    if (disponivel && !disponivel.disponivel && valor === "workflow") aoMudar("simples");
  }, [disponivel, valor, aoMudar]);

  if (!disponivel?.temWorkflow) return null;
  return (
    <fieldset className="escolha-conducao">
      <legend>Condução</legend>
      <label>
        <input
          type="radio"
          name="conducao"
          value="simples"
          checked={valor === "simples"}
          onChange={() => aoMudar("simples")}
        />{" "}
        Simples <span className="muted">(uma conversa segue o roteiro)</span>
      </label>
      <label>
        <input
          type="radio"
          name="conducao"
          value="workflow"
          checked={valor === "workflow"}
          disabled={!disponivel.disponivel}
          aria-describedby="conducao-detalhe"
          onChange={() => aoMudar("workflow")}
        />{" "}
        Aprofundada <span className="muted">(workflow: agentes em paralelo com verificação independente)</span>
      </label>
      <p id="conducao-detalhe" className="ajuda-campo">
        {disponivel.disponivel
          ? `Aprofundada: ${textoDaEstimativa(disponivel)}.`
          : `Aprofundada indisponível: ${disponivel.motivo ?? "motivo desconhecido"}.`}
      </p>
      {disponivel.disponivel && (
        <p className="alerta-conducao" role="note">
          Somente justifica o uso em análises complexas ou de funcionalidades críticas.
        </p>
      )}
      {disponivel.fases.length > 0 && (
        <details className="conducao-fases">
          <summary>O que a Aprofundada faz, fase a fase</summary>
          <ol aria-label="Fases do workflow">
            {disponivel.fases.map((f) => (
              <li key={f.titulo}>
                <strong>{f.titulo}</strong>
                {f.detalhe && <>: {f.detalhe}</>}
                <span className="muted"> ({textoDaFase(f, disponivel.frentes)})</span>
              </li>
            ))}
          </ol>
          <p className="ajuda-campo">
            A simples faz o mesmo numa conversa só, sem a verificação independente.
          </p>
        </details>
      )}
    </fieldset>
  );
}
