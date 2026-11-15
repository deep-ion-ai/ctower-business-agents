import { useEffect, useRef, type KeyboardEvent } from "react";
import {
  AssistenteProjeto,
  type ApiAssistenteProjeto,
} from "./AssistenteProjeto";
import type { ProjetoCadastrado } from "./primeiroUso";
import "./assistenteProjeto.css";

const FOCAVEIS =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * O assistente de cadastro de projeto (#180) numa modal, aberto pelo botão “Novo
 * projeto” do Início e de Configurações › Projetos. Acessível: `role="dialog"`, foco
 * preso (Tab e Shift+Tab circulam dentro), `Esc` e “×” fecham e o foco volta ao
 * elemento que abriu a modal. Enquanto clona ou cadastra, o próprio assistente
 * desabilita o “Cancelar”; fechar pela modal nesse meio não desfaz o que já foi feito
 * (os clones ficam na pasta), só descarta o assistente.
 */
export function ModalNovoProjeto({
  api,
  aoFechar,
  aoConcluir,
  aoPrepararAmbiente,
}: {
  api: ApiAssistenteProjeto;
  /** Fechar sem concluir (Esc, “×” ou “Cancelar”). */
  aoFechar: () => void;
  /** “Fazer depois” no fim do assistente. */
  aoConcluir: (projeto: ProjetoCadastrado) => void;
  /** “Preparar o ambiente” no fim; sem ele, vale {@link aoConcluir}. */
  aoPrepararAmbiente?: (projeto: ProjetoCadastrado) => void;
}) {
  const caixa = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const anterior =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    caixa.current?.focus();
    return () => anterior?.focus();
  }, []);

  const teclas = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      aoFechar();
      return;
    }
    if (e.key !== "Tab" || !caixa.current) return;
    const itens = Array.from(
      caixa.current.querySelectorAll<HTMLElement>(FOCAVEIS),
    );
    if (itens.length === 0) {
      e.preventDefault();
      return;
    }
    const primeiro = itens[0];
    const ultimo = itens[itens.length - 1];
    const ativo = document.activeElement;
    if (e.shiftKey && (ativo === primeiro || ativo === caixa.current)) {
      e.preventDefault();
      ultimo.focus();
    } else if (!e.shiftKey && ativo === ultimo) {
      e.preventDefault();
      primeiro.focus();
    }
  };

  return (
    <div className="modal-projeto-fundo">
      <div
        ref={caixa}
        className="modal-projeto"
        role="dialog"
        aria-modal="true"
        aria-labelledby="modal-projeto-titulo"
        tabIndex={-1}
        onKeyDown={teclas}
      >
        <header className="modal-projeto-topo">
          <h2 id="modal-projeto-titulo">Novo projeto</h2>
          <button
            className="btn btn-sm"
            type="button"
            aria-label="Fechar"
            onClick={aoFechar}
          >
            ×
          </button>
        </header>
        <AssistenteProjeto
          api={api}
          aoConcluir={aoConcluir}
          aoPrepararAmbiente={aoPrepararAmbiente}
          aoCancelar={aoFechar}
        />
      </div>
    </div>
  );
}
