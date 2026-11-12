import { useEffect, useState } from "react";
import type { ApiAnalise, PromptPronto } from "./analise";

interface Props {
  api: ApiAnalise;
  sistema: string;
  analise: string;
  /** Se a sessão está viva para receber o prompt. */
  habilitado: boolean;
  aoEnviar: (texto: string) => void;
}

/**
 * Botões dos prompts prontos da análise (#32), enviados à sessão viva como se o
 * analista os tivesse digitado. Sem prompts (ou antes do comando existir), não mostra
 * nada.
 */
export function PromptsProntos({ api, sistema, analise, habilitado, aoEnviar }: Props) {
  const [prompts, setPrompts] = useState<PromptPronto[]>([]);

  useEffect(() => {
    let ativo = true;
    setPrompts([]);
    api
      .promptsProntos(sistema, analise)
      .then((lista) => ativo && setPrompts(lista))
      .catch(() => ativo && setPrompts([]));
    return () => {
      ativo = false;
    };
  }, [api, sistema, analise]);

  if (prompts.length === 0) return null;
  return (
    <div className="prompts-prontos" role="group" aria-label="Prompts prontos">
      {prompts.map((p) => (
        <button
          className="btn"
          key={p.id}
          type="button"
          disabled={!habilitado}
          title={p.descricao || p.texto}
          onClick={() => aoEnviar(p.texto)}
        >
          {p.rotulo}
        </button>
      ))}
    </div>
  );
}
