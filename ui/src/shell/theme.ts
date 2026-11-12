// Tema da UI (#85): `data-theme` no <html> escolhe os tokens de theme.css.
// Origem: deep-ion-ai/ctower-agents, ui/src/state/theme.ts (commit e458ff6,
// Apache-2.0). Lá a preferência vem do ui.json do app nativo; aqui ela vive só no
// localStorage, lido pelo public/theme-boot.js antes do primeiro quadro.
import { useCallback, useEffect, useState } from "react";

export type ThemePref = "light" | "dark" | "system";
export type Theme = "light" | "dark";

/** Preferência gravada, lida pelo theme-boot.js na próxima abertura. */
export const THEME_KEY = "cba.theme";

export const THEME_PREFS: { pref: ThemePref; rotulo: string }[] = [
  { pref: "system", rotulo: "Do sistema" },
  { pref: "light", rotulo: "Claro" },
  { pref: "dark", rotulo: "Escuro" },
];

const DARK_QUERY = "(prefers-color-scheme: dark)";

const systemTheme = (): Theme =>
  typeof matchMedia === "function" && !matchMedia(DARK_QUERY).matches ? "light" : "dark";

export const resolveTheme = (pref: ThemePref): Theme => (pref === "system" ? systemTheme() : pref);

const isPref = (v: unknown): v is ThemePref => v === "light" || v === "dark" || v === "system";

export function readThemePref(): ThemePref {
  try {
    const v = localStorage.getItem(THEME_KEY);
    return isPref(v) ? v : "system";
  } catch {
    return "system";
  }
}

export function applyTheme(pref: ThemePref): Theme {
  const theme = resolveTheme(pref);
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(THEME_KEY, pref);
  } catch {
    // Sem localStorage: vale só nesta abertura.
  }
  window.dispatchEvent(new CustomEvent<ThemePref>(THEME_EVENT, { detail: pref }));
  return theme;
}

/** Avisa os outros `useTheme` (barra de título e configurações ficam em sincronia). */
const THEME_EVENT = "cba-theme";

/** Preferência de tema, aplicada ao mudar e acompanhando o sistema quando for `system`. */
export function useTheme(): [ThemePref, (p: ThemePref) => void] {
  const [pref, setPref] = useState<ThemePref>(readThemePref);

  useEffect(() => {
    const onEvent = (e: Event) => setPref((e as CustomEvent<ThemePref>).detail);
    window.addEventListener(THEME_EVENT, onEvent);
    return () => window.removeEventListener(THEME_EVENT, onEvent);
  }, []);

  useEffect(() => {
    if (pref !== "system" || typeof matchMedia !== "function") return;
    const mq = matchMedia(DARK_QUERY);
    const onChange = () => {
      document.documentElement.dataset.theme = resolveTheme("system");
    };
    mq.addEventListener?.("change", onChange);
    return () => mq.removeEventListener?.("change", onChange);
  }, [pref]);

  const mudar = useCallback((p: ThemePref) => {
    applyTheme(p);
    setPref(p);
  }, []);

  return [pref, mudar];
}
