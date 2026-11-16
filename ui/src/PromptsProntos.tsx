import { useCallback, useEffect, useState } from "react";
import type { ApiAnalise, PromptPronto } from "./analise";
import { BotaoPainel } from "./BotaoPainel";

interface Props {
  api: ApiAnalise;
  projeto: string;
  analise: string;
  /** Se a sessão está viva para receber o prompt. */
  habilitado: boolean;
  aoEnviar: (texto: string) => void;
  /** Ids dos prompts que já têm botão próprio no painel (ações rápidas, #185). */
  excluir?: string[];
}

/**
 * Os demais prompts prontos da análise (#32), enviados à sessão viva como se o
 * analista os tivesse digitado. Ficam no botão "Mais pedidos", ao lado das ações
 * rápidas do painel (#130, #185). Sem prompts (ou antes do comando existir), não
 * mostra nada.
 */
export function PromptsProntos({ api, projeto, analise, habilitado, aoEnviar, excluir = [] }: Props) {
  const [todos, setPrompts] = useState<PromptPronto[]>([]);
  const prompts = todos.filter((p) => !excluir.includes(p.id));
  const [aberto, setAberto] = useState(false);
  const fechar = useCallback(() => setAberto(false), []);

  useEffect(() => {
    let ativo = true;
    setPrompts([]);
    api
      .promptsProntos(projeto, analise)
      .then((lista) => ativo && setPrompts(lista))
      .catch(() => ativo && setPrompts([]));
    return () => {
      ativo = false;
    };
  }, [api, projeto, analise]);

  if (prompts.length === 0) return null;
  return (
    <BotaoPainel
      aberto={aberto}
      aoAlternar={() => setAberto((v) => !v)}
      aoFechar={fechar}
      rotulo="Mais pedidos"
      conteudo="Mais pedidos ▾"
      classeBotao="btn btn-sm"
      rotuloPainel="Lista de pedidos prontos"
      classePainel="painel-prompts"
    >
      <div className="prompts-prontos" role="group" aria-label="Mais pedidos">
        {!habilitado && (
          <p className="muted">Os prompts vão para a conversa aberta aqui, no terminal do app.</p>
        )}
        {prompts.map((p) => (
          <button
            className="btn"
            key={p.id}
            type="button"
            disabled={!habilitado}
            title={p.descricao || p.texto}
            onClick={() => {
              setAberto(false);
              aoEnviar(p.texto);
            }}
          >
            {p.rotulo}
          </button>
        ))}
      </div>
    </BotaoPainel>
  );
}
