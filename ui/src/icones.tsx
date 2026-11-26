import type { ReactNode } from "react";

/**
 * Ícones de traço (24×24, `currentColor`) das telas novas. Desenhados à mão, no
 * mesmo estilo dos que já existem no app, para não trazer uma biblioteca.
 */
function Svg({ children, tamanho = 16 }: { children: ReactNode; tamanho?: number }) {
  return (
    <svg
      width={tamanho}
      height={tamanho}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {children}
    </svg>
  );
}

export const IconeVoltar = () => (
  <Svg>
    <path d="M19 12H5" />
    <path d="M12 19l-7-7 7-7" />
  </Svg>
);
export const IconeBuscar = () => (
  <Svg>
    <circle cx="11" cy="11" r="7" />
    <path d="M20 20l-3.5-3.5" />
  </Svg>
);
export const IconeConfirmar = () => (
  <Svg>
    <path d="M20 6L9 17l-5-5" />
  </Svg>
);
export const IconeMais = () => (
  <Svg>
    <path d="M12 5v14" />
    <path d="M5 12h14" />
  </Svg>
);
/** "⋯": o menu de mais ações. */
export const IconeReticencias = () => (
  <Svg>
    <circle cx="5" cy="12" r="1" />
    <circle cx="12" cy="12" r="1" />
    <circle cx="19" cy="12" r="1" />
  </Svg>
);
export const IconeDescartar = () => (
  <Svg>
    <path d="M18 6L6 18" />
    <path d="M6 6l12 12" />
  </Svg>
);
export const IconeDesfazer = () => (
  <Svg>
    <path d="M9 14L4 9l5-5" />
    <path d="M4 9h11a5 5 0 0 1 0 10h-3" />
  </Svg>
);
/** Justificar sem funcionalidade: um círculo cortado. */
export const IconeSemFuncionalidade = () => (
  <Svg>
    <circle cx="12" cy="12" r="9" />
    <path d="M5.6 5.6l12.8 12.8" />
  </Svg>
);
export const IconeIa = () => (
  <Svg>
    <path d="M12 3l1.8 4.7L18.5 9.5l-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8z" />
    <path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z" />
  </Svg>
);
export const IconeRestaurar = () => (
  <Svg>
    <path d="M3 12a9 9 0 1 0 3-6.7" />
    <path d="M3 4v5h5" />
  </Svg>
);
export const IconeLinkExterno = () => (
  <Svg tamanho={13}>
    <path d="M14 4h6v6" />
    <path d="M10 14L20 4" />
    <path d="M19 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5" />
  </Svg>
);
export const IconeLigada = () => (
  <Svg tamanho={14}>
    <path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7" />
    <path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7" />
  </Svg>
);
export const IconeInfo = () => (
  <Svg>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5" />
    <path d="M12 8h.01" />
  </Svg>
);

/** Atenção: triângulo com exclamação (alerta de atenção). */
export const IconeAtencao = () => (
  <Svg tamanho={18}>
    <path d="M10.3 3.9L1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" />
    <path d="M12 9v4" />
    <path d="M12 17h.01" />
  </Svg>
);
/** Erro: círculo com exclamação (alerta de erro). */
export const IconeErro = () => (
  <Svg tamanho={18}>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 8v5" />
    <path d="M12 16h.01" />
  </Svg>
);

// O botão só de ícone mora nos componentes compartilhados (#406).
export { BotaoIcone } from "./componentes/BotaoIcone";
