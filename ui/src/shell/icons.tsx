// Ícones de traço do shell, na cor do texto (`currentColor`), sem biblioteca.
// Origem: deep-ion-ai/ctower-agents, ui/src/shell/icons.tsx (commit e458ff6,
// Apache-2.0), com o subconjunto usado aqui e os ícones das áreas deste app (#85).
const PATHS = {
  terminal: (
    <>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M7 9l3 3-3 3M13 15h4" />
    </>
  ),
  project: <path d="M3 6h6l2 2h10v11H3z" />,
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1" />
    </>
  ),
  search: (
    <>
      <circle cx="11" cy="11" r="7" />
      <path d="M20 20l-4-4" />
    </>
  ),
  sync: <path d="M20 11a8 8 0 0 0-14-5l-2 2M4 13a8 8 0 0 0 14 5l2-2M4 4v4h4M20 20v-4h-4" />,
  close: <path d="M6 6l12 12M18 6L6 18" />,
  chevronDown: <path d="M6 9l6 6 6-6" />,
  chevronRight: <path d="M9 6l6 6-6 6" />,
  sidebar: (
    <>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M9 4v16" />
    </>
  ),
  warning: (
    <>
      <path d="M12 3l10 18H2z" />
      <path d="M12 10v5M12 18h.01" />
    </>
  ),
  plus: <path d="M12 5v14M5 12h14" />,
  github: (
    <path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.9a3.4 3.4 0 0 0-.9-2.6c3.1-.4 6.4-1.5 6.4-7A5.4 5.4 0 0 0 20 4.8 5 5 0 0 0 19.9 1S18.7.7 16 2.5a13.4 13.4 0 0 0-7 0C6.3.7 5.1 1 5.1 1A5 5 0 0 0 5 4.8a5.4 5.4 0 0 0-1.5 3.7c0 5.5 3.3 6.6 6.4 7a3.4 3.4 0 0 0-.9 2.6V22" />
  ),
  gitlab: (
    <path d="M22.6 14.4L12 22.1 1.4 14.4a.8.8 0 0 1-.3-.9l1.2-3.8 2.4-7.5a.4.4 0 0 1 .8 0l2.4 7.5h8.2l2.4-7.5a.4.4 0 0 1 .8 0l2.4 7.5 1.2 3.8a.8.8 0 0 1-.3.9z" />
  ),
  jira: (
    <>
      <path d="M12 3l9 9-9 9-9-9z" />
      <path d="M12 8.5l3.5 3.5-3.5 3.5-3.5-3.5z" />
    </>
  ),
  menu: <path d="M4 7h16M4 12h16M4 17h16" />,
  send: <path d="M21 3L10 14M21 3l-7 18-4-7-7-4z" />,
  code: <path d="M8 7l-5 5 5 5M16 7l5 5-5 5" />,
  stop: <rect x="6" y="6" width="12" height="12" rx="1.5" />,
  /** Início: casa. */
  home: <path d="M3 11l9-7 9 7M5 9.5V20h5v-6h4v6h5V9.5" />,
  /** Base de conhecimento: livro aberto. */
  book: (
    <>
      <path d="M12 6c-2-1.5-5-2-9-2v14c4 0 7 .5 9 2 2-1.5 5-2 9-2V4c-4 0-7 .5-9 2z" />
      <path d="M12 6v14" />
    </>
  ),
  /** Análise: documento com lupa. */
  analysis: (
    <>
      <path d="M14 3H6v18h6M14 3l4 4v3M14 3v4h4" />
      <circle cx="16.5" cy="15.5" r="3" />
      <path d="M21 20l-2.4-2.4" />
    </>
  ),
  /** Documento da base. */
  file: <path d="M14 3H6v18h12V7zM14 3v4h4M9 12h6M9 16h6" />,
  /** Tema claro, escuro e do sistema. */
  sun: (
    <>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
    </>
  ),
  moon: <path d="M20 14.5A8 8 0 0 1 9.5 4 8 8 0 1 0 20 14.5z" />,
  monitor: (
    <>
      <rect x="3" y="4" width="18" height="12" rx="2" />
      <path d="M8 20h8M12 16v4" />
    </>
  ),
  /** Revisão do documento (#130). */
  check: <path d="M5 12.5l4.5 4.5L19 7.5" />,
  flag: <path d="M5 21V4M5 4h11l-2 4 2 4H5" />,
  deepen: (
    <>
      <circle cx="11" cy="11" r="7" />
      <path d="M20 20l-4-4M11 8v6M8 11h6" />
    </>
  ),
  pencil: <path d="M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4" />,
  undo: <path d="M9 14L4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3" />,
  reply: <path d="M10 8L4 13l6 5M4 13h11a5 5 0 0 1 5 5v1" />,
  trash: <path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" />,
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 16 }: { name: IconName; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {PATHS[name]}
    </svg>
  );
}
