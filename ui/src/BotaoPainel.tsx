import { useEffect, useId, useRef, type ReactNode } from "react";

interface Props {
  aberto: boolean;
  aoAlternar: () => void;
  aoFechar: () => void;
  /** Rótulo acessível do botão (o botão pode ser só um ícone). */
  rotulo: string;
  /** O que aparece no botão; sem ele, o rótulo. */
  conteudo?: ReactNode;
  titulo?: string;
  classeBotao?: string;
  /** Rótulo da região do painel. */
  rotuloPainel: string;
  classePainel?: string;
  desabilitado?: boolean;
  children: ReactNode;
}

/**
 * Botão da barra que abre um painel flutuante logo abaixo dele (#130). O painel
 * fecha pelo mesmo botão, com Esc (o foco volta ao botão) ou com um clique fora.
 */
export function BotaoPainel({
  aberto,
  aoAlternar,
  aoFechar,
  rotulo,
  conteudo,
  titulo,
  classeBotao = "btn",
  rotuloPainel,
  classePainel,
  desabilitado,
  children,
}: Props) {
  const id = useId();
  const raiz = useRef<HTMLDivElement>(null);
  const botao = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!aberto) return;
    const tecla = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      aoFechar();
      botao.current?.focus();
    };
    const clique = (e: MouseEvent) => {
      if (raiz.current && e.target instanceof Node && !raiz.current.contains(e.target)) aoFechar();
    };
    document.addEventListener("keydown", tecla);
    document.addEventListener("mousedown", clique);
    return () => {
      document.removeEventListener("keydown", tecla);
      document.removeEventListener("mousedown", clique);
    };
  }, [aberto, aoFechar]);

  return (
    <div className="botao-painel" ref={raiz}>
      <button
        ref={botao}
        type="button"
        className={classeBotao}
        aria-label={rotulo}
        aria-expanded={aberto}
        aria-controls={aberto ? id : undefined}
        title={titulo ?? rotulo}
        disabled={desabilitado}
        onClick={aoAlternar}
      >
        {conteudo ?? rotulo}
      </button>
      {aberto && (
        <section
          id={id}
          className={classePainel ? `painel-flutuante ${classePainel}` : "painel-flutuante"}
          aria-label={rotuloPainel}
        >
          {children}
        </section>
      )}
    </div>
  );
}

