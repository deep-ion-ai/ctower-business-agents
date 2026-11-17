import { useId, useRef, type KeyboardEvent, type ReactNode } from "react";

/** Abas do painel lateral da análise (#185). */
export type Aba = "conversa" | "codigo";

const ABAS: { id: Aba; rotulo: string }[] = [
  { id: "conversa", rotulo: "Conversa com o agente" },
  { id: "codigo", rotulo: "Código" },
];

interface Props {
  aba: Aba;
  aoMudarAba: (aba: Aba) => void;
  conversa: ReactNode;
  codigo: ReactNode;
  /** Há uma evidência aberta: a aba Código ganha o marcador. */
  codigoAberto?: boolean;
}

/**
 * Painel à direita da análise, com as abas "Conversa com o agente" e "Código". As duas
 * ficam montadas e a que não está à vista fica `hidden`: desmontar a conversa
 * encerraria o processo do Claude. Setas, Home e End trocam de aba (padrão WAI-ARIA).
 */
export function PainelLateral({ aba, aoMudarAba, conversa, codigo, codigoAberto }: Props) {
  // O `:` do useId atrapalha seletores de id (aria-labelledby em leitores e testes).
  const base = useId().replace(/:/g, "");
  const botoes = useRef<Record<Aba, HTMLButtonElement | null>>({ conversa: null, codigo: null });

  const teclado = (e: KeyboardEvent<HTMLButtonElement>) => {
    const atual = ABAS.findIndex((a) => a.id === aba);
    const destino =
      e.key === "ArrowRight"
        ? (atual + 1) % ABAS.length
        : e.key === "ArrowLeft"
          ? (atual - 1 + ABAS.length) % ABAS.length
          : e.key === "Home"
            ? 0
            : e.key === "End"
              ? ABAS.length - 1
              : -1;
    if (destino < 0) return;
    e.preventDefault();
    const nova = ABAS[destino].id;
    aoMudarAba(nova);
    botoes.current[nova]?.focus();
  };

  const idAba = (a: Aba) => `${base}-aba-${a}`;
  const idPainel = (a: Aba) => `${base}-painel-${a}`;

  return (
    <section className="painel-conversa painel-lateral" aria-label="Painel lateral">
      <div className="abas-lateral" role="tablist" aria-label="Painel lateral">
        {ABAS.map((a) => (
          <button
            key={a.id}
            ref={(el) => {
              botoes.current[a.id] = el;
            }}
            id={idAba(a.id)}
            type="button"
            role="tab"
            className="aba-lateral"
            aria-selected={aba === a.id}
            aria-controls={idPainel(a.id)}
            tabIndex={aba === a.id ? 0 : -1}
            data-aberta={a.id === "codigo" && codigoAberto ? "true" : undefined}
            onClick={() => aoMudarAba(a.id)}
            onKeyDown={teclado}
          >
            {a.rotulo}
          </button>
        ))}
      </div>
      {ABAS.map((a) => (
        <div
          key={a.id}
          id={idPainel(a.id)}
          role="tabpanel"
          aria-labelledby={idAba(a.id)}
          className="painel-aba"
          hidden={aba !== a.id}
        >
          {a.id === "conversa" ? conversa : codigo}
        </div>
      ))}
    </section>
  );
}
