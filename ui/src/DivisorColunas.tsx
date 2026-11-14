import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type RefObject } from "react";

/** Largura inicial da coluna da conversa, em px. */
export const LARGURA_PADRAO = 460;
/** Menor largura da conversa e menor largura que sobra ao documento. */
export const LARGURA_MINIMA = 320;
export const DOCUMENTO_MINIMO = 360;
const PASSO = 24;
const CHAVE = "cba.larguraConversa";

function lerGuardada(): number {
  try {
    const v = Number(localStorage.getItem(CHAVE));
    return Number.isFinite(v) && v >= LARGURA_MINIMA ? v : LARGURA_PADRAO;
  } catch {
    return LARGURA_PADRAO;
  }
}

function guardar(v: number) {
  try {
    localStorage.setItem(CHAVE, String(Math.round(v)));
  } catch {
    // Sem armazenamento: a largura vale só nesta tela.
  }
}

/**
 * Largura da coluna da conversa, ajustável pelo {@link DivisorColunas} e lembrada
 * entre as aberturas (#130).
 */
export function useLarguraConversa() {
  const [largura, setLargura] = useState(lerGuardada);
  const mudar = useCallback((v: number) => {
    setLargura(v);
    guardar(v);
  }, []);
  return [largura, mudar] as const;
}

interface Props {
  /** As colunas: a conversa encosta na borda direita delas. */
  colunas: RefObject<HTMLElement | null>;
  largura: number;
  aoMudar: (largura: number) => void;
}

/**
 * Divisor arrastável entre o documento e a conversa. Também responde ao teclado
 * (setas, Home e End) e volta à largura padrão com duplo clique.
 */
export function DivisorColunas({ colunas, largura, aoMudar }: Props) {
  const [arrastando, setArrastando] = useState(false);
  const inicio = useRef<{ x: number; largura: number } | null>(null);

  const maxima = useCallback(() => {
    const total = colunas.current?.clientWidth ?? 0;
    // Sem largura medida (colunas ocultas ou empilhadas), não há teto.
    if (total === 0) return Number.MAX_SAFE_INTEGER;
    return Math.max(LARGURA_MINIMA, total - DOCUMENTO_MINIMO);
  }, [colunas]);
  const limitar = useCallback(
    (v: number) => Math.min(maxima(), Math.max(LARGURA_MINIMA, v)),
    [maxima],
  );

  // A janela encolheu: a conversa não pode tomar o espaço mínimo do documento.
  useEffect(() => {
    const el = colunas.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const obs = new ResizeObserver(() => {
      const v = limitar(largura);
      if (v !== largura) aoMudar(v);
    });
    obs.observe(el);
    return () => obs.disconnect();
  }, [colunas, largura, limitar, aoMudar]);

  const aoApertar = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    inicio.current = { x: e.clientX, largura };
    setArrastando(true);
  };
  const aoMover = (e: PointerEvent<HTMLDivElement>) => {
    if (!inicio.current) return;
    // A conversa fica à direita: arrastar para a esquerda a alarga.
    aoMudar(limitar(inicio.current.largura + inicio.current.x - e.clientX));
  };
  const aoSoltar = (e: PointerEvent<HTMLDivElement>) => {
    if (!inicio.current) return;
    e.currentTarget.releasePointerCapture(e.pointerId);
    inicio.current = null;
    setArrastando(false);
  };
  const aoTeclar = (e: KeyboardEvent<HTMLDivElement>) => {
    const novo =
      e.key === "ArrowLeft"
        ? largura + PASSO
        : e.key === "ArrowRight"
          ? largura - PASSO
          : e.key === "Home"
            ? maxima()
            : e.key === "End"
              ? LARGURA_MINIMA
              : null;
    if (novo === null) return;
    e.preventDefault();
    aoMudar(limitar(novo));
  };

  return (
    <div
      className="divisor-colunas"
      role="separator"
      aria-orientation="vertical"
      aria-label="Largura da conversa"
      aria-valuemin={LARGURA_MINIMA}
      aria-valuemax={Math.min(maxima(), 4000)}
      aria-valuenow={Math.round(largura)}
      title="Arraste para ajustar a largura da conversa (duplo clique volta ao padrão)"
      tabIndex={0}
      data-arrastando={arrastando || undefined}
      onPointerDown={aoApertar}
      onPointerMove={aoMover}
      onPointerUp={aoSoltar}
      onPointerCancel={aoSoltar}
      onDoubleClick={() => aoMudar(limitar(LARGURA_PADRAO))}
      onKeyDown={aoTeclar}
    />
  );
}
