import type { ReactNode } from "react";
import { Icon, type IconName } from "./icons";
import { THEME_PREFS, useTheme, type ThemePref } from "./theme";

const ICONE_TEMA: Record<ThemePref, IconName> = { system: "monitor", light: "sun", dark: "moon" };

/** Marca do produto: o mesmo alvo do logo do Control Tower, em âmbar. */
export function Logo() {
  return (
    <span className="brand-logo" aria-hidden="true">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
        <circle cx="12" cy="12" r="9" />
        <circle cx="12" cy="12" r="3.5" />
        <path d="M12 3v5M12 16v5M3 12h5M16 12h5" />
      </svg>
    </span>
  );
}

/** Tema claro, escuro ou do sistema; o botão escolhido fica marcado. */
export function AlternanciaTema() {
  const [pref, mudar] = useTheme();
  return (
    <div className="theme-switch" role="group" aria-label="Tema">
      {THEME_PREFS.map((t) => (
        <button
          key={t.pref}
          type="button"
          className="ibtn"
          aria-label={`Tema ${t.rotulo.toLowerCase()}`}
          title={`Tema ${t.rotulo.toLowerCase()}`}
          aria-pressed={pref === t.pref}
          onClick={() => mudar(t.pref)}
        >
          <Icon name={ICONE_TEMA[t.pref]} size={14} />
        </button>
      ))}
    </div>
  );
}

/**
 * Barra de título (#85), no desenho do Control Tower: marca e versão à esquerda,
 * ações à direita. `antes` entra antes da marca (ex.: o botão da gaveta).
 */
export function Titlebar({
  produto,
  versao,
  antes,
  acoes,
}: {
  produto: string;
  versao: string | null;
  antes?: ReactNode;
  acoes?: ReactNode;
}) {
  return (
    <header className="titlebar">
      {antes}
      <div className="brand">
        <Logo />
        <h1 className="brand-name">{produto}</h1>
        <span className="brand-version" aria-label="Versão">
          {versao ? `Versão ${versao}` : ""}
        </span>
      </div>
      <span className="titlebar-spacer" />
      <div className="titlebar-actions">
        {acoes}
        <AlternanciaTema />
      </div>
    </header>
  );
}
